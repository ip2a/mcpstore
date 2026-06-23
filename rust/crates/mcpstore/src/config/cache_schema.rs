use serde::{Deserialize, Serialize};
use std::fmt;

use super::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum CacheBackend {
    #[default]
    Memory,
    Redis,
    #[serde(rename = "openkeyv_memory", alias = "openkeyv-memory")]
    OpenKeyvMemory,
    #[serde(rename = "openkeyv_redis", alias = "openkeyv-redis")]
    OpenKeyvRedis,
}

impl CacheBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Redis => "redis",
            Self::OpenKeyvMemory => "openkeyv_memory",
            Self::OpenKeyvRedis => "openkeyv_redis",
        }
    }
}

impl fmt::Display for CacheBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default)]
    pub backend: CacheBackend,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis_url: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            backend: CacheBackend::Memory,
            redis_url: None,
            namespace: default_namespace(),
        }
    }
}
