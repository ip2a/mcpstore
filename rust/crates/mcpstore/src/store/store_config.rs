use openkeyv::StoreConfig as OpenKeyvStoreConfig;
use serde_json::{json, Value};

/// Configuration object users pass to `MCPStore::swap_store`.
///
/// Rust users implement this trait for custom store configs.
/// Built-in implementations cover Memory and Redis.
pub trait StoreConfig: Send + Sync {
    fn store_name(&self) -> &str;
    fn to_openkeyv_config(&self) -> OpenKeyvStoreConfig;
}

/// Memory store configuration. No connection parameters needed.
#[derive(Clone, Debug, Default)]
pub struct MemoryStoreConfig;

impl StoreConfig for MemoryStoreConfig {
    fn store_name(&self) -> &str {
        "memory"
    }
    fn to_openkeyv_config(&self) -> OpenKeyvStoreConfig {
        OpenKeyvStoreConfig::memory()
    }
}

/// Redis store configuration.
#[derive(Clone, Debug)]
pub struct RedisStoreConfig {
    pub url: String,
    pub namespace: Option<String>,
}

impl RedisStoreConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            namespace: None,
        }
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }
}

impl StoreConfig for RedisStoreConfig {
    fn store_name(&self) -> &str {
        "redis"
    }
    fn to_openkeyv_config(&self) -> OpenKeyvStoreConfig {
        let mut config = json!({ "url": self.url });
        if let Some(namespace) = &self.namespace {
            config["keyspace"] = Value::String(namespace.clone());
        }
        OpenKeyvStoreConfig::redis(config)
    }
}

/// Generic store config from JSON — for CLI / config-file driven usage.
#[derive(Clone, Debug)]
pub struct JsonStoreConfig {
    pub store: String,
    pub config: Value,
}

impl JsonStoreConfig {
    pub fn memory() -> Self {
        Self::new("memory", serde_json::json!({}))
    }

    pub fn redis(url: impl Into<String>) -> Self {
        Self::new("redis", serde_json::json!({"url": url.into()}))
    }

    pub fn new(store: impl Into<String>, config: Value) -> Self {
        Self {
            store: store.into(),
            config,
        }
    }
}

impl StoreConfig for JsonStoreConfig {
    fn store_name(&self) -> &str {
        &self.store
    }
    fn to_openkeyv_config(&self) -> OpenKeyvStoreConfig {
        OpenKeyvStoreConfig::new(&self.store, self.config.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_config_preserves_namespace_as_keyspace() {
        let config = RedisStoreConfig::new("redis://127.0.0.1/").with_namespace("tenant-a");
        let openkeyv = config.to_openkeyv_config();
        assert_eq!(openkeyv.config["keyspace"], "tenant-a");
    }

    #[test]
    fn redis_config_omits_keyspace_without_namespace() {
        let config = RedisStoreConfig::new("redis://127.0.0.1/");
        let openkeyv = config.to_openkeyv_config();
        assert!(openkeyv.config.get("keyspace").is_none());
    }
}
