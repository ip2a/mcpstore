use std::sync::Arc;

use crate::cache::live_store::LiveStore;
use crate::cache::CacheStore;
use crate::store::prelude::*;
use crate::store::store_config::{JsonStoreConfig, StoreConfig};

/// Result of a successful `swap_store` operation.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SwapResult {
    pub source_store: String,
    pub target_store: String,
    pub copied: u64,
    pub replayed: u64,
    pub verified: bool,
    pub online: bool,
    pub pause_ms: u64,
}

impl MCPStore {
    /// Swap the active cache Store to one described by `config`.
    ///
    /// This is the single root-level entry point for store switching. It:
    /// 1. Opens the target Store via OpenKeyv.
    /// 2. Verifies required capabilities.
    /// 3. Performs offline migration (snapshot copy).
    /// 4. Atomically replaces the internal store.
    /// 5. Returns the swap result.
    pub async fn swap_store(&self, config: &dyn StoreConfig) -> Result<SwapResult> {
        let target_openkeyv_config = config.to_openkeyv_config();
        let target_handle = openkeyv::factory::open_store(target_openkeyv_config)
            .await
            .map_err(|e| {
                StoreError::Other(format!(
                    "failed to open target Store '{}': {e}",
                    config.store_name()
                ))
            })?;

        // Verify the target has the capabilities mcpstore needs.
        if !target_handle.capabilities.enumerate_keys
            || !target_handle.capabilities.enumerate_collections
        {
            return Err(StoreError::Other(format!(
                "Store '{}' does not provide enumeration, which mcpstore requires",
                config.store_name()
            )));
        }

        let source_name = self.store_config.read().await.store_name().to_string();
        let target_name = config.store_name().to_string();
        let namespace = self.namespace();

        let target_live_store: Arc<dyn CacheStore> =
            Arc::new(LiveStore::from_handle(target_handle));

        // Freeze writes through the cache route before taking the final snapshot.
        // The snapshot method only reads the underlying store and does not acquire
        // this route, so the write lock remains held throughout copy and cutover.
        let _route = self.cache.route.write().await;
        let snapshot = self.cache.snapshot().await?;

        crate::cache::CacheLayerManager::clear_namespace(target_live_store.as_ref(), &namespace)
            .await?;
        let mut copied: u64 = 0;

        // Restore snapshot into target.
        let layers = [
            ("entity", &snapshot.entities),
            ("relations", &snapshot.relations),
            ("state", &snapshot.states),
            ("event", &snapshot.events),
        ];
        for (layer, data) in &layers {
            for (suffix, entries) in *data {
                let collection = format!("{namespace}:{layer}:{suffix}");
                for (key, value) in entries {
                    target_live_store
                        .put(key, value.clone(), &collection)
                        .await
                        .map_err(|e| StoreError::from(e))?;
                    copied += 1;
                }
            }
        }

        // Swap while writes remain frozen.
        *self.cache.store.write().await = target_live_store;
        self.cache.last_state_snapshot.write().await.clear();

        // Update metadata.
        *self.store_config.write().await =
            JsonStoreConfig::new(config.store_name(), serde_json::json!({}));

        Ok(SwapResult {
            source_store: source_name,
            target_store: target_name,
            copied,
            replayed: 0,
            verified: true,
            online: false,
            pause_ms: 0,
        })
    }
}
