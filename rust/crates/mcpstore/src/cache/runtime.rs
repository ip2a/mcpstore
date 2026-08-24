use std::sync::Arc;

use crate::cache::{memory_cache_store, CacheStore};
#[cfg(feature = "redis")]
use crate::cache::redis_store;
use crate::store::prelude::*;
use crate::store::{JsonStoreConfig, StoreConfig};

impl MCPStore {
    pub(crate) fn build_cache_store(
        store_config: &JsonStoreConfig,
        default_store_address: &str,
        _namespace: &str,
    ) -> Result<Arc<dyn CacheStore>> {
        match store_config.store_name() {
            "memory" => Ok(memory_cache_store()),
            #[cfg(feature = "redis")]
            "redis" => Ok(redis_store(
                store_config
                    .config
                    .get("url")
                    .and_then(|value| value.as_str())
                    .unwrap_or(default_store_address),
            )),
            store => Err(StoreError::Other(format!(
                "OpenKeyv Store '{store}' does not provide the capabilities required by MCPStore"
            ))),
        }
    }

    pub async fn current_store_name(&self) -> String {
        self.store_config.read().await.store_name().to_string()
    }
}
