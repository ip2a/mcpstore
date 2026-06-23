//! MCPStore Service Registry
//!
//! Hot-path module for service registration, discovery, and tool resolution.
//! Replaces Python core/registry/ (10,905 lines) with Rust-native:
//! - HashMap + RwLock for in-memory lookups
//! - Tool resolution by name or service
//! - Scope resolution (store-wide vs agent-specific)
//!
//! P1 priority. This is the highest-traffic data structure in MCPStore.

mod models;
pub mod scope;
pub mod service;
#[cfg(test)]
mod tests;
pub mod tool;

pub use models::{ConnectionStatus, ServiceEntry, ServiceRegistry, ToolInfo};

impl ServiceRegistry {
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
