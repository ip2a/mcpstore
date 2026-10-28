//! Host-side request telemetry persistence: every tool call and management
//! operation lands as one row in a single local SQLite file (WAL mode).
//! Writes go through a bounded queue + dedicated writer thread so the call hot
//! path never blocks on I/O; reads open their own short-lived connection (WAL
//! allows concurrent readers alongside the single writer).
//!
//! This lives in the app/daemon layer, not the SDK: the SDK only knows
//! [`mcpstore::telemetry::RequestRecord`] and the sink trait.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mcpstore::telemetry::{FrameRecord, RequestRecord};
use rusqlite::{Connection, types::Value};

/// Records older than this are purged at open and once per hour by the writer.
// ponytail: constant retention; wire to `[diagnostics]` config when a real need shows up.
const RETENTION_DAYS: i64 = 30;
/// Frames decay faster than requests (handshake/list chatter dominates), so
/// they live on a shorter clock.
// ponytail: same as RETENTION_DAYS — promote both together when configurable.
const FRAME_RETENTION_DAYS: i64 = 7;
/// Per-field cap on stored args/result JSON; oversized payloads are truncated
/// and flagged instead of silently bloating the DB.
// ponytail: 1 MiB flat cap; make configurable if a payload-heavy workload complains.
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const QUEUE_CAP: usize = 1024;
const BATCH_MAX: usize = 64;

#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub caller: Option<String>,
    pub service: Option<String>,
    pub tool: Option<String>,
    pub success: Option<bool>,
    pub from_unix_ms: Option<i64>,
    pub to_unix_ms: Option<i64>,
    /// Page backwards: only records with `id < before_id`.
    pub before_id: Option<i64>,
    pub limit: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageTotals {
    pub bucket_unix_ms: i64,
    pub calls: i64,
    pub errors: i64,
    pub avg_ms: f64,
    pub max_ms: i64,
}

/// One day of the frame-capture heatmap: client requests per MCP primitive
/// (gateway face) plus error envelopes seen that day.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HeatmapDay {
    pub bucket_unix_ms: i64,
    pub tool_calls: i64,
    pub resource_calls: i64,
    pub prompt_calls: i64,
    pub other_calls: i64,
    pub errors: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageSeriesPoint {
    pub bucket_unix_ms: i64,
    pub label: String,
    pub calls: i64,
    pub errors: i64,
    pub avg_ms: f64,
    pub max_ms: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageStats {
    pub from_unix_ms: i64,
    pub to_unix_ms: i64,
    pub by_hour: Vec<UsageTotals>,
    pub by_caller: Vec<UsageSeriesPoint>,
    pub by_service: Vec<UsageSeriesPoint>,
    pub by_tool: Vec<UsageSeriesPoint>,
    pub by_instance: Vec<UsageSeriesPoint>,
}

/// One connection's frame timeline summary (a session spans handshake → close).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FrameSessionSummary {
    pub session_id: String,
    pub face: String,
    pub caller: Option<String>,
    pub peer: Option<String>,
    pub instance_id: Option<String>,
    pub first_ts_unix_ms: i64,
    pub last_ts_unix_ms: i64,
    pub frame_count: i64,
}

/// What crosses the writer queue: completed requests or raw frames.
enum TelemetryItem {
    Request(RequestRecord),
    Frame(FrameRecord),
}

const SELECT_COLUMNS: &str = "id, request_id, started_at_unix_ms, duration_ms, kind, caller, \
     scope, session_key, instance_id, service_name, tool, success, error_code, error_summary, \
     args_json, result_json, args_truncated, result_truncated";

/// Default DB path: `telemetry.db` beside the app config (same dir as gateway-wired.json).
pub fn default_db_path() -> PathBuf {
    mcpstore::ConfigManager::new()
        .app_config_path()
        .parent()
        .map(|dir| dir.join("telemetry.db"))
        .unwrap_or_else(|| PathBuf::from("telemetry.db"))
}

fn open_connection(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS requests (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            request_id TEXT NOT NULL,
            started_at_unix_ms INTEGER NOT NULL,
            duration_ms INTEGER NOT NULL,
            kind TEXT NOT NULL,
            caller TEXT,
            scope TEXT,
            session_key TEXT,
            instance_id TEXT,
            service_name TEXT,
            tool TEXT,
            success INTEGER NOT NULL,
            error_code TEXT,
            error_summary TEXT,
            args_json TEXT,
            result_json TEXT,
            args_truncated INTEGER NOT NULL DEFAULT 0,
            result_truncated INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_requests_time ON requests(started_at_unix_ms);
        CREATE INDEX IF NOT EXISTS idx_requests_kind_time ON requests(kind, started_at_unix_ms);
        CREATE INDEX IF NOT EXISTS idx_requests_caller ON requests(caller, started_at_unix_ms);
        CREATE INDEX IF NOT EXISTS idx_requests_service_tool
            ON requests(service_name, tool, started_at_unix_ms);
        CREATE INDEX IF NOT EXISTS idx_requests_success_time
            ON requests(success, started_at_unix_ms);
        CREATE TABLE IF NOT EXISTS frames (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            ts_unix_ms INTEGER NOT NULL,
            face TEXT NOT NULL,
            direction TEXT NOT NULL,
            method TEXT,
            jsonrpc_id TEXT,
            kind TEXT NOT NULL,
            payload TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            truncated INTEGER NOT NULL DEFAULT 0,
            caller TEXT,
            instance_id TEXT,
            peer TEXT,
            transport TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_frames_session ON frames(session_id, ts_unix_ms);
        CREATE INDEX IF NOT EXISTS idx_frames_face_time ON frames(face, ts_unix_ms);",
    )
}

fn purge_expired(conn: &Connection) -> rusqlite::Result<()> {
    let request_cutoff = now_ms() - RETENTION_DAYS * 24 * 3600 * 1000;
    conn.execute(
        "DELETE FROM requests WHERE started_at_unix_ms < ?1",
        [request_cutoff],
    )?;
    let frame_cutoff = now_ms() - FRAME_RETENTION_DAYS * 24 * 3600 * 1000;
    conn.execute("DELETE FROM frames WHERE ts_unix_ms < ?1", [frame_cutoff])?;
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Truncate a serialized payload to MAX_PAYLOAD_BYTES on a UTF-8 char boundary.
fn cap_payload(payload: Option<String>) -> (Option<String>, bool) {
    let Some(payload) = payload else {
        return (None, false);
    };
    if payload.len() <= MAX_PAYLOAD_BYTES {
        return (Some(payload), false);
    }
    let mut end = MAX_PAYLOAD_BYTES;
    while end > 0 && !payload.is_char_boundary(end) {
        end -= 1;
    }
    (Some(payload[..end].to_string()), true)
}

fn insert_batch(conn: &Connection, batch: &[RequestRecord]) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO requests (request_id, started_at_unix_ms, duration_ms, kind, caller, scope, \
         session_key, instance_id, service_name, tool, success, error_code, error_summary, \
         args_json, result_json, args_truncated, result_truncated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
    )?;
    for record in batch {
        stmt.execute(rusqlite::params![
            record.request_id,
            record.started_at_unix_ms,
            record.duration_ms,
            record.kind,
            record.caller,
            record.scope,
            record.session_key,
            record.instance_id,
            record.service_name,
            record.tool,
            record.success,
            record.error_code,
            record.error_summary,
            record.args_json,
            record.result_json,
            record.args_truncated,
            record.result_truncated,
        ])?;
    }
    Ok(())
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestRecord> {
    Ok(RequestRecord {
        id: Some(row.get(0)?),
        request_id: row.get(1)?,
        started_at_unix_ms: row.get(2)?,
        duration_ms: row.get(3)?,
        kind: row.get(4)?,
        caller: row.get(5)?,
        scope: row.get(6)?,
        session_key: row.get(7)?,
        instance_id: row.get(8)?,
        service_name: row.get(9)?,
        tool: row.get(10)?,
        success: row.get::<_, i64>(11)? != 0,
        error_code: row.get(12)?,
        error_summary: row.get(13)?,
        args_json: row.get(14)?,
        result_json: row.get(15)?,
        args_truncated: row.get::<_, i64>(16)? != 0,
        result_truncated: row.get::<_, i64>(17)? != 0,
    })
}

const FRAME_COLUMNS: &str = "id, session_id, ts_unix_ms, face, direction, method, jsonrpc_id, \
     kind, payload, size_bytes, truncated, caller, instance_id, peer, transport";

fn insert_frame_batch(conn: &Connection, batch: &[FrameRecord]) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO frames (session_id, ts_unix_ms, face, direction, method, jsonrpc_id, kind, \
         payload, size_bytes, truncated, caller, instance_id, peer, transport)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    )?;
    for record in batch {
        stmt.execute(rusqlite::params![
            record.session_id,
            record.ts_unix_ms,
            record.face,
            record.direction,
            record.method,
            record.jsonrpc_id,
            record.kind,
            record.payload,
            record.size_bytes,
            record.truncated,
            record.caller,
            record.instance_id,
            record.peer,
            record.transport,
        ])?;
    }
    Ok(())
}

fn row_to_frame(row: &rusqlite::Row<'_>) -> rusqlite::Result<FrameRecord> {
    Ok(FrameRecord {
        id: Some(row.get(0)?),
        session_id: row.get(1)?,
        ts_unix_ms: row.get(2)?,
        face: row.get(3)?,
        direction: row.get(4)?,
        method: row.get(5)?,
        jsonrpc_id: row.get(6)?,
        kind: row.get(7)?,
        payload: row.get(8)?,
        size_bytes: row.get(9)?,
        truncated: row.get::<_, i64>(10)? != 0,
        caller: row.get(11)?,
        instance_id: row.get(12)?,
        peer: row.get(13)?,
        transport: row.get(14)?,
    })
}

/// One telemetry file per data directory. Clone the handle freely; dropping the
/// last clone lets the writer thread drain the queue and exit.
pub struct TelemetryLog {
    path: PathBuf,
    tx: std::sync::mpsc::SyncSender<TelemetryItem>,
    dropped: AtomicU64,
}

impl TelemetryLog {
    /// Open (creating schema if needed) and start the writer thread. Fails loud:
    /// an unusable telemetry file is a startup error, not a silent skip.
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Arc<Self>> {
        let path = path.as_ref().to_path_buf();
        {
            let conn = open_connection(&path)?;
            init_schema(&conn)?;
            purge_expired(&conn)?;
        }
        let (tx, rx) = std::sync::mpsc::sync_channel::<TelemetryItem>(QUEUE_CAP);
        let writer_path = path.clone();
        std::thread::Builder::new()
            .name("telemetry-writer".to_string())
            .spawn(move || {
                let Ok(conn) = open_connection(&writer_path) else {
                    tracing::error!("telemetry writer could not reopen {:?}", writer_path);
                    return;
                };
                let mut last_purge = std::time::Instant::now();
                loop {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(first) => {
                            let mut batch = Vec::with_capacity(BATCH_MAX);
                            batch.push(first);
                            while batch.len() < BATCH_MAX {
                                match rx.try_recv() {
                                    Ok(more) => batch.push(more),
                                    Err(_) => break,
                                }
                            }
                            let mut requests = Vec::with_capacity(batch.len());
                            let mut frames = Vec::new();
                            for item in batch {
                                match item {
                                    TelemetryItem::Request(record) => requests.push(record),
                                    TelemetryItem::Frame(record) => frames.push(record),
                                }
                            }
                            if !requests.is_empty() {
                                if let Err(error) = insert_batch(&conn, &requests) {
                                    tracing::warn!(%error, count = requests.len(), "telemetry insert failed");
                                }
                            }
                            if !frames.is_empty() {
                                if let Err(error) = insert_frame_batch(&conn, &frames) {
                                    tracing::warn!(%error, count = frames.len(), "frame insert failed");
                                }
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if last_purge.elapsed() >= Duration::from_secs(3600) {
                                if let Err(error) = purge_expired(&conn) {
                                    tracing::warn!(%error, "telemetry retention purge failed");
                                }
                                last_purge = std::time::Instant::now();
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .expect("spawn telemetry writer thread");
        Ok(Arc::new(Self {
            path,
            tx,
            dropped: AtomicU64::new(0),
        }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Queue a record. Never blocks the caller; when the queue is full the
    /// record is dropped and counted (telemetry must never break a call).
    pub fn record(&self, mut record: RequestRecord) {
        (record.args_json, record.args_truncated) = cap_payload(record.args_json.take());
        (record.result_json, record.result_truncated) = cap_payload(record.result_json.take());
        self.send(TelemetryItem::Request(record));
    }

    /// Queue a frame — same contract as [`record`](Self::record).
    pub fn record_frame(&self, mut record: FrameRecord) {
        (record.payload, record.truncated) =
            match cap_payload(Some(std::mem::take(&mut record.payload))) {
                (Some(payload), truncated) => (payload, truncated),
                (None, truncated) => (String::new(), truncated),
            };
        self.send(TelemetryItem::Frame(record));
    }

    fn send(&self, item: TelemetryItem) {
        match self.tx.try_send(item) {
            Ok(()) => {}
            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                let dropped = self.dropped.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped == 1 || dropped % 1000 == 0 {
                    tracing::warn!(dropped, "telemetry queue full, dropping records");
                }
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {}
        }
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    fn read_conn(&self) -> rusqlite::Result<Connection> {
        open_connection(&self.path)
    }

    /// Newest first, `limit` capped at 500.
    pub fn history(&self, filter: HistoryFilter) -> rusqlite::Result<Vec<RequestRecord>> {
        let conn = self.read_conn()?;
        let mut sql = format!("SELECT {SELECT_COLUMNS} FROM requests WHERE 1=1");
        let mut args: Vec<Value> = Vec::new();
        let mut push = |sql: &mut String, clause: &str, value: Value| {
            sql.push_str(&format!(" {clause}?{}", args.len() + 1));
            args.push(value);
        };
        if let Some(value) = filter.caller.as_deref() {
            push(&mut sql, "AND caller = ", Value::Text(value.to_string()));
        }
        if let Some(value) = filter.service.as_deref() {
            push(&mut sql, "AND service_name = ", Value::Text(value.to_string()));
        }
        if let Some(value) = filter.tool.as_deref() {
            push(&mut sql, "AND tool = ", Value::Text(value.to_string()));
        }
        if let Some(value) = filter.success {
            push(&mut sql, "AND success = ", Value::from(value));
        }
        if let Some(value) = filter.from_unix_ms {
            push(&mut sql, "AND started_at_unix_ms >= ", value.into());
        }
        if let Some(value) = filter.to_unix_ms {
            push(&mut sql, "AND started_at_unix_ms <= ", value.into());
        }
        if let Some(value) = filter.before_id {
            push(&mut sql, "AND id < ", value.into());
        }
        sql.push_str(" ORDER BY id DESC LIMIT ?");
        let limit = filter.limit.clamp(1, 500) as i64;
        sql.push_str(&(args.len() + 1).to_string());
        args.push(limit.into());

        let mut stmt = conn.prepare(&sql)?;
        let records = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), row_to_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(records)
    }

    pub fn get(&self, id: i64) -> rusqlite::Result<Option<RequestRecord>> {
        let conn = self.read_conn()?;
        let mut stmt =
            conn.prepare(&format!("SELECT {SELECT_COLUMNS} FROM requests WHERE id = ?1"))?;
        let mut rows = stmt.query_map([id], row_to_record)?;
        match rows.next() {
            Some(row) => row.map(Some),
            None => Ok(None),
        }
    }

    /// Sessions with frames inside the trailing `hours` window, newest first.
    /// Every frame in a session carries the same caller/peer/instance values,
    /// so the aggregate picks any non-null one.
    pub fn frame_sessions(
        &self,
        face: Option<&str>,
        hours: i64,
    ) -> rusqlite::Result<Vec<FrameSessionSummary>> {
        let conn = self.read_conn()?;
        let from = now_ms() - hours.max(1) * 3600 * 1000;
        let mut sql = "SELECT session_id, MAX(face), MAX(caller), MAX(peer), MAX(instance_id), \
             MIN(ts_unix_ms), MAX(ts_unix_ms), COUNT(*) FROM frames WHERE ts_unix_ms >= ?1"
            .to_string();
        if face.is_some() {
            sql.push_str(" AND face = ?2");
        }
        sql.push_str(" GROUP BY session_id ORDER BY MAX(ts_unix_ms) DESC LIMIT 100");
        let mut stmt = conn.prepare(&sql)?;
        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(FrameSessionSummary {
                session_id: row.get(0)?,
                face: row.get(1)?,
                caller: row.get(2)?,
                peer: row.get(3)?,
                instance_id: row.get(4)?,
                first_ts_unix_ms: row.get(5)?,
                last_ts_unix_ms: row.get(6)?,
                frame_count: row.get(7)?,
            })
        };
        let rows = match face {
            Some(face) => stmt.query_map(rusqlite::params![from, face], map_row)?,
            None => stmt.query_map([from], map_row)?,
        };
        rows.collect()
    }

    /// One session's frame timeline, oldest first.
    pub fn frame_history(&self, session_id: &str, limit: u32) -> rusqlite::Result<Vec<FrameRecord>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {FRAME_COLUMNS} FROM frames WHERE session_id = ?1 ORDER BY id LIMIT ?2"
        ))?;
        let limit = limit.clamp(1, 1000) as i64;
        let rows = stmt.query_map(rusqlite::params![session_id, limit], row_to_frame)?;
        rows.collect()
    }

    pub fn frame_detail(&self, id: i64) -> rusqlite::Result<Option<FrameRecord>> {
        let conn = self.read_conn()?;
        let mut stmt =
            conn.prepare(&format!("SELECT {FRAME_COLUMNS} FROM frames WHERE id = ?1"))?;
        let mut rows = stmt.query_map([id], row_to_frame)?;
        match rows.next() {
            Some(row) => row.map(Some),
            None => Ok(None),
        }
    }

    /// Tool-call usage stats over the trailing `hours` window, hourly buckets,
    /// split per dimension. Only `kind = 'tool'` counts.
    pub fn usage_stats(&self, hours: u32) -> rusqlite::Result<UsageStats> {
        let conn = self.read_conn()?;
        let to = now_ms();
        let from = to - i64::from(hours.max(1)) * 3600 * 1000;
        let totals_sql = format!(
            "SELECT (started_at_unix_ms / 3600000) * 3600000 AS bucket, COUNT(*), \
             SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), AVG(duration_ms), MAX(duration_ms) \
             FROM requests WHERE kind = 'tool' AND started_at_unix_ms >= ?1 \
             GROUP BY bucket ORDER BY bucket"
        );
        let by_hour = {
            let mut stmt = conn.prepare(&totals_sql)?;
            let rows = stmt.query_map([from], |row| {
                Ok(UsageTotals {
                    bucket_unix_ms: row.get(0)?,
                    calls: row.get(1)?,
                    errors: row.get(2)?,
                    avg_ms: row.get(3)?,
                    max_ms: row.get(4)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let series_sql = format!(
            "SELECT (started_at_unix_ms / 3600000) * 3600000 AS bucket, COALESCE({{col}}, '') AS label, \
             COUNT(*), SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), AVG(duration_ms), MAX(duration_ms) \
             FROM requests WHERE kind = 'tool' AND started_at_unix_ms >= ?1 \
             GROUP BY bucket, label ORDER BY bucket LIMIT 500"
        );
        let series = |col: &str| -> rusqlite::Result<Vec<UsageSeriesPoint>> {
            let mut stmt = conn.prepare(&series_sql.replace("{col}", col))?;
            let rows = stmt.query_map([from], |row| {
                Ok(UsageSeriesPoint {
                    bucket_unix_ms: row.get(0)?,
                    label: row.get(1)?,
                    calls: row.get(2)?,
                    errors: row.get(3)?,
                    avg_ms: row.get(4)?,
                    max_ms: row.get(5)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        };
        Ok(UsageStats {
            from_unix_ms: from,
            to_unix_ms: to,
            by_hour,
            by_caller: series("caller")?,
            by_service: series("service_name")?,
            by_tool: series("tool")?,
            by_instance: series("instance_id")?,
        })
    }

    /// Per-day tool-call totals (UTC day buckets) over the trailing `days` window.
    pub fn daily_stats(&self, days: u32) -> rusqlite::Result<Vec<UsageTotals>> {
        let conn = self.read_conn()?;
        let to = now_ms();
        let from = to - i64::from(days.max(1)) * 24 * 3600 * 1000;
        let mut stmt = conn.prepare(
            "SELECT (started_at_unix_ms / 86400000) * 86400000 AS bucket, COUNT(*), \
             SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), AVG(duration_ms), MAX(duration_ms) \
             FROM requests WHERE kind = 'tool' AND started_at_unix_ms >= ?1 \
             GROUP BY bucket ORDER BY bucket",
        )?;
        let rows = stmt.query_map([from], |row| {
            Ok(UsageTotals {
                bucket_unix_ms: row.get(0)?,
                calls: row.get(1)?,
                errors: row.get(2)?,
                avg_ms: row.get(3)?,
                max_ms: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// Per-day MCP-primitive call situation from the frame capture (gateway face only —
    /// client-driven surface; upstream would double-count). Counts client requests per
    /// primitive (tools/resources/prompts/other) plus down-stream error envelopes.
    pub fn heatmap_stats(&self, days: u32) -> rusqlite::Result<Vec<HeatmapDay>> {
        let conn = self.read_conn()?;
        let to = now_ms();
        let from = to - i64::from(days.max(1)) * 24 * 3600 * 1000;
        let mut stmt = conn.prepare(
            "SELECT (ts_unix_ms / 86400000) * 86400000 AS bucket, \
             SUM(CASE WHEN kind = 'request' AND method LIKE 'tools/%' THEN 1 ELSE 0 END), \
             SUM(CASE WHEN kind = 'request' AND method LIKE 'resources/%' THEN 1 ELSE 0 END), \
             SUM(CASE WHEN kind = 'request' AND method LIKE 'prompts/%' THEN 1 ELSE 0 END), \
             SUM(CASE WHEN kind = 'request' AND (method IS NULL OR \
                  (method NOT LIKE 'tools/%' AND method NOT LIKE 'resources/%' \
                   AND method NOT LIKE 'prompts/%')) THEN 1 ELSE 0 END), \
             SUM(CASE WHEN kind = 'error' THEN 1 ELSE 0 END) \
             FROM frames WHERE face = 'gateway' AND ts_unix_ms >= ?1 \
             GROUP BY bucket ORDER BY bucket",
        )?;
        let rows = stmt.query_map([from], |row| {
            Ok(HeatmapDay {
                bucket_unix_ms: row.get(0)?,
                tool_calls: row.get(1)?,
                resource_calls: row.get(2)?,
                prompt_calls: row.get(3)?,
                other_calls: row.get(4)?,
                errors: row.get(5)?,
            })
        })?;
        rows.collect()
    }
}

impl mcpstore::telemetry::RequestTelemetry for TelemetryLog {
    fn record(&self, record: RequestRecord) {
        TelemetryLog::record(self, record);
    }
}

impl mcpstore::telemetry::FrameTelemetry for TelemetryLog {
    fn record_frame(&self, record: FrameRecord) {
        TelemetryLog::record_frame(self, record);
    }
}

/// One-shot synchronous insert for short-lived processes (CLI command audit):
/// open, insert, close. The daemon uses [`TelemetryLog`] instead.
pub fn record_once(path: &Path, mut record: RequestRecord) -> rusqlite::Result<()> {
    (record.args_json, record.args_truncated) = cap_payload(record.args_json.take());
    (record.result_json, record.result_truncated) = cap_payload(record.result_json.take());
    let conn = open_connection(path)?;
    init_schema(&conn)?;
    insert_batch(&conn, std::slice::from_ref(&record))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mcpstore-telemetry-test-{}-{}",
            tag,
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("telemetry.db")
    }

    fn tool_record(caller: &str, service: &str, success: bool) -> RequestRecord {
        let mut record = RequestRecord::new("tool", "search");
        record.caller = Some(caller.to_string());
        record.service_name = Some(service.to_string());
        record.instance_id = Some("inst-1".to_string());
        record.success = success;
        record
    }

    #[test]
    fn roundtrip_history_and_detail() {
        let path = temp_path("roundtrip");
        let log = TelemetryLog::open(&path).unwrap();
        log.record(tool_record("claude", "fs", true));
        let mut failed = tool_record("codex", "git", false);
        failed.error_code = Some("TOOL_FAILED".to_string());
        failed.error_summary = Some("boom".to_string());
        log.record(failed);
        log.record(RequestRecord::new("management", "mcpstore_service_add"));
        // Wait for the writer to drain.
        std::thread::sleep(Duration::from_millis(300));

        let all = log.history(HistoryFilter {
            limit: 50,
            ..Default::default()
        });
        assert_eq!(all.unwrap().len(), 3);

        let failures = log
            .history(HistoryFilter {
                success: Some(false),
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].error_code.as_deref(), Some("TOOL_FAILED"));

        let by_caller = log
            .history(HistoryFilter {
                caller: Some("claude".to_string()),
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(by_caller.len(), 1);
        let id = by_caller[0].id.unwrap();
        assert_eq!(log.get(id).unwrap().unwrap().caller.as_deref(), Some("claude"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn payload_cap_truncates_and_flags() {
        let (payload, truncated) = cap_payload(Some("x".repeat(MAX_PAYLOAD_BYTES + 1)));
        assert!(truncated);
        assert_eq!(payload.unwrap().len(), MAX_PAYLOAD_BYTES);
        let (payload, truncated) = cap_payload(Some("短".repeat(10)));
        assert!(!truncated);
        assert_eq!(payload.unwrap().chars().count(), 10);
    }

    #[test]
    fn heatmap_stats_groups_primitives_by_utc_day() {
        let path = temp_path("heatmap");
        let log = TelemetryLog::open(&path).unwrap();

        let mut tool = FrameRecord::new("gw:s", "gateway", "up");
        tool.from_envelope(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call"}));
        log.record_frame(tool);

        let mut read = FrameRecord::new("gw:s", "gateway", "up");
        read.from_envelope(&serde_json::json!({"jsonrpc":"2.0","id":2,"method":"resources/read"}));
        log.record_frame(read);

        let mut error = FrameRecord::new("gw:s", "gateway", "down");
        error.from_envelope(&serde_json::json!({"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"boom"}}));
        log.record_frame(error);

        // Upstream face frames must not count (gateway face only).
        let mut upstream = FrameRecord::new("up:s", "upstream", "up");
        upstream.from_envelope(&serde_json::json!({"jsonrpc":"2.0","id":9,"method":"tools/call"}));
        log.record_frame(upstream);

        let mut init = FrameRecord::new("gw:s", "gateway", "up");
        init.from_envelope(&serde_json::json!({"jsonrpc":"2.0","id":3,"method":"initialize"}));
        log.record_frame(init);

        let mut notification = FrameRecord::new("gw:s", "gateway", "up");
        notification.from_envelope(&serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        log.record_frame(notification);
        std::thread::sleep(Duration::from_millis(300));

        let days = log.heatmap_stats(53 * 7).unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].tool_calls, 1);
        assert_eq!(days[0].resource_calls, 1);
        assert_eq!(days[0].prompt_calls, 0);
        assert_eq!(days[0].other_calls, 1, "initialize is an other-request; notifications are not calls");
        assert_eq!(days[0].errors, 1);
        assert_eq!(days[0].bucket_unix_ms % 86_400_000, 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn daily_stats_groups_by_utc_day() {
        let path = temp_path("daily");
        let log = TelemetryLog::open(&path).unwrap();
        log.record(tool_record("claude", "fs", true));
        log.record(tool_record("codex", "git", true));
        std::thread::sleep(Duration::from_millis(300));

        let days = log.daily_stats(35).unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].calls, 2);
        assert_eq!(days[0].errors, 0);
        assert_eq!(
            days[0].bucket_unix_ms % 86_400_000,
            0,
            "bucket must be UTC-day aligned"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn usage_stats_groups_by_hour_and_dims() {
        let path = temp_path("stats");
        let log = TelemetryLog::open(&path).unwrap();
        log.record(tool_record("claude", "fs", true));
        log.record(tool_record("claude", "fs", false));
        log.record(tool_record("codex", "git", true));
        log.record(RequestRecord::new("management", "mcpstore_service_add"));
        std::thread::sleep(Duration::from_millis(300));

        let stats = log.usage_stats(24).unwrap();
        assert_eq!(stats.by_hour.len(), 1);
        assert_eq!(stats.by_hour[0].calls, 3);
        assert_eq!(stats.by_hour[0].errors, 1);
        let claude: Vec<_> = stats
            .by_caller
            .iter()
            .filter(|point| point.label == "claude")
            .collect();
        assert_eq!(claude.len(), 1);
        assert_eq!(claude[0].calls, 2);
        // Management records never enter tool stats.
        assert!(stats.by_tool.iter().all(|p| p.label != "mcpstore_service_add"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn retention_purges_expired_rows() {
        let path = temp_path("retention");
        {
            let log = TelemetryLog::open(&path).unwrap();
            log.record(tool_record("claude", "fs", true));
            std::thread::sleep(Duration::from_millis(300));
        }
        // Backdate the row directly, then reopen: open() purges expired rows.
        let conn = open_connection(&path).unwrap();
        let old = now_ms() - (RETENTION_DAYS + 1) * 24 * 3600 * 1000;
        conn.execute("UPDATE requests SET started_at_unix_ms = ?1", [old]).unwrap();
        drop(conn);
        let log = TelemetryLog::open(&path).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            log.history(HistoryFilter {
                limit: 50,
                ..Default::default()
            })
            .unwrap()
            .len(),
            0
        );
        std::fs::remove_file(&path).ok();
    }

    fn frame(session_id: &str, face: &str, direction: &str, method: Option<&str>) -> FrameRecord {
        let mut record = FrameRecord::new(session_id, face, direction);
        record.method = method.map(str::to_string);
        record.kind = match (method, direction) {
            (Some(_), "up") => "request".to_string(),
            (Some(_), _) => "notification".to_string(),
            (None, _) => "response".to_string(),
        };
        record.caller = (face == "gateway").then(|| "claude".to_string());
        record.instance_id = (face == "upstream").then(|| "inst-1".to_string());
        record.peer = Some("peer-x".to_string());
        record.transport = (face == "upstream").then(|| "stdio".to_string());
        record.payload = format!(r#"{{"jsonrpc":"2.0","method":"{method:?}"}}"#);
        record.size_bytes = record.payload.len() as i64;
        record
    }

    #[test]
    fn frame_roundtrip_sessions_history_and_retention() {
        let path = temp_path("frames");
        {
            let log = TelemetryLog::open(&path).unwrap();
            log.record_frame(frame("s-gw", "gateway", "up", Some("initialize")));
            log.record_frame(frame("s-gw", "gateway", "down", None));
            log.record_frame(frame("s-up", "upstream", "up", Some("tools/list")));
            log.record_frame(frame("s-up", "upstream", "down", None));
            std::thread::sleep(Duration::from_millis(300));

            let sessions = log.frame_sessions(None, 24).unwrap();
            assert_eq!(sessions.len(), 2);
            let gw = sessions.iter().find(|s| s.face == "gateway").unwrap();
            assert_eq!(gw.session_id, "s-gw");
            assert_eq!(gw.frame_count, 2);
            assert_eq!(gw.caller.as_deref(), Some("claude"));
            let up = log.frame_sessions(Some("upstream"), 24).unwrap();
            assert_eq!(up.len(), 1);
            assert_eq!(up[0].instance_id.as_deref(), Some("inst-1"));

            let history = log.frame_history("s-up", 50).unwrap();
            assert_eq!(history.len(), 2);
            assert_eq!(history[0].direction, "up");
            assert_eq!(history[1].direction, "down");
            let id = history[0].id.unwrap();
            assert_eq!(
                log.frame_detail(id).unwrap().unwrap().method.as_deref(),
                Some("tools/list")
            );

            // Backdate one session's frames beyond the frame clock, reopen:
            // frames go, requests stay (their retention is longer).
            let conn = open_connection(&path).unwrap();
            let old = now_ms() - (FRAME_RETENTION_DAYS + 1) * 24 * 3600 * 1000;
            conn.execute("UPDATE frames SET ts_unix_ms = ?1", [old]).unwrap();
            drop(conn);
        }
        let log = TelemetryLog::open(&path).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert!(log.frame_sessions(None, 24).unwrap().is_empty());
        std::fs::remove_file(&path).ok();
    }
}
