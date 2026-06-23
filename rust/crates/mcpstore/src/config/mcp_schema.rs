use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::service_schema::ServerConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, ServerConfig>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub agents: HashMap<String, Vec<String>>,
}
