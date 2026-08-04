use super::defaults::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct CacheBackend(String);

impl CacheBackend {
    pub fn memory() -> Self {
        Self("memory".into())
    }

    pub fn redis() -> Self {
        Self("redis".into())
    }

    pub fn openkeyv(backend: impl Into<String>) -> Self {
        Self(backend.into().to_ascii_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CacheBackend {
    fn default() -> Self {
        Self::memory()
    }
}

impl std::fmt::Display for CacheBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default)]
    pub backend: CacheBackend,
    #[serde(default, alias = "redis_url", skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            backend: CacheBackend::memory(),
            url: None,
            namespace: default_namespace(),
        }
    }
}
