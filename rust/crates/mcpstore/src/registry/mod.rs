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

pub use models::{
    ConfigRevision, ConnectionStatus, ServiceDefinition, ServiceInstance, ServiceRegistry, ToolInfo,
};

impl ServiceRegistry {
    pub async fn clear(&self) {
        self.definitions.write().await.clear();
        self.instances.write().await.clear();
        self.instance_index.write().await.clear();
        self.agent_index.write().await.clear();
    }
}
