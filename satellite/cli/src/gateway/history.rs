use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rusqlite::Connection;

use super::GatewayCallRecord;

const RETENTION_DAYS: i64 = 30;
const MAX_CALLS: usize = 5_000;

enum HistoryItem {
    Save(GatewayCallRecord),
    Flush(std::sync::mpsc::Sender<()>),
}

/// Durable latest-state gateway call stories in the existing telemetry DB.
pub(super) struct GatewayCallHistory {
    tx: std::sync::mpsc::Sender<HistoryItem>,
}

impl GatewayCallHistory {
    pub(super) fn open(path: &Path) -> rusqlite::Result<(Arc<Self>, Vec<GatewayCallRecord>)> {
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
        let cutoff = now_ms() - RETENTION_DAYS * 24 * 3600 * 1000;
        conn.execute(
            "DELETE FROM gateway_calls WHERE updated_at_unix_ms < ?1",
            [cutoff],
        )?;
        conn.execute(
            "DELETE FROM gateway_calls WHERE call_id IN (
                SELECT call_id FROM gateway_calls ORDER BY seq DESC LIMIT -1 OFFSET ?1
            )",
            [MAX_CALLS],
        )?;
        let mut stmt = conn.prepare(
            "SELECT record_json FROM gateway_calls ORDER BY seq DESC LIMIT ?1",
        )?;
        let saved = stmt
            .query_map([super::GATEWAY_CALL_LOG_CAP], |row| row.get::<_, String>(0))?
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

        // Snapshot updates may briefly outpace SQLite; coalesce this queue if
        // sustained writer lag becomes measurable rather than dropping terminals.
        let (tx, rx) = std::sync::mpsc::channel::<HistoryItem>();
        let writer_path = path.to_path_buf();
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
                while let Ok(item) = rx.recv() {
                    match item {
                        HistoryItem::Flush(done) => {
                            let _ = done.send(());
                            continue;
                        }
                        HistoryItem::Save(record) => {
                            let result =
                                serde_json::to_string(&record).map_err(|error| error.to_string());
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
                    }
                }
            })
            .expect("spawn gateway call writer");
        Ok((Arc::new(Self { tx }), saved))
    }

    pub(super) fn save(&self, record: GatewayCallRecord) {
        if let Err(error) = self.tx.send(HistoryItem::Save(record)) {
            tracing::warn!(%error, "gateway call snapshot queue is unavailable");
        }
    }

    #[cfg(test)]
    pub(super) fn flush(&self) {
        let (done, wait) = std::sync::mpsc::channel();
        self.tx.send(HistoryItem::Flush(done)).unwrap();
        wait.recv().unwrap();
    }
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
        {
            let (history, restored) = GatewayCallHistory::open(&path).unwrap();
            assert!(restored.is_empty());
            history.save(saved.clone());
            saved.seq = 2;
            saved.phase = GatewayCallPhase::Succeeded;
            saved.success = true;
            saved.duration_ms = 5;
            history.save(saved.clone());
            history.flush();
        }
        let (_, restored) = GatewayCallHistory::open(&path).unwrap();
        assert_eq!(restored, vec![saved]);
        std::fs::remove_file(path).ok();
    }
}
