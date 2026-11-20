use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolChangeServiceResult {
    pub changed: bool,
    pub changes_count: usize,
    pub added_tools: Vec<String>,
    pub removed_tools: Vec<String>,
    pub updated_tools: Vec<String>,
    pub service_name: String,
    pub client_id: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolChangeSummary {
    pub changed: bool,
    pub services: Vec<String>,
    pub trigger: String,
    pub timestamp: i64,
    pub details: serde_json::Value,
}
