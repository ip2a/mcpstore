use std::sync::Arc;

#[cfg(feature = "redis")]
use crate::cache::redis_kv_store;
#[cfg(feature = "valkey")]
use crate::cache::valkey_kv_store;
use crate::cache::{memory_kv_store, KvStore};
use crate::store::prelude::*;
use crate::store::{JsonStoreConfig, StoreSpec};

impl MCPStore {
    pub(crate) fn build_kv_store(
        store_config: &JsonStoreConfig,
        default_store_address: &str,
        namespace: &str,
    ) -> Result<Arc<KvStore>> {
        match store_config.store_name() {
            "memory" => Ok(memory_kv_store()),
            #[cfg(feature = "redis")]
            "redis" => Ok(redis_kv_store(
                store_config
                    .config
                    .get("url")
                    .and_then(|value| value.as_str())
                    .unwrap_or(default_store_address),
                namespace,
            )),
            #[cfg(feature = "valkey")]
            "valkey" => Ok(valkey_kv_store(
                store_config
                    .config
                    .get("url")
                    .and_then(|value| value.as_str())
                    .unwrap_or(default_store_address),
                namespace,
            )),
            store => Err(Error::new(
                FailureCode::Internal,
                format!(
                "OpenKeyv Store '{store}' does not provide the capabilities required by MCPStore"
            ),
            )),
        }
    }

    pub async fn current_store_name(&self) -> String {
        self.kernel
            .persistence
            .store_config
            .read()
            .await
            .store_name()
            .to_string()
    }

    /// Stable identity for caller-side memoization keys. It distinguishes store config
    /// and namespace even when service instance IDs collide.
    pub async fn store_identity(&self) -> String {
        let config = self.kernel.persistence.store_config.read().await;
        let serialized = format!("{}:{}", config.store_name(), config.config);
        drop(config);
        blake3::hash(format!("{serialized}:{}", self.namespace()).as_bytes())
            .to_hex()
            .to_string()
    }
}
