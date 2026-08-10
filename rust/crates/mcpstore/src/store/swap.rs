use std::sync::Arc;
use std::time::Duration;

use crate::cache::live_store::LiveStore;
use crate::cache::CacheStore;
use crate::event_reactor::EventBackend;
use crate::store::prelude::*;
use crate::store::store_config::{JsonStoreConfig, StoreConfig};
use openkeyv::{AsyncKeyValue, MigrationOptions};

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

        let target_live_store = Arc::new(LiveStore::from_handle(target_handle));
        let target_handle = target_live_store.handle();
        let target_event_backend = EventBackend::from_store(target_handle.clone());
        let mut copied = 0;
        let mut replayed = 0;
        let online = target_handle.capabilities.change_feed;

        if online {
            let source = match self.event_backend.read().await.clone() {
                Some(source) => source,
                None => {
                    let current = self.store_config.read().await;
                    let handle = openkeyv::factory::open_store(current.to_openkeyv_config())
                        .await
                        .map_err(|e| StoreError::Other(format!("source Store: {e}")))?;
                    EventBackend::from_store(handle)
                }
            };
            crate::cache::CacheLayerManager::clear_namespace(
                target_live_store.as_ref(),
                &namespace,
            )
            .await?;
            let (report, mut changes) = openkeyv::copy_snapshot_with_feed(
                source.cap(),
                &target_handle,
                &MigrationOptions::default(),
            )
            .await
            .map_err(|e| StoreError::Other(format!("Store migration: {e}")))?;
            copied = report.copied;

            let _route = self.cache.route.write().await;
            let barrier_collection = "__mcpstore_migration";
            let barrier_key = format!("{}-{}", std::process::id(), uuid::Uuid::new_v4());
            source
                .cap()
                .put(
                    &barrier_key,
                    openkeyv::Value::utf8("cutover"),
                    Some(barrier_collection),
                    None,
                )
                .await
                .map_err(|e| StoreError::Other(format!("migration barrier: {e}")))?;
            loop {
                let change = tokio::time::timeout(Duration::from_secs(10), changes.recv())
                    .await
                    .map_err(|_| StoreError::Other("migration ChangeFeed timeout".into()))?
                    .map_err(|e| StoreError::Other(format!("migration ChangeFeed: {e}")))?
                    .ok_or_else(|| StoreError::Other("migration ChangeFeed ended".into()))?;
                if change.collection == barrier_collection && change.key == barrier_key {
                    break;
                }
                openkeyv::apply_change(
                    source.cap(),
                    &target_handle,
                    &change,
                    &MigrationOptions::default(),
                )
                .await
                .map_err(|e| StoreError::Other(format!("migration replay: {e}")))?;
                replayed += 1;
            }
        } else {
            let _route = self.cache.route.write().await;
            let snapshot = self.cache.snapshot().await?;
            crate::cache::CacheLayerManager::clear_namespace(
                target_live_store.as_ref(),
                &namespace,
            )
            .await?;
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
                            .await?;
                        copied += 1;
                    }
                }
            }
        }

        // Swap while writes remain frozen.
        *self.cache.store.write().await = target_live_store;
        self.cache.last_state_snapshot.write().await.clear();

        // Update metadata.
        *self.store_config.write().await =
            JsonStoreConfig::new(config.store_name(), config.to_openkeyv_config().config);
        *self.event_backend.write().await = online.then_some(target_event_backend);

        Ok(SwapResult {
            source_store: source_name,
            target_store: target_name,
            copied,
            replayed,
            verified: true,
            online,
            pause_ms: 0,
        })
    }
}
