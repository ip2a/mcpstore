use std::sync::Arc;
use std::time::{Duration, Instant};

use openkeyv::{AsyncKeyValue, MigrationOptions};

use crate::cache::store::KvStore;
use crate::cache::KvLayerManager;
use crate::store::kernel::persistence::EventBackend;
use crate::store::kernel::persistence::PersistenceRouter;
use crate::store::prelude::*;
use crate::store::store_config::{JsonStoreConfig, StoreConfig};
use crate::store::swap::SwapResult;

/// Collection for the cutover barrier change. Lives outside the namespace
/// prefix, so `clear_namespace` never touches it on either side.
const BARRIER_COLLECTION: &str = "__mcpstore_migration";

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
        let target_kv_store = Arc::new(KvStore::from_handle(target_handle));
        let target_handle = target_kv_store.handle();
        let target_event_backend = EventBackend::from_store(target_handle.clone());
        let mut copied = 0;
        let mut replayed = 0;
        let online = target_handle.capabilities.change_feed;

        // Phase 1: bulk copy while writers keep hitting the source. The online
        // path keeps the feed stream from the copy point for the tail replay;
        // the offline path re-runs under the barrier below.
        let mut changes = None;
        if online {
            let source = self.open_source_event_backend().await?;
            KvLayerManager::clear_namespace(target_kv_store.as_ref(), &namespace).await?;
            let (report, stream) = openkeyv::copy_snapshot_with_feed(
                source.cap(),
                &target_handle,
                &MigrationOptions::default(),
            )
            .await
            .map_err(|e| Error::new(FailureCode::Internal, format!("Store migration: {e}")))?;
            copied = report.copied;
            changes = Some((source, stream));
        }

        // Phase 2: exclusive barrier. Every mutation site parks on the shared
        // `route` read guard, so from here until the swap completes no write
        // can land on either store — the drain, the verification and the
        // switch are atomic to the hot path. pause_ms measures this window.
        let pause_started = Instant::now();
        let _barrier = self.cache.route.write().await;
        if let Some((source, stream)) = changes.as_mut() {
            // Drain the feed up to the barrier, replaying every change into
            // the target. The barrier change itself is data too: apply it,
            // then read it back from the target — only a matching read-back
            // counts as verified.
            let barrier_key = format!("{}-{}", std::process::id(), uuid::Uuid::new_v4());
            source
                .cap()
                .put(
                    &barrier_key,
                    openkeyv::Value::utf8("cutover"),
                    Some(BARRIER_COLLECTION),
                    None,
                )
                .await
                .map_err(|e| {
                    Error::new(FailureCode::Internal, format!("migration barrier: {e}"))
                })?;
            loop {
                let change = tokio::time::timeout(Duration::from_secs(10), stream.recv())
                    .await
                    .map_err(|_| Error::new(FailureCode::Internal, "migration ChangeFeed timeout"))?
                    .map_err(|e| {
                        Error::new(FailureCode::Internal, format!("migration ChangeFeed: {e}"))
                    })?
                    .ok_or_else(|| {
                        Error::new(FailureCode::Internal, "migration ChangeFeed ended")
                    })?;
                openkeyv::apply_change(
                    source.cap(),
                    &target_handle,
                    &change,
                    &MigrationOptions::default(),
                )
                .await
                .map_err(|e| Error::new(FailureCode::Internal, format!("migration replay: {e}")))?;
                replayed += 1;
                if change.collection == BARRIER_COLLECTION && change.key == barrier_key {
                    let read_back = target_handle
                        .get(&barrier_key, Some(BARRIER_COLLECTION))
                        .await
                        .map_err(|e| {
                            Error::new(FailureCode::Internal, format!("migration read-back: {e}"))
                        })?;
                    if read_back != Some(openkeyv::Value::utf8("cutover")) {
                        return Err(Error::new(
                            FailureCode::Internal,
                            format!("migration barrier read-back mismatch: {read_back:?}"),
                        ));
                    }
                    break;
                }
            }
            // The barrier key must not outlive the migration on either side.
            source
                .cap()
                .delete(&barrier_key, Some(BARRIER_COLLECTION))
                .await
                .map_err(|e| {
                    Error::new(FailureCode::Internal, format!("migration cleanup: {e}"))
                })?;
            target_handle
                .delete(&barrier_key, Some(BARRIER_COLLECTION))
                .await
                .map_err(|e| {
                    Error::new(FailureCode::Internal, format!("migration cleanup: {e}"))
                })?;
        } else {
            // Target without a feed: snapshot and copy under the barrier, so
            // the switch is exact by construction.
            let snapshot = self.cache.snapshot().await?;
            KvLayerManager::clear_namespace(target_kv_store.as_ref(), &namespace).await?;
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
                        target_kv_store
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

        self.cache.activate_store_unlocked(target_kv_store).await;
        let pause_ms = u64::try_from(pause_started.elapsed().as_millis()).unwrap_or(u64::MAX);
        drop(_barrier);

        // After cutover the active config must only ever attach: a later
        // reconnect replaying "create" would reset a live store other
        // processes are attached to.
        if target_name == "shm" {
            target_openkeyv_config.config["mode"] =
                serde_json::Value::String("open".to_string());
        }
        *self.store_config.write().await =
            JsonStoreConfig::new(config.store_name(), target_openkeyv_config.config);
        self.on_cutover(target_event_backend, online).await;

        Ok(SwapResult {
            source_store: source_name,
            target_store: target_name,
            copied,
            replayed,
            online,
            pause_ms,
        })
    }

    async fn open_source_event_backend(&self) -> Result<EventBackend> {
        if let Some(source) = self.event_backend.read().await.clone() {
            return Ok(source);
        }
        let current = self.store_config.read().await;
        let mut open_config = current.to_openkeyv_config();
        // A shm source is live data other processes are attached to; reopening
        // it as "create" would reset the store under everyone. Force attach.
        if current.store_name() == "shm" {
            open_config.config["mode"] = serde_json::Value::String("open".to_string());
        }
        let handle = openkeyv::factory::open_store(open_config)
            .await
            .map_err(|e| Error::new(FailureCode::Internal, format!("source Store: {e}")))?;
        Ok(EventBackend::from_store(handle))
    }

    async fn on_cutover(&self, backend: EventBackend, online: bool) {
        *self.event_backend.write().await = online.then_some(backend);
        // Wake every feed consumer: they resubscribe on the new backend and
        // drain it fully before waiting again.
        self.event_epoch.send_modify(|generation| *generation += 1);
    }
}
