use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

/// Connection status of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

/// Tool metadata.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
}

/// Service metadata entry.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ServiceEntry {
    pub name: String,
    pub original_name: String,
    pub agent_id: String,
    pub transport: String,
    pub url: Option<String>,
    pub command: Option<String>,
    pub status: ConnectionStatus,
    pub tools: Vec<ToolInfo>,
    pub config: serde_json::Value,
    pub added_time: i64,
}

/// Global service registry with in-memory indexes.
pub struct ServiceRegistry {
    /// service_global_name -> ServiceEntry
    pub(in crate::registry) services: Arc<RwLock<HashMap<String, ServiceEntry>>>,
    /// tool_global_name -> service_global_name
    pub(in crate::registry) tool_index: Arc<RwLock<HashMap<String, String>>>,
    /// agent_id -> [service_global_name]
    pub(in crate::registry) agent_scopes: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// original_name -> service_global_name
    pub(in crate::registry) original_name_index: Arc<RwLock<HashMap<String, String>>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            tool_index: Arc::new(RwLock::new(HashMap::new())),
            agent_scopes: Arc::new(RwLock::new(HashMap::new())),
            original_name_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
