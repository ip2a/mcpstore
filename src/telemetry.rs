//! Request-telemetry contracts. The SDK only defines the record shape and the
//! sink trait — persistence (SQLite writer, queries) lives in the host/daemon
//! layer, so SDK consumers never inherit a storage engine.

use serde::Serialize;

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
