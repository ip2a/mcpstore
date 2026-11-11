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

/// Shared-memory store configuration (OpenKeyv `shm` backend).
///
/// One mmap'd backing file shared by multiple local processes: one process
/// creates it (explicit, exclusive, resets contents), every other process
/// opens it. Optional layout hints default to the OpenKeyv backend defaults
/// when unset.
#[cfg(feature = "shm")]
#[derive(Clone, Debug)]
pub struct ShmStoreConfig {
    path: std::path::PathBuf,
    create: bool,
    capacity_bytes: Option<u64>,
    index_entries: Option<u32>,
    change_retention: Option<u32>,
}

#[cfg(feature = "shm")]
impl ShmStoreConfig {
    /// Initialize (or reset) the backing file. Fails while any other process
    /// still has it attached.
    pub fn create(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            create: true,
            capacity_bytes: None,
            index_entries: None,
            change_retention: None,
        }
    }

    /// Attach to an existing backing file.
    pub fn open(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            create: false,
            capacity_bytes: None,
            index_entries: None,
            change_retention: None,
        }
    }

    pub fn with_capacity_bytes(mut self, capacity_bytes: u64) -> Self {
        self.capacity_bytes = Some(capacity_bytes);
        self
    }

    pub fn with_index_entries(mut self, index_entries: u32) -> Self {
        self.index_entries = Some(index_entries);
        self
    }

    pub fn with_change_retention(mut self, change_retention: u32) -> Self {
        self.change_retention = Some(change_retention);
        self
    }
}

#[cfg(feature = "shm")]
impl StoreConfig for ShmStoreConfig {
    fn store_name(&self) -> &str {
        "shm"
    }
    fn to_openkeyv_config(&self) -> OpenKeyvStoreConfig {
        let mut config = json!({
            "path": self.path.display().to_string(),
            "mode": if self.create { "create" } else { "open" },
        });
        if let Some(capacity_bytes) = self.capacity_bytes {
            config["capacity_bytes"] = json!(capacity_bytes);
        }
        if let Some(index_entries) = self.index_entries {
            config["index_entries"] = json!(index_entries);
        }
        if let Some(change_retention) = self.change_retention {
            config["change_retention"] = json!(change_retention);
        }
        OpenKeyvStoreConfig::shm(config)
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

    #[cfg(test)]
    pub(crate) fn shared_memory() -> Self {
        Self::new("memory-test-shared", serde_json::json!({}))
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

    #[cfg(feature = "shm")]
    #[test]
    fn shm_create_serializes_mode_and_layout() {
        let config = ShmStoreConfig::create("/tmp/a.shm")
            .with_capacity_bytes(268_435_456)
            .with_index_entries(65_536)
            .with_change_retention(10_000)
            .to_openkeyv_config();
        assert_eq!(config.store, "shm");
        assert_eq!(config.config["mode"], "create");
        assert_eq!(config.config["path"], "/tmp/a.shm");
        assert_eq!(config.config["capacity_bytes"], 268_435_456);
        assert_eq!(config.config["index_entries"], 65_536);
        assert_eq!(config.config["change_retention"], 10_000);
    }

    #[cfg(feature = "shm")]
    #[test]
    fn shm_open_omits_optional_layout_keys() {
        let config = ShmStoreConfig::open("/tmp/a.shm").to_openkeyv_config();
        assert_eq!(config.store, "shm");
        assert_eq!(config.config["mode"], "open");
        assert!(config.config.get("capacity_bytes").is_none());
        assert!(config.config.get("index_entries").is_none());
        assert!(config.config.get("change_retention").is_none());
    }
}
