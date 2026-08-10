use super::defaults::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default)]
    pub store: String,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            store: "memory".to_string(),
            config: serde_json::json!({}),
            namespace: default_namespace(),
        }
    }
}
