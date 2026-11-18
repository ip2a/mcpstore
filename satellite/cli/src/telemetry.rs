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

use mcpstore::config::{DiagnosticsConfig, FrameTelemetryConfig, RequestTelemetryConfig};
use mcpstore::telemetry::{FrameRecord, RequestRecord};
use rusqlite::{types::Value, Connection};

const QUEUE_CAP: usize = 1024;
const BATCH_MAX: usize = 64;

#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub caller: Option<String>,
    pub service: Option<String>,
    pub tool: Option<String>,
    pub kind: Option<String>,
    /// Free-text match across caller / service / tool / kind / error.
    pub q: Option<String>,
    pub success: Option<bool>,
    pub from_unix_ms: Option<i64>,
    pub to_unix_ms: Option<i64>,
    /// Page backwards: only records with `id < before_id`.
    pub before_id: Option<i64>,
    pub limit: u32,
    pub offset: u32,
}

/// Paginated request-history listing (server-side filter + LIMIT/OFFSET).
#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryPage {
    pub requests: Vec<RequestRecord>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
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

/// Frame activity grouped by service instance for a time bucket. `instance_id`
/// identifies the (service, scope) pair — `service` is only a display label
/// resolved against the live registry by the API layer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServiceActivityPoint {
    pub bucket_unix_ms: i64,
    pub service: String,
    pub instance_id: Option<String>,
    pub frame_count: i64,
}

/// Frame activity grouped by wire method for a time bucket. Every frame carries
/// its real method (ping, initialize, tools/call, notifications/…), so there is
/// no synthetic "other" bucket here.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MethodActivityPoint {
    pub bucket_unix_ms: i64,
    pub method: String,
    pub frame_count: i64,
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
    /// Filled by the API from the live service catalog (`instance_id` lookup).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    /// Filled by the API: `"store"` or the agent id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub first_ts_unix_ms: i64,
    pub last_ts_unix_ms: i64,
    pub frame_count: i64,
    /// Sum of `size_bytes` for client→server frames (`direction = up`).
    pub request_bytes: i64,
    /// Sum of `size_bytes` for server→client frames (`direction = down`).
    pub response_bytes: i64,
}

/// Paginated frame-session listing (server-side filter + LIMIT/OFFSET).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FrameSessionsPage {
    pub sessions: Vec<FrameSessionSummary>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}

/// What crosses the writer queue: completed requests or raw frames.
enum TelemetryItem {
    Request(RequestRecord),
    Frame(FrameRecord),
}

const SELECT_COLUMNS: &str = "id, request_id, started_at_unix_ms, duration_ms, kind, caller, \
     scope, session_key, instance_id, service_name, tool, success, error_code, error_summary, \
     args_json, result_json, args_truncated, result_truncated, timing_json";

/// Telemetry DB path beside the selected workspace's app config.
pub fn db_path(manager: &mcpstore::ConfigManager) -> PathBuf {
    manager
        .app_config_path()
        .parent()
        .map(|dir| dir.join("telemetry.db"))
        .unwrap_or_else(|| PathBuf::from("telemetry.db"))
}

/// Default DB path: `telemetry.db` beside the default app config.
pub fn default_db_path() -> PathBuf {
    db_path(&mcpstore::ConfigManager::new())
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
            result_truncated INTEGER NOT NULL DEFAULT 0,
            timing_json TEXT
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
            transport TEXT,
            request_id TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_frames_session ON frames(session_id, ts_unix_ms);
        CREATE INDEX IF NOT EXISTS idx_frames_face_time ON frames(face, ts_unix_ms);",
    )?;
    ensure_column(conn, "requests", "timing_json", "TEXT")?;
    ensure_column(conn, "frames", "request_id", "TEXT")?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_requests_request_id ON requests(request_id);
         CREATE INDEX IF NOT EXISTS idx_frames_request_id ON frames(request_id);",
    )
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let exists = {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let mut rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut exists = false;
        for name in &mut rows {
            if name? == column {
                exists = true;
                break;
            }
        }
        exists
    };
    if exists {
        return Ok(());
    }
    conn.execute_batch(&format!(
        "ALTER TABLE {table} ADD COLUMN {column} {definition}"
    ))
}

fn purge_expired(conn: &Connection, diagnostics: &DiagnosticsConfig) -> rusqlite::Result<()> {
    if let Some(days) = diagnostics.requests.retention_days {
        let age = i64::try_from(days)
            .unwrap_or(i64::MAX)
            .saturating_mul(24 * 3600 * 1000);
        conn.execute(
            "DELETE FROM requests WHERE started_at_unix_ms < ?1",
            [now_ms().saturating_sub(age)],
        )?;
    }
    if let Some(limit) = diagnostics.requests.max_records {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        conn.execute(
            "DELETE FROM requests WHERE id NOT IN \
             (SELECT id FROM requests ORDER BY id DESC LIMIT ?1)",
            [limit],
        )?;
    }
    if let Some(days) = diagnostics.frames.retention_days {
        let age = i64::try_from(days)
            .unwrap_or(i64::MAX)
            .saturating_mul(24 * 3600 * 1000);
        conn.execute(
            "DELETE FROM frames WHERE ts_unix_ms < ?1",
            [now_ms().saturating_sub(age)],
        )?;
    }
    if let Some(limit) = diagnostics.frames.max_records {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        conn.execute(
            "DELETE FROM frames WHERE id NOT IN \
             (SELECT id FROM frames ORDER BY id DESC LIMIT ?1)",
            [limit],
        )?;
    }
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Truncate a serialized payload to the configured cap on a UTF-8 char boundary.
fn cap_payload(payload: Option<String>, max_bytes: usize) -> (Option<String>, bool) {
    let Some(payload) = payload else {
        return (None, false);
    };
    if payload.len() <= max_bytes {
        return (Some(payload), false);
    }
    let mut end = max_bytes;
    while end > 0 && !payload.is_char_boundary(end) {
        end -= 1;
    }
    (Some(payload[..end].to_string()), true)
}

fn insert_batch(conn: &Connection, batch: &[RequestRecord]) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO requests (request_id, started_at_unix_ms, duration_ms, kind, caller, scope, \
         session_key, instance_id, service_name, tool, success, error_code, error_summary, \
         args_json, result_json, args_truncated, result_truncated, timing_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
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
            record
                .timing
                .as_ref()
                .map(|timing| serde_json::to_string(timing).unwrap()),
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
        timing: row
            .get::<_, Option<String>>(18)?
            .map(|timing| serde_json::from_str(&timing))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    18,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
    })
}

const FRAME_COLUMNS: &str = "id, session_id, ts_unix_ms, face, direction, method, jsonrpc_id, \
     kind, payload, size_bytes, truncated, caller, instance_id, peer, transport, request_id";

fn insert_frame_batch(conn: &Connection, batch: &[FrameRecord]) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO frames (session_id, ts_unix_ms, face, direction, method, jsonrpc_id, kind, \
         payload, size_bytes, truncated, caller, instance_id, peer, transport, request_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            record.request_id,
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
        request_id: row.get(15)?,
    })
}

/// One telemetry file per data directory. Clone the handle freely; dropping the
/// last clone lets the writer thread drain the queue and exit.
pub struct TelemetryLog {
    path: PathBuf,
    tx: std::sync::mpsc::SyncSender<TelemetryItem>,
    dropped: AtomicU64,
    requests: RequestTelemetryConfig,
    frames: FrameTelemetryConfig,
}

impl TelemetryLog {
    /// Open (creating schema if needed) and start the writer thread. Fails loud:
    /// an unusable telemetry file is a startup error, not a silent skip.
    pub fn open(
        path: impl AsRef<Path>,
        diagnostics: &DiagnosticsConfig,
    ) -> rusqlite::Result<Arc<Self>> {
        let path = path.as_ref().to_path_buf();
        {
            let conn = open_connection(&path)?;
            init_schema(&conn)?;
            purge_expired(&conn, diagnostics)?;
        }
        let (tx, rx) = std::sync::mpsc::sync_channel::<TelemetryItem>(QUEUE_CAP);
        let writer_path = path.clone();
        let writer_diagnostics = diagnostics.clone();
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
                                if let Err(error) = purge_expired(&conn, &writer_diagnostics) {
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
            requests: diagnostics.requests.clone(),
            frames: diagnostics.frames.clone(),
        }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn requests(&self) -> &RequestTelemetryConfig {
        &self.requests
    }

    /// Queue a record. Never blocks the caller; when the queue is full the
    /// record is dropped and counted (telemetry must never break a call).
    pub fn record(&self, mut record: RequestRecord) {
        let source_enabled = match record.kind.as_str() {
            "tool" => self.requests.capture_tool,
            // CLI uses record_once below, so daemon management records have one source switch.
            "management" => self.requests.capture_management,
            _ => true,
        };
        if !self.requests.enabled || !source_enabled {
            return;
        }
        if self.requests.capture_payload {
            (record.args_json, record.args_truncated) =
                cap_payload(record.args_json.take(), self.requests.max_payload_bytes);
            (record.result_json, record.result_truncated) =
                cap_payload(record.result_json.take(), self.requests.max_payload_bytes);
        } else {
            record.args_json = None;
            record.result_json = None;
            record.args_truncated = false;
            record.result_truncated = false;
        }
        self.send(TelemetryItem::Request(record));
    }

    /// Queue a frame — same contract as [`record`](Self::record).
    pub fn record_frame(&self, mut record: FrameRecord) {
        if !self.frames.enabled {
            return;
        }
        if self.frames.capture_payload {
            (record.payload, record.truncated) = match cap_payload(
                Some(std::mem::take(&mut record.payload)),
                self.frames.max_payload_bytes,
            ) {
                (Some(payload), truncated) => (payload, truncated),
                (None, truncated) => (String::new(), truncated),
            };
        } else {
            record.payload.clear();
            record.truncated = false;
        }
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

    /// Newest first. Filtering and pagination happen in SQL — `limit` capped
    /// at 500, `offset` capped at 10_000.
    pub fn history(&self, filter: HistoryFilter) -> rusqlite::Result<HistoryPage> {
        let conn = self.read_conn()?;
        let mut where_sql = String::from("WHERE 1=1");
        let mut args: Vec<Value> = Vec::new();
        fn push(sql: &mut String, args: &mut Vec<Value>, clause: &str, value: Value) {
            sql.push_str(&format!(" {clause}?{}", args.len() + 1));
            args.push(value);
        }
        if let Some(value) = filter.caller.as_deref() {
            push(
                &mut where_sql,
                &mut args,
                "AND caller = ",
                Value::Text(value.to_string()),
            );
        }
        if let Some(value) = filter.service.as_deref() {
            push(
                &mut where_sql,
                &mut args,
                "AND service_name = ",
                Value::Text(value.to_string()),
            );
        }
        if let Some(value) = filter.tool.as_deref() {
            push(
                &mut where_sql,
                &mut args,
                "AND tool = ",
                Value::Text(value.to_string()),
            );
        }
        if let Some(value) = filter.kind.as_deref() {
            push(
                &mut where_sql,
                &mut args,
                "AND kind = ",
                Value::Text(value.to_string()),
            );
        }
        if let Some(value) = filter.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let pattern = format!("%{}%", value);
            let n = args.len() + 1;
            where_sql.push_str(&format!(
                " AND (\
                    IFNULL(caller, '') LIKE ?{n} ESCAPE '\\' \
                    OR IFNULL(service_name, '') LIKE ?{n} ESCAPE '\\' \
                    OR IFNULL(tool, '') LIKE ?{n} ESCAPE '\\' \
                    OR IFNULL(kind, '') LIKE ?{n} ESCAPE '\\' \
                    OR IFNULL(error_code, '') LIKE ?{n} ESCAPE '\\' \
                    OR IFNULL(error_summary, '') LIKE ?{n} ESCAPE '\\'\
                )"
            ));
            args.push(Value::Text(pattern));
        }
        if let Some(value) = filter.success {
            push(
                &mut where_sql,
                &mut args,
                "AND success = ",
                Value::from(value),
            );
        }
        if let Some(value) = filter.from_unix_ms {
            push(
                &mut where_sql,
                &mut args,
                "AND started_at_unix_ms >= ",
                value.into(),
            );
        }
        if let Some(value) = filter.to_unix_ms {
            push(
                &mut where_sql,
                &mut args,
                "AND started_at_unix_ms <= ",
                value.into(),
            );
        }
        if let Some(value) = filter.before_id {
            push(&mut where_sql, &mut args, "AND id < ", value.into());
        }

        let total: i64 = {
            let sql = format!("SELECT COUNT(*) FROM requests {where_sql}");
            let mut stmt = conn.prepare(&sql)?;
            stmt.query_row(rusqlite::params_from_iter(args.iter()), |row| row.get(0))?
        };

        let limit = filter.limit.clamp(1, 500);
        let offset = filter.offset.min(10_000);
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM requests {where_sql} ORDER BY id DESC LIMIT ?{} OFFSET ?{}",
            args.len() + 1,
            args.len() + 2
        );
        let mut page_args = args;
        page_args.push((limit as i64).into());
        page_args.push((offset as i64).into());

        let mut stmt = conn.prepare(&sql)?;
        let requests = stmt
            .query_map(rusqlite::params_from_iter(page_args.iter()), row_to_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(HistoryPage {
            requests,
            total,
            limit,
            offset,
        })
    }

    pub fn get(&self, id: i64) -> rusqlite::Result<Option<RequestRecord>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM requests WHERE id = ?1"
        ))?;
        let mut rows = stmt.query_map([id], row_to_record)?;
        match rows.next() {
            Some(row) => row.map(Some),
            None => Ok(None),
        }
    }

    /// Sessions with frames inside the trailing `hours` window, newest first.
    /// Every frame in a session carries the same caller/peer/instance values,
    /// so the aggregate picks any non-null one. Filtering and pagination happen
    /// in SQL — callers must not pull the full set and slice client-side.
    pub fn frame_sessions(
        &self,
        face: Option<&str>,
        hours: i64,
        limit: u32,
        offset: u32,
    ) -> rusqlite::Result<FrameSessionsPage> {
        let conn = self.read_conn()?;
        let from = now_ms() - hours.max(1) * 3600 * 1000;
        let limit = limit.clamp(1, 100);
        let offset = offset.min(10_000);
        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(FrameSessionSummary {
                session_id: row.get(0)?,
                face: row.get(1)?,
                caller: row.get(2)?,
                peer: row.get(3)?,
                instance_id: row.get(4)?,
                service_name: None,
                scope: None,
                first_ts_unix_ms: row.get(5)?,
                last_ts_unix_ms: row.get(6)?,
                frame_count: row.get(7)?,
                request_bytes: row.get(8)?,
                response_bytes: row.get(9)?,
            })
        };
        let (total, sessions) = match face {
            Some(face) => {
                let total: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM (
                        SELECT session_id FROM frames
                        WHERE ts_unix_ms >= ?1 AND face = ?2
                        GROUP BY session_id
                    )",
                    rusqlite::params![from, face],
                    |row| row.get(0),
                )?;
                let mut stmt = conn.prepare(
                    "SELECT session_id, MAX(face), MAX(caller), MAX(peer), MAX(instance_id), \
                     MIN(ts_unix_ms), MAX(ts_unix_ms), COUNT(*), \
                     COALESCE(SUM(CASE WHEN direction = 'up' THEN size_bytes ELSE 0 END), 0), \
                     COALESCE(SUM(CASE WHEN direction = 'down' THEN size_bytes ELSE 0 END), 0) \
                     FROM frames \
                     WHERE ts_unix_ms >= ?1 AND face = ?2 \
                     GROUP BY session_id ORDER BY MAX(ts_unix_ms) DESC LIMIT ?3 OFFSET ?4",
                )?;
                let rows = stmt.query_map(
                    rusqlite::params![from, face, limit as i64, offset as i64],
                    map_row,
                )?;
                (total, rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            None => {
                let total: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM (
                        SELECT session_id FROM frames
                        WHERE ts_unix_ms >= ?1
                        GROUP BY session_id
                    )",
                    [from],
                    |row| row.get(0),
                )?;
                let mut stmt = conn.prepare(
                    "SELECT session_id, MAX(face), MAX(caller), MAX(peer), MAX(instance_id), \
                     MIN(ts_unix_ms), MAX(ts_unix_ms), COUNT(*), \
                     COALESCE(SUM(CASE WHEN direction = 'up' THEN size_bytes ELSE 0 END), 0), \
                     COALESCE(SUM(CASE WHEN direction = 'down' THEN size_bytes ELSE 0 END), 0) \
                     FROM frames \
                     WHERE ts_unix_ms >= ?1 \
                     GROUP BY session_id ORDER BY MAX(ts_unix_ms) DESC LIMIT ?2 OFFSET ?3",
                )?;
                let rows = stmt.query_map(
                    rusqlite::params![from, limit as i64, offset as i64],
                    map_row,
                )?;
                (total, rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
        };
        Ok(FrameSessionsPage {
            sessions,
            total,
            limit,
            offset,
        })
    }

    /// One session's frame timeline, oldest first.
    pub fn frame_history(
        &self,
        session_id: &str,
        limit: u32,
    ) -> rusqlite::Result<Vec<FrameRecord>> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {FRAME_COLUMNS} FROM frames WHERE session_id = ?1 ORDER BY id LIMIT ?2"
        ))?;
        let limit = limit.clamp(1, 1000) as i64;
        let rows = stmt.query_map(rusqlite::params![session_id, limit], row_to_frame)?;
        rows.collect()
    }

    /// Recent frames across sessions, oldest→newest within the returned window
    /// (fetched newest-first then reversed) so LTR timelines read chronologically.
    pub fn frame_recent(
        &self,
        face: Option<&str>,
        limit: u32,
    ) -> rusqlite::Result<Vec<FrameRecord>> {
        let conn = self.read_conn()?;
        let limit = limit.clamp(1, 2000) as i64;
        let mut frames = match face {
            Some(face) => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {FRAME_COLUMNS} FROM frames WHERE face = ?1 \
                     ORDER BY ts_unix_ms DESC, id DESC LIMIT ?2"
                ))?;
                let rows = stmt.query_map(rusqlite::params![face, limit], row_to_frame)?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            }
            None => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {FRAME_COLUMNS} FROM frames \
                     ORDER BY ts_unix_ms DESC, id DESC LIMIT ?1"
                ))?;
                let rows = stmt.query_map([limit], row_to_frame)?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            }
        };
        frames.reverse();
        Ok(frames)
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

    /// Frame-based activity stats with flexible time range and bucketing.
    /// Returns buckets with counts per MCP primitive category.
    pub fn frame_activity_stats(
        &self,
        hours: u32,
        bucket_ms: i64,
    ) -> rusqlite::Result<Vec<HeatmapDay>> {
        let conn = self.read_conn()?;
        let to = now_ms();
        let from = to - i64::from(hours.max(1)) * 3600 * 1000;
        let mut stmt = conn.prepare(
            "SELECT (ts_unix_ms / ?1) * ?1 AS bucket, \
             SUM(CASE WHEN kind = 'request' AND method LIKE 'tools/%' THEN 1 ELSE 0 END), \
             SUM(CASE WHEN kind = 'request' AND method LIKE 'resources/%' THEN 1 ELSE 0 END), \
             SUM(CASE WHEN kind = 'request' AND method LIKE 'prompts/%' THEN 1 ELSE 0 END), \
             SUM(CASE WHEN kind = 'request' AND (method IS NULL OR \
                  (method NOT LIKE 'tools/%' AND method NOT LIKE 'resources/%' \
                   AND method NOT LIKE 'prompts/%')) THEN 1 ELSE 0 END), \
             SUM(CASE WHEN kind = 'error' THEN 1 ELSE 0 END) \
             FROM frames WHERE ts_unix_ms >= ?2 \
             GROUP BY bucket ORDER BY bucket",
        )?;
        let rows = stmt.query_map(rusqlite::params![bucket_ms, from], |row| {
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

    /// Frame activity grouped by service instance over a time range.
    ///
    /// Only the upstream face carries a service identity (`instance_id`); the
    /// gateway face records client-side frames, whose `peer` is a client name.
    /// Grouping by `instance_id` keeps same-named services in different scopes
    /// (store vs agent) separate.
    pub fn frame_activity_by_service(
        &self,
        hours: u32,
        bucket_ms: i64,
    ) -> rusqlite::Result<Vec<ServiceActivityPoint>> {
        let conn = self.read_conn()?;
        let to = now_ms();
        let from = to - i64::from(hours.max(1)) * 3600 * 1000;
        let mut stmt = conn.prepare(
            "SELECT (ts_unix_ms / ?1) * ?1 AS bucket, COALESCE(peer, '—') AS service, \
             instance_id, COUNT(*) \
             FROM frames WHERE face = 'upstream' AND ts_unix_ms >= ?2 \
             GROUP BY bucket, service, instance_id ORDER BY bucket, service",
        )?;
        let rows = stmt.query_map(rusqlite::params![bucket_ms, from], |row| {
            Ok(ServiceActivityPoint {
                bucket_unix_ms: row.get(0)?,
                service: row.get(1)?,
                instance_id: row.get(2)?,
                frame_count: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Frame activity grouped by wire method over a time range. Both faces
    /// count: every boundary crossing is a real named frame. Frames without a
    /// method (responses, error envelopes) are not part of this dimension.
    pub fn frame_activity_by_method(
        &self,
        hours: u32,
        bucket_ms: i64,
    ) -> rusqlite::Result<Vec<MethodActivityPoint>> {
        let conn = self.read_conn()?;
        let to = now_ms();
        let from = to - i64::from(hours.max(1)) * 3600 * 1000;
        let mut stmt = conn.prepare(
            "SELECT (ts_unix_ms / ?1) * ?1 AS bucket, method, COUNT(*) \
             FROM frames WHERE method IS NOT NULL AND ts_unix_ms >= ?2 \
             GROUP BY bucket, method ORDER BY bucket, method",
        )?;
        let rows = stmt.query_map(rusqlite::params![bucket_ms, from], |row| {
            Ok(MethodActivityPoint {
                bucket_unix_ms: row.get(0)?,
                method: row.get(1)?,
                frame_count: row.get(2)?,
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
pub fn record_once(
    path: &Path,
    mut record: RequestRecord,
    config: &RequestTelemetryConfig,
) -> rusqlite::Result<()> {
    if !config.enabled || !config.capture_cli {
        return Ok(());
    }
    if config.capture_payload {
        (record.args_json, record.args_truncated) =
            cap_payload(record.args_json.take(), config.max_payload_bytes);
        (record.result_json, record.result_truncated) =
            cap_payload(record.result_json.take(), config.max_payload_bytes);
    } else {
        record.args_json = None;
        record.result_json = None;
        record.args_truncated = false;
        record.result_truncated = false;
    }
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
        let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
        log.record(tool_record("claude", "fs", true));
        let mut failed = tool_record("codex", "git", false);
        failed.error_code = Some("TOOL_FAILED".to_string());
        failed.error_summary = Some("boom".to_string());
        log.record(failed);
        log.record(RequestRecord::new("management", "mcpstore_service_add"));
        // Wait for the writer to drain.
        std::thread::sleep(Duration::from_millis(300));

        let all = log
            .history(HistoryFilter {
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(all.total, 3);
        assert_eq!(all.requests.len(), 3);

        let page = log
            .history(HistoryFilter {
                limit: 1,
                offset: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.requests.len(), 1);
        assert_eq!(page.offset, 1);

        let failures = log
            .history(HistoryFilter {
                success: Some(false),
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(failures.total, 1);
        assert_eq!(failures.requests.len(), 1);
        assert_eq!(
            failures.requests[0].error_code.as_deref(),
            Some("TOOL_FAILED")
        );

        let by_caller = log
            .history(HistoryFilter {
                caller: Some("claude".to_string()),
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(by_caller.total, 1);
        assert_eq!(by_caller.requests.len(), 1);
        let id = by_caller.requests[0].id.unwrap();
        assert_eq!(
            log.get(id).unwrap().unwrap().caller.as_deref(),
            Some("claude")
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn migrates_existing_db_without_losing_rows() {
        let path = temp_path("migration");
        std::fs::remove_file(&path).ok();
        let conn = open_connection(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE requests (
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
            CREATE TABLE frames (
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
            INSERT INTO requests (request_id, started_at_unix_ms, duration_ms, kind, success)
                VALUES ('before-migration', 1, 2, 'tool', 1);
            INSERT INTO frames (session_id, ts_unix_ms, face, direction, kind, payload, size_bytes)
                VALUES ('before-migration', 1, 'gateway', 'up', 'request', '{}', 2);",
        )
        .unwrap();

        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT request_id, timing_json FROM requests",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .unwrap(),
            ("before-migration".to_string(), None)
        );
        assert_eq!(
            conn.query_row("SELECT session_id, request_id FROM frames", [], |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?
            )))
            .unwrap(),
            ("before-migration".to_string(), None)
        );
        drop(conn);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn request_and_frame_call_correlation_roundtrip() {
        let path = temp_path("call-correlation");
        std::fs::remove_file(&path).ok();
        let conn = open_connection(&path).unwrap();
        init_schema(&conn).unwrap();
        let timing = mcpstore::telemetry::CallTiming {
            request_write_ms: Some(2),
            first_byte_ms: Some(5),
            response_read_ms: Some(8),
        };
        let mut request = RequestRecord::new("tool", "echo");
        request.request_id = "gateway-call-1".to_string();
        request.timing = Some(timing.clone());
        insert_batch(&conn, &[request]).unwrap();
        let mut frame = FrameRecord::new("upstream-session", "upstream", "down");
        frame.request_id = Some("gateway-call-1".to_string());
        insert_frame_batch(&conn, &[frame]).unwrap();
        drop(conn);

        let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
        let requests = log
            .history(HistoryFilter {
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(requests.requests[0].request_id, "gateway-call-1");
        assert_eq!(requests.requests[0].timing, Some(timing));
        let frames = log.frame_history("upstream-session", 10).unwrap();
        assert_eq!(frames[0].request_id.as_deref(), Some("gateway-call-1"));
        drop(log);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn payload_cap_truncates_and_flags() {
        let max_bytes = 1024 * 1024;
        let (payload, truncated) = cap_payload(Some("x".repeat(max_bytes + 1)), max_bytes);
        assert!(truncated);
        assert_eq!(payload.unwrap().len(), max_bytes);
        let (payload, truncated) = cap_payload(Some("短".repeat(10)), max_bytes);
        assert!(!truncated);
        assert_eq!(payload.unwrap().chars().count(), 10);
    }

    #[test]
    fn disabled_request_capture_preserves_history_and_applies_record_limit() {
        let path = temp_path("disabled-request-history");
        std::fs::remove_file(&path).ok();
        let conn = open_connection(&path).unwrap();
        init_schema(&conn).unwrap();
        let records = ["r1", "r2", "r3"]
            .into_iter()
            .map(|id| {
                let mut record = tool_record("claude", "fs", true);
                record.request_id = id.to_string();
                record
            })
            .collect::<Vec<_>>();
        insert_batch(&conn, &records).unwrap();
        drop(conn);

        let mut diagnostics = DiagnosticsConfig::default();
        diagnostics.requests.enabled = false;
        diagnostics.requests.max_records = Some(2);
        let log = TelemetryLog::open(&path, &diagnostics).unwrap();
        log.record(tool_record("codex", "git", true));
        let history = log
            .history(HistoryFilter {
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(history.total, 2);
        assert_eq!(history.requests[0].request_id, "r3");
        assert_eq!(history.requests[1].request_id, "r2");
        assert_eq!(
            log.get(history.requests[1].id.unwrap())
                .unwrap()
                .unwrap()
                .request_id,
            "r2"
        );
        drop(log);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn request_source_and_payload_switches_are_enforced_at_the_sink() {
        let path = temp_path("request-source-policy");
        std::fs::remove_file(&path).ok();
        let mut diagnostics = DiagnosticsConfig::default();
        diagnostics.requests.capture_tool = false;
        diagnostics.requests.capture_payload = false;
        let log = TelemetryLog::open(&path, &diagnostics).unwrap();
        let mut tool = tool_record("claude", "fs", true);
        tool.args_json = Some("TOOL_SENTINEL".to_string());
        log.record(tool);
        let mut management = RequestRecord::new("management", "POST /services");
        management.args_json = Some("MANAGEMENT_SENTINEL".to_string());
        management.result_json = Some("MANAGEMENT_SENTINEL".to_string());
        management.args_truncated = true;
        management.result_truncated = true;
        log.record(management);
        std::thread::sleep(Duration::from_millis(300));

        let history = log
            .history(HistoryFilter {
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(history.total, 1);
        let record = &history.requests[0];
        assert_eq!(record.kind, "management");
        assert_eq!(record.args_json, None);
        assert_eq!(record.result_json, None);
        assert!(!record.args_truncated);
        assert!(!record.result_truncated);
        assert!(!serde_json::to_string(record).unwrap().contains("SENTINEL"));
        drop(log);

        diagnostics.requests.capture_tool = true;
        diagnostics.requests.capture_management = false;
        diagnostics.requests.capture_payload = true;
        diagnostics.requests.max_payload_bytes = 8;
        let log = TelemetryLog::open(&path, &diagnostics).unwrap();
        log.record(RequestRecord::new("management", "ignored"));
        let mut tool = tool_record("claude", "fs", true);
        tool.request_id = "new-tool".to_string();
        tool.args_json = Some("123456789".to_string());
        log.record(tool);
        std::thread::sleep(Duration::from_millis(300));
        let history = log
            .history(HistoryFilter {
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(history.total, 2);
        let tool = history
            .requests
            .iter()
            .find(|record| record.request_id == "new-tool")
            .unwrap();
        assert_eq!(tool.args_json.as_deref(), Some("12345678"));
        assert!(tool.args_truncated);
        drop(log);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn heatmap_stats_groups_primitives_by_utc_day() {
        let path = temp_path("heatmap");
        let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();

        let mut tool = FrameRecord::new("gw:s", "gateway", "up");
        tool.from_envelope(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call"}));
        log.record_frame(tool);

        let mut read = FrameRecord::new("gw:s", "gateway", "up");
        read.from_envelope(&serde_json::json!({"jsonrpc":"2.0","id":2,"method":"resources/read"}));
        log.record_frame(read);

        let mut error = FrameRecord::new("gw:s", "gateway", "down");
        error.from_envelope(
            &serde_json::json!({"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"boom"}}),
        );
        log.record_frame(error);

        // Upstream face frames must not count (gateway face only).
        let mut upstream = FrameRecord::new("up:s", "upstream", "up");
        upstream.from_envelope(&serde_json::json!({"jsonrpc":"2.0","id":9,"method":"tools/call"}));
        log.record_frame(upstream);

        let mut init = FrameRecord::new("gw:s", "gateway", "up");
        init.from_envelope(&serde_json::json!({"jsonrpc":"2.0","id":3,"method":"initialize"}));
        log.record_frame(init);

        let mut notification = FrameRecord::new("gw:s", "gateway", "up");
        notification.from_envelope(
            &serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        );
        log.record_frame(notification);
        std::thread::sleep(Duration::from_millis(300));

        let days = log.heatmap_stats(53 * 7).unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].tool_calls, 1);
        assert_eq!(days[0].resource_calls, 1);
        assert_eq!(days[0].prompt_calls, 0);
        assert_eq!(
            days[0].other_calls, 1,
            "initialize is an other-request; notifications are not calls"
        );
        assert_eq!(days[0].errors, 1);
        assert_eq!(days[0].bucket_unix_ms % 86_400_000, 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn daily_stats_groups_by_utc_day() {
        let path = temp_path("daily");
        let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
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
        let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
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
        assert!(stats
            .by_tool
            .iter()
            .all(|p| p.label != "mcpstore_service_add"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn retention_purges_expired_rows() {
        let path = temp_path("retention");
        {
            let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
            log.record(tool_record("claude", "fs", true));
            std::thread::sleep(Duration::from_millis(300));
        }
        // Backdate the row directly, then reopen: open() purges expired rows.
        let conn = open_connection(&path).unwrap();
        let old = now_ms()
            - (DiagnosticsConfig::default()
                .requests
                .retention_days
                .unwrap() as i64
                + 1)
                * 24
                * 3600
                * 1000;
        conn.execute("UPDATE requests SET started_at_unix_ms = ?1", [old])
            .unwrap();
        drop(conn);
        let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            log.history(HistoryFilter {
                limit: 50,
                ..Default::default()
            })
            .unwrap()
            .total,
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
            let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
            log.record_frame(frame("s-gw", "gateway", "up", Some("initialize")));
            log.record_frame(frame("s-gw", "gateway", "down", None));
            log.record_frame(frame("s-up", "upstream", "up", Some("tools/list")));
            log.record_frame(frame("s-up", "upstream", "down", None));
            std::thread::sleep(Duration::from_millis(300));

            let sessions = log.frame_sessions(None, 24, 50, 0).unwrap();
            assert_eq!(sessions.total, 2);
            assert_eq!(sessions.sessions.len(), 2);
            let gw = sessions
                .sessions
                .iter()
                .find(|s| s.face == "gateway")
                .unwrap();
            assert_eq!(gw.session_id, "s-gw");
            assert_eq!(gw.frame_count, 2);
            assert_eq!(gw.caller.as_deref(), Some("claude"));
            assert!(
                gw.request_bytes > 0,
                "up frames should contribute request_bytes"
            );
            assert!(
                gw.response_bytes > 0,
                "down frames should contribute response_bytes"
            );
            let up = log.frame_sessions(Some("upstream"), 24, 50, 0).unwrap();
            assert_eq!(up.total, 1);
            assert_eq!(up.sessions.len(), 1);
            assert_eq!(up.sessions[0].instance_id.as_deref(), Some("inst-1"));
            assert!(up.sessions[0].request_bytes > 0);
            assert!(up.sessions[0].response_bytes > 0);
            let page = log.frame_sessions(None, 24, 1, 0).unwrap();
            assert_eq!(page.total, 2);
            assert_eq!(page.sessions.len(), 1);

            let recent = log.frame_recent(None, 10).unwrap();
            assert_eq!(recent.len(), 4);
            assert_eq!(recent[0].session_id, "s-gw");
            assert_eq!(recent[3].session_id, "s-up");
            let recent_up = log.frame_recent(Some("upstream"), 10).unwrap();
            assert_eq!(recent_up.len(), 2);

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
            let old = now_ms()
                - (DiagnosticsConfig::default().frames.retention_days.unwrap() as i64 + 1)
                    * 24
                    * 3600
                    * 1000;
            conn.execute("UPDATE frames SET ts_unix_ms = ?1", [old])
                .unwrap();
            drop(conn);
        }
        let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(log.frame_sessions(None, 24, 50, 0).unwrap().total, 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn frame_activity_by_service_scopes_by_instance() {
        let path = temp_path("frame-svc");
        let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
        let mut a = frame("s1", "upstream", "up", Some("tools/call"));
        a.instance_id = Some("store-demo".to_string());
        let mut b = frame("s2", "upstream", "up", Some("tools/call"));
        b.instance_id = Some("agent-demo".to_string());
        b.peer = Some("peer-y".to_string());
        log.record_frame(a);
        log.record_frame(b);
        // Gateway face frames carry no service identity — excluded.
        log.record_frame(frame("s3", "gateway", "up", Some("tools/call")));
        std::thread::sleep(Duration::from_millis(300));

        let points = log.frame_activity_by_service(24, 3_600_000).unwrap();
        assert_eq!(
            points.len(),
            2,
            "same-named peers in different scopes stay separate"
        );
        let ids: Vec<_> = points.iter().map(|p| p.instance_id.as_deref()).collect();
        assert!(ids.contains(&Some("store-demo")));
        assert!(ids.contains(&Some("agent-demo")));
        assert!(points.iter().all(|p| p.frame_count == 1));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn frame_activity_by_method_names_every_frame() {
        let path = temp_path("frame-method");
        let log = TelemetryLog::open(&path, &DiagnosticsConfig::default()).unwrap();
        log.record_frame(frame("s1", "upstream", "up", Some("ping")));
        log.record_frame(frame("s1", "upstream", "up", Some("ping")));
        log.record_frame(frame("s2", "gateway", "up", Some("tools/list")));
        // Responses and error envelopes have no method — excluded.
        log.record_frame(frame("s1", "upstream", "down", None));
        std::thread::sleep(Duration::from_millis(300));

        let points = log.frame_activity_by_method(24, 3_600_000).unwrap();
        let ping = points
            .iter()
            .find(|p| p.method == "ping")
            .expect("ping keeps its real name instead of an 'other' bucket");
        assert_eq!(ping.frame_count, 2);
        assert!(points.iter().any(|p| p.method == "tools/list"));
        assert!(!points.iter().any(|p| p.method == "tools/call"));
        let _ = std::fs::remove_file(path);
    }
}
