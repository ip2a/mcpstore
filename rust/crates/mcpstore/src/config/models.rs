//! Data models for MCP configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transport type for MCP connections.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TransportType {
    Stdio,
    Sse,
    #[serde(rename = "streamable-http")]
    StreamableHttp,
}

/// Full server configuration with all optional fields.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfigFull {
    pub url: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub transport: Option<TransportType>,
    #[serde(rename = "workingDir", default)]
    pub working_dir: Option<String>,
}
