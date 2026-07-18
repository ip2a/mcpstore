use serde::Serialize;

use crate::identity::{InstanceId, ScopeRef};
use crate::registry::ServiceInstance;

#[derive(Debug, Clone, Serialize)]
pub struct ScopedServiceEntry {
    pub instance: ServiceInstance,
    pub tool_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScopedToolEntry {
    pub name: String,
    pub tool_name: String,
    pub title: Option<String>,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
    pub annotations: Option<serde_json::Value>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
}
