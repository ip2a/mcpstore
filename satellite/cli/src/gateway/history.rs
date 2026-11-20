use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use mcpstore::config::GatewayCallsConfig;
use rusqlite::Connection;

use super::GatewayCallRecord;

enum HistoryItem {
    Save(GatewayCallRecord),
    Flush(std::sync::mpsc::Sender<()>),
}

/// Durable latest-state gateway call stories in the existing telemetry DB.
pub(super) struct GatewayCallHistory {
    tx: Option<std::sync::mpsc::Sender<HistoryItem>>,
}

impl GatewayCallHistory {
    pub(super) fn open(
        path: &Path,
        config: &GatewayCallsConfig,
    ) -> rusqlite::Result<(Arc<Self>, Vec<GatewayCallRecord>)> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS gateway_calls (
                call_id TEXT PRIMARY KEY,
                seq INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL,
                record_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_gateway_calls_seq ON gateway_calls(seq);",
        )?;
        purge_expired(&conn, config)?;
        let mut stmt =
            conn.prepare("SELECT record_json FROM gateway_calls ORDER BY seq DESC LIMIT ?1")?;
        let memory_capacity = i64::try_from(config.memory_capacity).unwrap_or(i64::MAX);
        let saved = stmt
            .query_map([memory_capacity], |row| row.get::<_, String>(0))?
            .map(|row| {
                let json = row?;
                serde_json::from_str(&json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        drop(conn);

        // Reads and startup cleanup remain available when new persistence is off.
        let tx = if config.enabled && config.persist {
            // Snapshot updates may briefly outpace SQLite; coalesce this queue if
            // sustained writer lag becomes measurable rather than dropping terminals.
            let (tx, rx) = std::sync::mpsc::channel::<HistoryItem>();
            let writer_path = path.to_path_buf();
            let writer_config = config.clone();
            std::thread::Builder::new()
                .name("gateway-call-writer".to_string())
                .spawn(move || {
                    let Ok(conn) = Connection::open(writer_path) else {
                        tracing::error!("gateway call writer could not reopen telemetry DB");
                        return;
                    };
                    if let Err(error) = conn.busy_timeout(Duration::from_secs(5)) {
                        tracing::error!(%error, "gateway call writer could not configure SQLite");
                        return;
                    }
                    let mut last_purge = std::time::Instant::now();
                    loop {
                        match rx.recv_timeout(Duration::from_millis(100)) {
                            Ok(HistoryItem::Flush(done)) => {
                                let _ = done.send(());
                            }
                            Ok(HistoryItem::Save(record)) => {
                                let result = serde_json::to_string(&record)
                                    .map_err(|error| error.to_string());
                                match result {
                                    Ok(json) => {
                                        if let Err(error) = conn.execute(
                                            "INSERT INTO gateway_calls
                                                (call_id, seq, updated_at_unix_ms, record_json)
                                             VALUES (?1, ?2, ?3, ?4)
                                             ON CONFLICT(call_id) DO UPDATE SET
                                                seq = excluded.seq,
                                                updated_at_unix_ms = excluded.updated_at_unix_ms,
                                                record_json = excluded.record_json",
                                            rusqlite::params![
                                                record.call_id,
                                                record.seq,
                                                now_ms(),
                                                json
                                            ],
                                        ) {
                                            tracing::warn!(%error, "gateway call snapshot insert failed");
                                        }
                                    }
                                    Err(error) => {
                                        tracing::warn!(%error, "gateway call snapshot encoding failed")
                                    }
                                }
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                if last_purge.elapsed() >= Duration::from_secs(3600) {
                                    if let Err(error) = purge_expired(&conn, &writer_config) {
                                        tracing::warn!(%error, "gateway call retention purge failed");
                                    }
                                    last_purge = std::time::Instant::now();
                                }
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                })
                .expect("spawn gateway call writer");
            Some(tx)
        } else {
            None
        };
        Ok((Arc::new(Self { tx }), saved))
    }

    pub(super) fn save(&self, record: GatewayCallRecord) {
        if let Some(tx) = &self.tx {
            if let Err(error) = tx.send(HistoryItem::Save(record)) {
                tracing::warn!(%error, "gateway call snapshot queue is unavailable");
            }
        }
    }

    #[cfg(test)]
    pub(super) fn flush(&self) {
        if let Some(tx) = &self.tx {
            let (done, wait) = std::sync::mpsc::channel();
            tx.send(HistoryItem::Flush(done)).unwrap();
            wait.recv().unwrap();
        }
    }
}

fn purge_expired(conn: &Connection, config: &GatewayCallsConfig) -> rusqlite::Result<()> {
    if let Some(days) = config.retention_days {
        let age = i64::try_from(days)
            .unwrap_or(i64::MAX)
            .saturating_mul(24 * 3600 * 1000);
        conn.execute(
            "DELETE FROM gateway_calls WHERE updated_at_unix_ms < ?1",
            [now_ms().saturating_sub(age)],
        )?;
    }
    if let Some(limit) = config.max_calls {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        conn.execute(
            "DELETE FROM gateway_calls WHERE call_id NOT IN \
             (SELECT call_id FROM gateway_calls ORDER BY seq DESC LIMIT ?1)",
            [limit],
        )?;
    }
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::{GatewayCallKind, GatewayCallPhase};
    use mcpstore::{ScopeRef, ServiceInstanceKey};

    fn record() -> GatewayCallRecord {
        GatewayCallRecord {
            seq: 1,
            call_id: "persistent-call".to_string(),
            phase: GatewayCallPhase::CallingUpstream,
            upstream_name: Some("echo".to_string()),
            namespace_applied: false,
            name_mapped: false,
            error: None,
            routing_ms: Some(1),
            request_write_ms: None,
            first_byte_ms: None,
            response_read_ms: None,
            at_unix_ms: now_ms() as u64,
            caller: Some("test".to_string()),
            kind: GatewayCallKind::Tool,
            name: "demo__echo".to_string(),
            instance_id: Some(ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id()),
            service_name: "demo".to_string(),
            duration_ms: 0,
            success: false,
        }
    }

    #[test]
    fn snapshots_upsert_and_restore_latest_state() {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-gateway-history-{}-{}.db",
            std::process::id(),
            now_ms()
        ));
        let mut saved = record();
        let config = GatewayCallsConfig::default();
        {
            let (history, restored) = GatewayCallHistory::open(&path, &config).unwrap();
            assert!(restored.is_empty());
            history.save(saved.clone());
            saved.seq = 2;
            saved.phase = GatewayCallPhase::Succeeded;
            saved.success = true;
            saved.duration_ms = 5;
            history.save(saved.clone());
            history.flush();
        }
        let (_, restored) = GatewayCallHistory::open(&path, &config).unwrap();
        assert_eq!(restored, vec![saved]);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn disabled_persistence_still_reads_and_applies_retention() {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-gateway-history-disabled-{}-{}.db",
            std::process::id(),
            now_ms()
        ));
        let mut old = record();
        old.call_id = "old-call".to_string();
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE gateway_calls (
                call_id TEXT PRIMARY KEY,
                seq INTEGER NOT NULL,
                updated_at_unix_ms INTEGER NOT NULL,
                record_json TEXT NOT NULL
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO gateway_calls (call_id, seq, updated_at_unix_ms, record_json)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                old.call_id,
                old.seq,
                now_ms() - 31 * 24 * 3600 * 1000,
                serde_json::to_string(&old).unwrap()
            ],
        )
        .unwrap();
        old.call_id = "new-call".to_string();
        old.seq = 2;
        conn.execute(
            "INSERT INTO gateway_calls (call_id, seq, updated_at_unix_ms, record_json)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                old.call_id,
                old.seq,
                now_ms(),
                serde_json::to_string(&old).unwrap()
            ],
        )
        .unwrap();
        drop(conn);

        let mut config = GatewayCallsConfig::default();
        config.enabled = false;
        config.persist = false;
        config.max_calls = Some(1);
        let (history, restored) = GatewayCallHistory::open(&path, &config).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].call_id, "new-call");
        history.save(record());
        history.flush();
        drop(history);
        let (_, restored) = GatewayCallHistory::open(&path, &config).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].call_id, "new-call");
        std::fs::remove_file(path).ok();
    }
}
