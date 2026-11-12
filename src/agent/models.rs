use serde::Serialize;

use crate::identity::{InstanceId, ScopeRef};
use crate::registry::ServiceInstance;
use crate::state::ServiceState;

#[derive(Debug, Clone, Serialize)]
pub struct ScopedServiceEntry {
    pub instance: ServiceInstance,
    pub tool_count: usize,
    pub state: ServiceState,
}

/// A scope-wide catalog page: entries plus how many instances were skipped
/// (disconnected, auth pending…). A skipped instance never blanks the page —
/// callers surface the count instead.
#[derive(Debug, Clone, Serialize)]
pub struct ScopedCatalogPage<T> {
    pub entries: Vec<T>,
    pub skipped_instances: usize,
}

/// Stamp the owning instance identity onto a serialized catalog entry so
/// scope-wide pages can group without a second lookup.
pub(crate) fn insert_instance_identity(
    value: &mut serde_json::Value,
    instance_id: InstanceId,
    service_name: &str,
    scope: &ScopeRef,
) {
    if let serde_json::Value::Object(object) = value {
        object.insert("instance_id".to_string(), serde_json::json!(instance_id));
        object.insert("service_name".to_string(), serde_json::json!(service_name));
        object.insert("scope".to_string(), serde_json::json!(scope));
    }
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
    pub icons: Option<serde_json::Value>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
}
