use serde::{Deserialize, Serialize};

use crate::config::{ScopeDeclarations, ServiceLifecycleConfig};
use crate::identity::{InstanceId, ScopeRef};
use crate::registry::{ConfigRevision, ServiceDefinition};

#[derive(Debug, Clone)]
pub struct CacheHealthReport {
    pub namespace: String,
    pub backend: String,
    pub entities: Vec<String>,
    pub relations: Vec<String>,
    pub states: Vec<String>,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceDefinitionEntity {
    pub service_name: String,
    pub base_config: serde_json::Map<String, serde_json::Value>,
    pub scopes: ScopeDeclarations,
    pub lifecycle: Option<ServiceLifecycleConfig>,
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub base_revision: u64,
    pub added_time: i64,
}

impl From<&ServiceDefinition> for ServiceDefinitionEntity {
    fn from(definition: &ServiceDefinition) -> Self {
        Self {
            service_name: definition.service_name.clone(),
            base_config: definition.base_config.clone(),
            scopes: definition.scopes.clone(),
            lifecycle: definition.lifecycle.clone(),
            metadata: definition.metadata.clone(),
            base_revision: definition.base_revision,
            added_time: definition.added_time,
        }
    }
}

impl From<ServiceDefinitionEntity> for ServiceDefinition {
    fn from(entity: ServiceDefinitionEntity) -> Self {
        Self {
            service_name: entity.service_name,
            base_config: entity.base_config,
            scopes: entity.scopes,
            lifecycle: entity.lifecycle,
            metadata: entity.metadata,
            base_revision: entity.base_revision,
            added_time: entity.added_time,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceInstanceEntity {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub transport: String,
    pub url: Option<String>,
    pub command: Option<String>,
    pub effective_config: serde_json::Map<String, serde_json::Value>,
    pub config_revision: ConfigRevision,
    pub applied_config_revision: Option<ConfigRevision>,
    pub added_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolEntity {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub tool_name: String,
    #[serde(default)]
    pub title: Option<String>,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub annotations: Option<serde_json::Value>,
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceRelationItem {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub established_time: i64,
    pub last_access: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AgentInstanceRelation {
    pub instances: Vec<InstanceRelationItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceToolRelation {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    #[serde(default)]
    pub tools: Vec<String>,
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
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
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
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub tool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextToolVisibilityState {
    pub context_key: String,
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub mode: ToolVisibilityMode,
    #[serde(default)]
    pub tools: Vec<SessionToolItem>,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolPreferenceState {
    pub context_key: String,
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub tool_name: String,
    #[serde(default)]
    pub preferences: serde_json::Map<String, serde_json::Value>,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenApiImportContextState {
    pub last_service_name: String,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolVisibilityMode {
    Allowlist,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolStatusItem {
    pub tool_name: String,
    pub status: ToolAvailability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceStatus {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
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
    #[serde(default)]
    pub lifecycle_state: ServiceLifecycleState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ServiceLifecycleState {
    #[serde(default)]
    pub restart_attempts: i32,
    #[serde(default)]
    pub manually_stopped: bool,
    #[serde(default)]
    pub manually_stopped_at: Option<i64>,
    #[serde(default)]
    pub manual_stop_persistent: bool,
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

#[cfg(test)]
mod tests {
    use super::ServiceDefinitionEntity;
    use crate::config::ScopeDeclarations;
    use crate::registry::ServiceDefinition;

    #[test]
    fn service_definition_entity_preserves_metadata_roundtrip() {
        let metadata = serde_json::Map::from_iter([
            ("owner".to_string(), serde_json::json!("platform")),
            (
                "extension".to_string(),
                serde_json::json!({"enabled": true, "labels": ["internal"]}),
            ),
        ]);
        let definition = ServiceDefinition {
            service_name: "inventory".to_string(),
            base_config: serde_json::Map::from_iter([(
                "command".to_string(),
                serde_json::json!("inventory-server"),
            )]),
            scopes: ScopeDeclarations::store_only(),
            lifecycle: None,
            metadata: metadata.clone(),
            base_revision: 7,
            added_time: 123,
        };

        let entity = ServiceDefinitionEntity::from(&definition);
        assert_eq!(entity.metadata, metadata);

        let restored = ServiceDefinition::from(entity);
        assert_eq!(restored, definition);
    }
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
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub tool_name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub arguments: Vec<ToolArgumentTransform>,
    #[serde(default)]
    pub safety_policy: Option<ToolTransformSafetyPolicy>,
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
    #[serde(default)]
    pub validation_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolTransformSafetyPolicy {
    #[serde(default = "ToolTransformSafetyPolicy::default_reject_dangerous_argument_names")]
    pub reject_dangerous_argument_names: bool,
    #[serde(default = "ToolTransformSafetyPolicy::default_dangerous_argument_name_patterns")]
    pub dangerous_argument_name_patterns: Vec<String>,
}

impl Default for ToolTransformSafetyPolicy {
    fn default() -> Self {
        Self {
            reject_dangerous_argument_names: Self::default_reject_dangerous_argument_names(),
            dangerous_argument_name_patterns: Self::default_dangerous_argument_name_patterns(),
        }
    }
}

impl ToolTransformSafetyPolicy {
    fn default_reject_dangerous_argument_names() -> bool {
        true
    }

    fn default_dangerous_argument_name_patterns() -> Vec<String> {
        ["__", "eval", "exec", "import", "open", "file"]
            .into_iter()
            .map(str::to_string)
            .collect()
    }
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
