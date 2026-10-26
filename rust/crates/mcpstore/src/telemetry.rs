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
/// log, tests, or any future backend); the SDK never persists anything itself.
pub trait RequestTelemetry: Send + Sync {
    fn record(&self, record: RequestRecord);
}
