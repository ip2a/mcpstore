//! MCPStore KV Layer
//!
//! Four logical layers over one shared-openkeyv store:
//! - Entity Layer:    {namespace}:entity:{type}    -> Service/Tool/Agent data
//! - Relation Layer:  {namespace}:relations:{type}  -> Agent-Service / Service-Tool mappings
//! - State Layer:     {namespace}:state:{type}     -> Connection/Health status
//! - Event Layer:     {namespace}:event:{type}     -> Event storage
//!
//! Uses openkeyv-backed storage with serde_json::Value as the universal
//! business value type.

use arc_swap::ArcSwap;
use arc_swap::ArcSwapAny;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Arc as StdArc, RwLock as SyncRwLock};
use std::time::{Duration, Instant};
use tokio::sync::RwLock as AsyncRwLock;

use crate::kv::metrics::{KvRequestMetrics, KvRequestMetricsSnapshot};
use crate::kv::store::KvStore;

pub use crate::kv::snapshot::KvSnapshot;

// ==================== Error Type ====================

#[derive(thiserror::Error, Debug)]
pub enum KvError {
    #[error("value must be a JSON object (dict), got: {0}")]
    NotAnObject(String),
    #[error("KV store error: {0}")]
    Store(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("kv write conflict: {0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, KvError>;

pub(crate) const KV_SCHEMA_VERSION: u32 = 2;
pub(crate) const KV_SCHEMA_STATE: &str = "kv_schema";
const KV_SCHEMA_KEY: &str = "current";

// ==================== KvLayerManager ====================

#[derive(Clone)]
pub(crate) enum SchemaState {
    Unverified,
    Ready,
}

pub struct KvLayerManager {
    pub(crate) route: AsyncRwLock<()>,
    pub(crate) store: ArcSwapAny<StdArc<KvStore>>,
    pub(crate) namespace: SyncRwLock<String>,
    pub(crate) last_empty_log: AsyncRwLock<HashMap<String, Instant>>,
    pub(crate) last_state_snapshot: AsyncRwLock<HashMap<String, serde_json::Value>>,
    pub(crate) metrics: KvRequestMetrics,
    pub(crate) log_interval: Duration,
    pub(crate) schema_ready: ArcSwap<SchemaState>,
}

impl KvLayerManager {
    pub(crate) fn new(store: Arc<KvStore>, namespace: impl Into<String>) -> Self {
        Self {
            route: AsyncRwLock::new(()),
            store: ArcSwapAny::from(store),
            namespace: SyncRwLock::new(namespace.into()),
            last_empty_log: AsyncRwLock::new(HashMap::new()),
            last_state_snapshot: AsyncRwLock::new(HashMap::new()),
            metrics: KvRequestMetrics::default(),
            log_interval: Duration::from_secs(60),
            schema_ready: ArcSwap::from(Arc::new(SchemaState::Unverified)),
        }
    }

    pub fn namespace(&self) -> String {
        self.namespace
            .read()
            .expect("kv namespace lock poisoned")
            .clone()
    }

    pub(crate) fn active_store(&self) -> Arc<KvStore> {
        self.store.load_full()
    }

    /// Store access for mutating operations. The mutation sites hold the shared
    /// `route` guard across the whole operation so an online migration's
    /// exclusive barrier can fence every in-flight write and CAS while it
    /// drains, verifies and swaps; reads stay lock-free.
    pub(crate) async fn write_store(
        &self,
    ) -> (tokio::sync::RwLockReadGuard<'_, ()>, Arc<KvStore>) {
        let guard = self.route.read().await;
        let store = self.active_store();
        (guard, store)
    }

    /// Swap the active store, taking the exclusive route barrier. Only the
    /// test-only snapshot replace uses the locking form — online migration
    /// holds the barrier itself and calls `activate_store_unlocked`.
    #[cfg(test)]
    pub(crate) async fn activate_store(&self, store: Arc<KvStore>) {
        let _barrier = self.route.write().await;
        self.activate_store_unlocked(store).await;
    }

    /// Swap the active store without taking the `route` barrier. Callers that
    /// already hold the exclusive barrier (online migration) must use this —
    /// taking the lock again would deadlock.
    pub(crate) async fn activate_store_unlocked(&self, store: Arc<KvStore>) {
        self.store.store(store);
        self.schema_ready.store(Arc::new(SchemaState::Unverified));
        self.last_state_snapshot.write().await.clear();
    }

    pub fn request_metrics_snapshot(&self) -> KvRequestMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub fn reset_request_metrics(&self) {
        self.metrics.reset();
    }

    pub(in crate::kv) fn record_request(
        &self,
        started_at: Instant,
        hit: Option<bool>,
        success: bool,
    ) {
        self.metrics.record(started_at.elapsed(), hit, !success);
    }

    // ---------- Logging helpers ----------

    pub(in crate::kv) async fn log_empty_collection(&self, collection: &str) {
        let now = Instant::now();
        let mut log = self.last_empty_log.write().await;
        match log.get(collection) {
            Some(last) if now.duration_since(*last) < self.log_interval => {}
            _ => {
                log.insert(collection.to_string(), now);
                tracing::debug!(
                    "[KV] [EMPTY] Collection is empty: collection={}",
                    collection
                );
            }
        }
    }

    pub(in crate::kv) async fn get_all_from_collection(
        &self,
        collection: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        self.ensure_current_schema().await?;
        let store = self.active_store();
        let started_at = Instant::now();
        let keys = match store.keys(collection).await {
            Ok(keys) => {
                self.record_request(started_at, None, true);
                keys
            }
            Err(err) => {
                self.record_request(started_at, None, false);
                return Err(err);
            }
        };
        if keys.is_empty() {
            self.log_empty_collection(collection).await;
            return Ok(HashMap::new());
        }
        let started_at = Instant::now();
        let results = match store.get_many(&keys, collection).await {
            Ok(results) => {
                self.record_request(started_at, None, true);
                results
            }
            Err(err) => {
                self.record_request(started_at, None, false);
                return Err(err);
            }
        };
        let mut values = HashMap::with_capacity(keys.len());
        for (index, key) in keys.iter().enumerate() {
            if let Some(Some(value)) = results.get(index) {
                values.insert(key.clone(), value.clone());
            }
        }
        Ok(values)
    }

    pub(in crate::kv) async fn has_state_changed(
        &self,
        key: &str,
        value: &Option<serde_json::Value>,
    ) -> bool {
        let mut snapshot = self.last_state_snapshot.write().await;
        match snapshot.get(key) {
            Some(prev) if prev == value.as_ref().unwrap_or(&serde_json::Value::Null) => false,
            _ => {
                if let Some(v) = value {
                    snapshot.insert(key.to_string(), v.clone());
                } else {
                    snapshot.remove(key);
                }
                true
            }
        }
    }

    pub(in crate::kv) async fn ensure_current_schema(&self) -> Result<()> {
        let ready = self.schema_ready.load_full();
        if matches!(*ready, SchemaState::Ready) {
            return Ok(());
        }
        self.ensure_schema().await?;
        self.schema_ready.rcu(|current| {
            if matches!(**current, SchemaState::Ready) {
                Arc::clone(current)
            } else {
                Arc::new(SchemaState::Ready)
            }
        });
        Ok(())
    }

    async fn ensure_schema(&self) -> Result<()> {
        let namespace = self.namespace();
        let collection = Self::state_collection_with_namespace(&namespace, KV_SCHEMA_STATE);
        let store = self.active_store();
        let current = store.get(KV_SCHEMA_KEY, &collection).await?;
        let version = current
            .as_ref()
            .and_then(|value| value.get("version"))
            .and_then(serde_json::Value::as_u64);

        match version {
            None if current.is_none() => {
                let prefix = format!("{namespace}:");
                for name in store.collections().await? {
                    if name.starts_with(&prefix)
                        && name != collection
                        && !store.keys(&name).await?.is_empty()
                    {
                        return Err(KvError::Validation(
                            "kv schema marker is missing for non-empty namespace".to_string(),
                        ));
                    }
                }
                store
                    .put(
                        KV_SCHEMA_KEY,
                        serde_json::json!({ "version": KV_SCHEMA_VERSION }),
                        &collection,
                    )
                    .await?;
            }
            Some(version) if version == u64::from(KV_SCHEMA_VERSION) => {}
            _ => {
                return Err(KvError::Validation(format!(
                    "kv schema mismatch: expected {}, found {version:?}",
                    KV_SCHEMA_VERSION
                )));
            }
        }
        Ok(())
    }

    pub(crate) async fn clear_namespace(store: &KvStore, namespace: &str) -> Result<()> {
        let prefix = format!("{namespace}:");
        for collection in store.collections().await? {
            if !collection.starts_with(&prefix) {
                continue;
            }
            for key in store.keys(&collection).await? {
                store.delete(&key, &collection).await?;
            }
        }
        Ok(())
    }
}
