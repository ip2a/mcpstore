//! MCPStore Service Registry
//!
//! Hot-path module for service registration, discovery, and tool resolution.
//! Replaces Python core/registry/ (10,905 lines) with Rust-native:
//! - HashMap + RwLock for in-memory lookups
//! - Tool resolution by name or service
//! - Scope resolution (store-wide vs agent-specific)
//!
//! P1 priority. This is the highest-traffic data structure in MCPStore.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod scope;
pub mod service;
pub mod tool;

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
    pub schema: serde_json::Value, // JSON Schema
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
    services: Arc<RwLock<HashMap<String, ServiceEntry>>>,
    /// tool_global_name -> service_global_name
    tool_index: Arc<RwLock<HashMap<String, String>>>,
    /// agent_id -> [service_global_name]
    agent_scopes: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// original_name -> service_global_name (for quick lookup by original name)
    original_name_index: Arc<RwLock<HashMap<String, String>>>,
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

    /// Build a tool global name using the naming convention.
    fn tool_name(service_name: &str, tool_original: &str) -> String {
        let prefix = format!("{service_name}_");
        if tool_original.starts_with(&prefix) {
            tool_original.to_string()
        } else {
            format!("{service_name}_{tool_original}")
        }
    }

    /// Clear all registry state.
    pub async fn clear(&self) {
        self.services.write().await.clear();
        self.tool_index.write().await.clear();
        self.agent_scopes.write().await.clear();
        self.original_name_index.write().await.clear();
    }

    /// List all agent ids that currently have scoped services.
    pub async fn list_agent_ids(&self) -> Vec<String> {
        self.agent_scopes.read().await.keys().cloned().collect()
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_lifecycle() {
        let reg = ServiceRegistry::new();

        // Register a service
        let entry = ServiceEntry {
            name: "svc1".to_string(),
            original_name: "svc1".to_string(),
            agent_id: "global_agent_store".to_string(),
            transport: "stdio".to_string(),
            url: None,
            command: Some("python tool.py".to_string()),
            status: ConnectionStatus::Connected,
            tools: vec![ToolInfo {
                name: "tool1".to_string(),
                description: "desc".to_string(),
                schema: serde_json::json!({}),
            }],
            config: serde_json::json!({}),
            added_time: 1234567890,
        };

        reg.register(entry.clone()).await;
        assert_eq!(reg.find_service("svc1").await, Some(entry.clone()));

        // Resolve tool
        let tool = reg.find_tool("svc1_tool1").await;
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name, "tool1");

        // Unregister
        reg.unregister("svc1").await;
        assert_eq!(reg.find_service("svc1").await, None);
        assert_eq!(reg.find_tool("svc1_tool1").await, None);
    }
}
