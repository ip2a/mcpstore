use std::sync::Arc;
use std::time::Duration;

use openkeyv::{AsyncKeyValue, MigrationOptions};

use crate::cache::live_store::LiveStore;
use crate::cache::{CacheLayerManager, CacheStore};
use crate::event_reactor::EventBackend;
use crate::store::kernel::persistence::PersistenceRouter;
use crate::store::prelude::*;
use crate::store::store_config::{JsonStoreConfig, StoreConfig};
use crate::store::swap::SwapResult;

impl PersistenceRouter {
    pub(crate) async fn migrate_and_cutover(&self, config: &dyn StoreConfig) -> Result<SwapResult> {
        let namespace = self.cache.namespace();
        let mut target_openkeyv_config = config.to_openkeyv_config();
        if matches!(config.store_name(), "redis" | "valkey") {
            target_openkeyv_config.config["keyspace"] =
                serde_json::Value::String(namespace.clone());
        }
        let target_handle = openkeyv::factory::open_store(target_openkeyv_config.clone())
            .await
            .map_err(|e| {
                Error::new(
                    FailureCode::Internal,
                    format!("failed to open target Store '{}': {e}", config.store_name()),
                )
            })?;
        if !target_handle.capabilities.enumerate_keys
            || !target_handle.capabilities.enumerate_collections
        {
            return Err(Error::new(
                FailureCode::Internal,
                format!(
                    "Store '{}' does not provide enumeration, which mcpstore requires",
                    config.store_name()
                ),
            ));
        }

        let source_name = self.store_config.read().await.store_name().to_string();
        let target_name = config.store_name().to_string();
        let target_live_store = Arc::new(LiveStore::from_handle(target_handle));
        let target_handle = target_live_store.handle();
        let target_event_backend = EventBackend::from_store(target_handle.clone());
        let mut copied = 0;
        let mut replayed = 0;
        let online = target_handle.capabilities.change_feed;

        if online {
            let source = self.open_source_event_backend().await?;
            CacheLayerManager::clear_namespace(target_live_store.as_ref(), &namespace).await?;
            let (report, mut changes) = openkeyv::copy_snapshot_with_feed(
                source.cap(),
                &target_handle,
                &MigrationOptions::default(),
            )
            .await
            .map_err(|e| Error::new(FailureCode::Internal, format!("Store migration: {e}")))?;
            copied = report.copied;

            let _barrier = self.cache.route.write().await;
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
                .map_err(|e| {
                    Error::new(FailureCode::Internal, format!("migration barrier: {e}"))
                })?;
            loop {
                let change = tokio::time::timeout(Duration::from_secs(10), changes.recv())
                    .await
                    .map_err(|_| Error::new(FailureCode::Internal, "migration ChangeFeed timeout"))?
                    .map_err(|e| {
                        Error::new(FailureCode::Internal, format!("migration ChangeFeed: {e}"))
                    })?
                    .ok_or_else(|| {
                        Error::new(FailureCode::Internal, "migration ChangeFeed ended")
                    })?;
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
                .map_err(|e| Error::new(FailureCode::Internal, format!("migration replay: {e}")))?;
                replayed += 1;
            }
        } else {
            let snapshot = self.cache.snapshot().await?;
            CacheLayerManager::clear_namespace(target_live_store.as_ref(), &namespace).await?;
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

        if !namespace.is_empty() && matches!(target_name.as_str(), "redis" | "valkey") {
            target_handle
                .put(
                    "keyspace_migration_v1",
                    openkeyv::Value::integer(1),
                    Some("__mcpstore_keyspace_meta"),
                    None,
                )
                .await
                .map_err(|e| Error::new(FailureCode::Internal, format!("migration marker: {e}")))?;
        }

        self.cache.activate_store(target_live_store).await;
        *self.store_config.write().await =
            JsonStoreConfig::new(config.store_name(), target_openkeyv_config.config);
        self.on_cutover(target_event_backend, online).await;

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

    async fn open_source_event_backend(&self) -> Result<EventBackend> {
        if let Some(source) = self.event_backend.read().await.clone() {
            return Ok(source);
        }
        let current = self.store_config.read().await;
        let handle = openkeyv::factory::open_store(current.to_openkeyv_config())
            .await
            .map_err(|e| Error::new(FailureCode::Internal, format!("source Store: {e}")))?;
        Ok(EventBackend::from_store(handle))
    }

    async fn on_cutover(&self, backend: EventBackend, online: bool) {
        *self.event_backend.write().await = online.then_some(backend);
    }
}
