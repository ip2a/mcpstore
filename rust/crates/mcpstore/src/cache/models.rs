//! Cache data models migrated from Python core/cache/models.py
//!
//! All models use serde for serialization and include from_dict/to_dict
//! equivalents via Serialize/Deserialize.

use serde::{Deserialize, Serialize};

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
