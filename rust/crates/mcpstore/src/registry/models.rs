use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::RwLock;

use crate::config::{ScopeDeclarations, ServiceLifecycleConfig};
use crate::identity::{InstanceId, ScopeRef, ScopeView, ServiceInstanceKey};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConfigRevision {
    pub base_revision: u64,
    pub scope_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceDefinition {
    pub service_name: String,
    pub base_config: Map<String, Value>,
    pub scopes: ScopeDeclarations,
    pub lifecycle: Option<ServiceLifecycleConfig>,
    pub handshake_mode: Option<crate::config::HandshakeMode>,
    pub base_revision: u64,
    pub metadata: Map<String, Value>,
    pub added_time: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceInstance {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub transport: String,
    pub url: Option<String>,
    pub command: Option<String>,
    pub tools: Vec<ToolInfo>,
    pub effective_config: Map<String, Value>,
    pub config_revision: ConfigRevision,
    pub applied_config_revision: Option<ConfigRevision>,
    pub added_time: i64,
}

impl ServiceInstance {
    pub fn key(&self) -> ServiceInstanceKey {
        ServiceInstanceKey::new(self.service_name.clone(), self.scope.clone())
    }

    pub fn restart_required(&self) -> bool {
        self.applied_config_revision
            .is_some_and(|applied| applied != self.config_revision)
    }
}

/// 作用域注册表条目（`list_scopes` / `scope_info` 返回）。
///
/// 把“作用域”当一等公民：每个 scope（root / store / agent）一个条目，
/// 带它在运行时 registry 里的服务数。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopeSummary {
    pub scope: ScopeView,
    pub service_count: usize,
}

/// Agent 实体（`find_agent` 返回）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub instance_ids: Vec<InstanceId>,
}

#[derive(Clone)]
pub struct ServiceRegistry {
    pub(in crate::registry) definitions: Arc<RwLock<HashMap<String, ServiceDefinition>>>,
    pub(in crate::registry) instances: Arc<RwLock<HashMap<InstanceId, ServiceInstance>>>,
    pub(in crate::registry) instance_index: Arc<RwLock<HashMap<ServiceInstanceKey, InstanceId>>>,
    pub(in crate::registry) agent_index: Arc<RwLock<HashMap<String, Vec<InstanceId>>>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
            instance_index: Arc::new(RwLock::new(HashMap::new())),
            agent_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl From<crate::transport::DiscoveredTool> for ToolInfo {
    fn from(tool: crate::transport::DiscoveredTool) -> Self {
        Self {
            name: tool.name,
            title: tool.title,
            description: tool.description,
            input_schema: tool.input_schema,
            output_schema: tool.output_schema,
            annotations: tool.annotations,
            meta: tool.meta,
        }
    }
}
