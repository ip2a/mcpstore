use crate::cache::models::HealthStatus;
use crate::registry::ServiceEntry;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ScopedServiceEntry {
    pub service: ServiceEntry,
    pub tool_count: usize,
    pub global_name: Option<String>,
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScopedToolEntry {
    pub name: String,
    pub original_name: String,
    pub title: Option<String>,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
    pub annotations: Option<serde_json::Value>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
    pub service_name: String,
    pub global_service_name: String,
    pub service_global_name: String,
    pub global_tool_name: String,
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScopedServiceHealth {
    pub service_name: String,
    pub health_status: HealthStatus,
}
