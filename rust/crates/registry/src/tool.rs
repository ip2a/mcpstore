//! Tool resolution within the registry.
//!
//! Find tools by name, list tools for a service, etc.

use crate::{ServiceRegistry, ToolInfo};

impl ServiceRegistry {
    /// Find which service owns a given tool (by global tool name).
    pub async fn find_tool_owner(&self, tool_global_name: &str) -> Option<String> {
        let tool_index = self.tool_index.read().await;
        tool_index.get(tool_global_name).cloned()
    }

    /// Get tool info by global tool name.
    pub async fn find_tool(&self, tool_global_name: &str) -> Option<ToolInfo> {
        let service_name = self.find_tool_owner(tool_global_name).await?;
        let services = self.services.read().await;
        let entry = services.get(&service_name)?;
        entry
            .tools
            .iter()
            .find(|t| {
                let global = Self::tool_name(&service_name, &t.name);
                global == tool_global_name
            })
            .cloned()
    }

    /// List all tools across all services.
    pub async fn list_all_tools(&self) -> Vec<ToolInfo> {
        let services = self.services.read().await;
        services.values().flat_map(|s| s.tools.clone()).collect()
    }

    /// List tools for a specific service.
    pub async fn list_service_tools(&self, service_name: &str) -> Vec<ToolInfo> {
        let services = self.services.read().await;
        services
            .get(service_name)
            .map(|s| s.tools.clone())
            .unwrap_or_default()
    }
}
