//! Service management within the registry.
//!
//! add / remove / update / list services.

use super::{ConnectionStatus, ServiceEntry, ServiceRegistry};

impl ServiceRegistry {
    /// Register or update a service entry and rebuild indexes.
    pub async fn register(&self, entry: ServiceEntry) {
        let mut services = self.services.write().await;
        let mut tool_index = self.tool_index.write().await;
        let mut original_index = self.original_name_index.write().await;

        // Remove old tool mappings if service already exists
        if let Some(old) = services.get(&entry.name) {
            for tool in &old.tools {
                let global_tool = Self::tool_name(&old.name, &tool.name);
                tool_index.remove(&global_tool);
            }
        }

        // Add new tool mappings
        for tool in &entry.tools {
            let global_tool = Self::tool_name(&entry.name, &tool.name);
            tool_index.insert(global_tool, entry.name.clone());
        }

        original_index.insert(entry.original_name.clone(), entry.name.clone());

        services.insert(entry.name.clone(), entry);
    }

    /// Remove a service and all its tool/agent mappings.
    pub async fn unregister(&self, name: &str) {
        let mut services = self.services.write().await;
        let mut tool_index = self.tool_index.write().await;
        let mut agent_scopes = self.agent_scopes.write().await;
        let mut original_index = self.original_name_index.write().await;

        if let Some(entry) = services.remove(name) {
            for tool in &entry.tools {
                let global_tool = Self::tool_name(name, &tool.name);
                tool_index.remove(&global_tool);
            }
            original_index.remove(&entry.original_name);

            // Remove from all agent scopes
            for scope in agent_scopes.values_mut() {
                scope.retain(|s| s != name);
            }
        }
    }

    /// List all registered services.
    pub async fn list_services(&self) -> Vec<ServiceEntry> {
        let services = self.services.read().await;
        services.values().cloned().collect()
    }

    /// Find a service by its global name.
    pub async fn find_service(&self, name: &str) -> Option<ServiceEntry> {
        let services = self.services.read().await;
        services.get(name).cloned()
    }

    /// Update service status.
    pub async fn update_status(&self, name: &str, status: ConnectionStatus) {
        let mut services = self.services.write().await;
        if let Some(entry) = services.get_mut(name) {
            entry.status = status;
        }
    }
}
