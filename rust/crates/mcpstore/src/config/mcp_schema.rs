use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::service_schema::ServerConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, ServerConfig>,
}

impl McpConfig {
    pub fn validate_structure(&self) -> Result<(), String> {
        for (service_name, config) in &self.mcp_servers {
            if service_name.trim().is_empty() {
                return Err("mcpServers contains an empty service name".to_string());
            }
            config
                .validate_structure()
                .map_err(|error| format!("service '{service_name}': {error}"))?;
        }
        Ok(())
    }

    pub fn agent_ids(&self) -> Vec<String> {
        let mut agent_ids = self
            .mcp_servers
            .values()
            .flat_map(|config| config.scopes().agents.into_keys())
            .collect::<Vec<_>>();
        agent_ids.sort();
        agent_ids.dedup();
        agent_ids
    }

    pub fn services_for_agent(&self, agent_id: &str) -> Vec<String> {
        let mut services = self
            .mcp_servers
            .iter()
            .filter(|(_, config)| config.scopes().agents.contains_key(agent_id))
            .map(|(service_name, _)| service_name.clone())
            .collect::<Vec<_>>();
        services.sort();
        services
    }
}
