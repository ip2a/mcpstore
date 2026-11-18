//! Request-telemetry contracts. The SDK only defines the record shape and the
//! sink trait — persistence (SQLite writer, queries) lives in the host/daemon
//! layer, so SDK consumers never inherit a storage engine.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// Internal MCP `_meta` key used to correlate gateway calls with upstream frames.
pub const CALL_ID_META_KEY: &str = "io.mcpstore/call_id";

/// Measured offsets from the start of a gateway call. HTTP first-byte timing is
/// recorded when response headers become available; stdio uses the first byte of
/// the matching JSON-RPC line. No value is synthesized when a transport cannot
/// observe the boundary.
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CallTiming {
    pub request_write_ms: Option<u64>,
    pub first_byte_ms: Option<u64>,
    pub response_read_ms: Option<u64>,
}

#[derive(Debug)]
pub struct CallTrace {
    call_id: String,
    started: Instant,
    started_at_unix_ms: i64,
    timing: Mutex<CallTiming>,
}

impl CallTrace {
    pub fn register(call_id: impl Into<String>) -> Arc<Self> {
        let call_id = call_id.into();
        let trace = Arc::new(Self {
            call_id: call_id.clone(),
            started: Instant::now(),
            started_at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis() as i64)
                .unwrap_or_default(),
            timing: Mutex::new(CallTiming::default()),
        });
        call_traces()
            .lock()
            .unwrap()
            .insert(call_id, Arc::clone(&trace));
        trace
    }

    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    pub fn started_at_unix_ms(&self) -> i64 {
        self.started_at_unix_ms
    }

    pub fn timing(&self) -> CallTiming {
        self.timing.lock().unwrap().clone()
    }

    pub fn unregister(&self) {
        call_traces().lock().unwrap().remove(&self.call_id);
        jsonrpc_traces()
            .lock()
            .unwrap()
            .retain(|_, trace| trace.call_id != self.call_id);
    }

    pub fn mark_request_write(&self) {
        self.mark(|timing| &mut timing.request_write_ms);
    }

    pub fn mark_first_byte(&self) {
        self.mark(|timing| &mut timing.first_byte_ms);
    }

    pub fn mark_response_read(&self) {
        self.mark(|timing| &mut timing.response_read_ms);
    }

    fn mark(&self, field: impl FnOnce(&mut CallTiming) -> &mut Option<u64>) {
        let elapsed = self.started.elapsed().as_millis() as u64;
        let mut timing = self.timing.lock().unwrap();
        let field = field(&mut timing);
        if field.is_none() {
            *field = Some(elapsed);
        }
    }

    fn mark_response_bytes(&self, first_byte: Instant) {
        let mut timing = self.timing.lock().unwrap();
        if timing.first_byte_ms.is_none() {
            timing.first_byte_ms = Some(
                first_byte
                    .checked_duration_since(self.started)
                    .unwrap_or_default()
                    .as_millis() as u64,
            );
        }
        if timing.response_read_ms.is_none() {
            timing.response_read_ms = Some(self.started.elapsed().as_millis() as u64);
        }
    }
}

fn call_traces() -> &'static Mutex<HashMap<String, Arc<CallTrace>>> {
    static TRACES: OnceLock<Mutex<HashMap<String, Arc<CallTrace>>>> = OnceLock::new();
    TRACES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn jsonrpc_traces() -> &'static Mutex<HashMap<(String, String), Arc<CallTrace>>> {
    static TRACES: OnceLock<Mutex<HashMap<(String, String), Arc<CallTrace>>>> = OnceLock::new();
    TRACES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn trace_for_call_id(call_id: &str) -> Option<Arc<CallTrace>> {
    call_traces().lock().unwrap().get(call_id).cloned()
}

pub(crate) fn bind_jsonrpc_trace(
    instance_id: &str,
    jsonrpc_id: String,
    trace: Arc<CallTrace>,
) {
    jsonrpc_traces()
        .lock()
        .unwrap()
        .insert((instance_id.to_string(), jsonrpc_id), trace);
}

pub(crate) fn trace_for_jsonrpc(
    instance_id: &str,
    jsonrpc_id: &str,
) -> Option<Arc<CallTrace>> {
    jsonrpc_traces()
        .lock()
        .unwrap()
        .get(&(instance_id.to_string(), jsonrpc_id.to_string()))
        .cloned()
}

pub(crate) fn mark_stdio_response(
    instance_id: &str,
    jsonrpc_id: &str,
    first_byte: Instant,
) {
    let trace = jsonrpc_traces()
        .lock()
        .unwrap()
        .get(&(instance_id.to_string(), jsonrpc_id.to_string()))
        .cloned();
    if let Some(trace) = trace {
        trace.mark_response_bytes(first_byte);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub request_id: String,
    pub started_at_unix_ms: i64,
    pub duration_ms: i64,
    /// `"tool"` (business tool execution) or `"management"` (gateway management
    /// tools / app-plane mutating API / CLI command).
    pub kind: String,
    /// Gateway mount id (`/mcp/<client>`), `"panel:<id>"` for delegated control-panel
    /// executions; None = local origin (CLI/facade/library).
    pub caller: Option<String>,
    /// `"store"` or `"agent:<id>"`.
    pub scope: Option<String>,
    pub session_key: Option<String>,
    pub instance_id: Option<String>,
    pub service_name: Option<String>,
    pub tool: Option<String>,
    pub success: bool,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub args_json: Option<String>,
    pub result_json: Option<String>,
    pub args_truncated: bool,
    pub result_truncated: bool,
    pub timing: Option<CallTiming>,
}

impl RequestRecord {
    pub fn new(kind: &str, tool: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default();
        Self {
            id: None,
            request_id: uuid::Uuid::new_v4().to_string(),
            started_at_unix_ms: now,
            duration_ms: 0,
            kind: kind.to_string(),
            caller: None,
            scope: None,
            session_key: None,
            instance_id: None,
            service_name: None,
            tool: Some(tool.into()),
            success: true,
            error_code: None,
            error_summary: None,
            args_json: None,
            result_json: None,
            args_truncated: false,
            result_truncated: false,
            timing: None,
        }
    }
}

/// Sink for completed request records. Implemented by the host (daemon SQLite
/// log, tests, or any future sink); the SDK never persists anything itself.
pub trait RequestTelemetry: Send + Sync {
    fn record(&self, record: RequestRecord);
}

/// One JSON-RPC frame crossing a transport boundary, in either direction and
/// on either face. Same host/SDK split as [`RequestRecord`]: the struct has
/// zero storage dependency — persistence lives entirely in the host layer.
#[derive(Debug, Clone, Serialize)]
pub struct FrameRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub session_id: String,
    pub ts_unix_ms: i64,
    /// `"gateway"` (inbound: someone connecting to mcpstore) or `"upstream"`
    /// (outbound: mcpstore connecting to a service).
    pub face: String,
    /// `"up"` = client→server direction, `"down"` = server→client — regardless
    /// of which side mcpstore is on.
    pub direction: String,
    /// JSON-RPC `method`; None on bare responses.
    pub method: Option<String>,
    /// Stringified JSON-RPC `id`; None on notifications.
    pub jsonrpc_id: Option<String>,
    /// `"request" | "response" | "notification" | "error"`.
    pub kind: String,
    /// Raw JSON-RPC envelope (the whole wire frame).
    pub payload: String,
    pub size_bytes: i64,
    pub truncated: bool,
    /// Gateway face: mount id (`/mcp/<client>`); upstream face: None.
    pub caller: Option<String>,
    /// Upstream face: the connected service instance id; gateway face: None.
    pub instance_id: Option<String>,
    /// Peer label: gateway face = client identity; upstream face = service name.
    pub peer: Option<String>,
    /// `"stdio" | "streamable-http"`.
    pub transport: Option<String>,
    /// Stable gateway call id shared with [`RequestRecord::request_id`].
    pub request_id: Option<String>,
}

impl FrameRecord {
    pub fn new(session_id: impl Into<String>, face: &str, direction: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default();
        Self {
            id: None,
            session_id: session_id.into(),
            ts_unix_ms: now,
            face: face.to_string(),
            direction: direction.to_string(),
            method: None,
            jsonrpc_id: None,
            kind: String::new(),
            payload: String::new(),
            size_bytes: 0,
            truncated: false,
            caller: None,
            instance_id: None,
            peer: None,
            transport: None,
            request_id: None,
        }
    }

    /// Fill `method`/`jsonrpc_id`/`kind` from a raw JSON-RPC envelope.
    pub fn from_envelope(&mut self, envelope: &serde_json::Value) {
        self.method = envelope
            .get("method")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        self.jsonrpc_id = envelope.get("id").map(stringify_jsonrpc_id);
        self.kind = match (&self.method, &self.jsonrpc_id, envelope.get("error")) {
            (Some(_), Some(_), _) => "request",
            (Some(_), None, _) => "notification",
            (None, Some(_), Some(_)) => "error",
            _ => "response",
        }
        .to_string();
    }
}

fn stringify_jsonrpc_id(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Sink for recorded frames. Implemented by the host (same SQLite log as
/// [`RequestTelemetry`]); the SDK never persists anything itself.
pub trait FrameTelemetry: Send + Sync {
    fn record_frame(&self, record: FrameRecord);
}
