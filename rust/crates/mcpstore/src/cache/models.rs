//! Cache data models migrated from Python core/cache/models.py
//!
//! All models use serde for serialization and include from_dict/to_dict
//! equivalents via Serialize/Deserialize.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CacheHealthReport {
    pub namespace: String,
    pub backend: String,
    pub entities: Vec<String>,
    pub relations: Vec<String>,
    pub states: Vec<String>,
    pub events: Vec<String>,
}

// ==================== Entity Layer Models ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceEntity {
    pub service_global_name: String,
    pub service_original_name: String,
    pub source_agent: String,
    pub config: serde_json::Value,
    pub added_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolEntity {
    pub tool_global_name: String,
    pub tool_original_name: String,
    pub service_global_name: String,
    pub service_original_name: String,
    pub source_agent: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub created_time: i64,
    pub tool_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentEntity {
    pub agent_id: String,
    pub created_time: i64,
    pub last_active: i64,
    #[serde(default)]
    pub is_global: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEntity {
    pub session_key: String,
    pub session_id: String,
    pub scope: SessionScope,
    pub agent_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_active: i64,
    pub lease_seconds: Option<i64>,
    pub expires_at: Option<i64>,
    pub version: u64,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionScope {
    Store,
    Agent,
}

// ==================== Relationship Layer Models ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceRelationItem {
    pub service_original_name: String,
    pub service_global_name: String,
    pub client_id: String,
    pub established_time: i64,
    pub last_access: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AgentServiceRelation {
    pub services: Vec<ServiceRelationItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolRelationItem {
    pub tool_global_name: String,
    pub tool_original_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceToolRelation {
    pub service_global_name: String,
    pub service_original_name: String,
    pub source_agent: String,
    #[serde(default)]
    pub tools: Vec<ToolRelationItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionServiceRelation {
    pub session_key: String,
    #[serde(default)]
    pub services: Vec<SessionServiceItem>,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionServiceItem {
    pub service_global_name: String,
    pub service_original_name: String,
    pub source_agent: String,
    pub bound_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionToolVisibility {
    pub session_key: String,
    pub mode: ToolVisibilityMode,
    #[serde(default)]
    pub tools: Vec<SessionToolItem>,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionToolItem {
    pub service_global_name: String,
    pub tool_global_name: String,
    pub tool_original_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextToolVisibilityState {
    pub context_key: String,
    pub service_global_name: String,
    pub mode: ToolVisibilityMode,
    #[serde(default)]
    pub tools: Vec<SessionToolItem>,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolVisibilityMode {
    Allowlist,
}

// ==================== State Layer Models ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolStatusItem {
    pub tool_global_name: String,
    pub tool_original_name: String,
    pub status: ToolAvailability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceStatus {
    pub service_global_name: String,
    pub health_status: HealthStatus,
    pub last_health_check: i64,
    pub connection_attempts: i32,
    pub max_connection_attempts: i32,
    pub current_error: Option<String>,
    #[serde(default)]
    pub tools: Vec<ToolStatusItem>,
    pub window_error_rate: Option<f64>,
    pub latency_p95: Option<f64>,
    pub latency_p99: Option<f64>,
    pub sample_size: Option<i32>,
    pub next_retry_time: Option<f64>,
    pub hard_deadline: Option<f64>,
    pub lease_deadline: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Init,
    Startup,
    Ready,
    Healthy,
    Degraded,
    CircuitOpen,
    HalfOpen,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionStatusState {
    pub session_key: String,
    pub status: SessionStatus,
    pub updated_at: i64,
    pub version: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionStateData {
    pub session_key: String,
    #[serde(default)]
    pub values: serde_json::Map<String, serde_json::Value>,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionContextState {
    pub context_key: String,
    pub active_session_key: Option<String>,
    pub auto_session_key: Option<String>,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolTransformRule {
    pub tool_global_name: String,
    pub service_global_name: String,
    pub original_tool_name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub arguments: Vec<ToolArgumentTransform>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub enabled: bool,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolArgumentTransform {
    pub original_name: String,
    pub new_name: Option<String>,
    pub hidden: bool,
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Closed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEvent {
    pub session_key: String,
    pub event_type: SessionEventType,
    pub occurred_at: i64,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventType {
    Create,
    BindService,
    UnbindService,
    SetToolVisibility,
    SetState,
    DeleteState,
    ClearState,
    UpdateMetadata,
    Extend,
    Close,
    Expire,
    CallDenied,
}
