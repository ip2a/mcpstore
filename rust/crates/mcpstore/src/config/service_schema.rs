use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub url: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(rename = "workingDir", default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl ServerConfig {
    pub fn infer_transport(&self) -> &str {
        if let Some(ref transport) = self.transport {
            return transport.as_str();
        }
        if self.url.is_some() {
            "streamable-http"
        } else if self.command.is_some() {
            "stdio"
        } else {
            "unknown"
        }
    }
}
