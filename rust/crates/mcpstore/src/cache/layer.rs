//! MCPStore Cache Layer
//!
//! Three-layer cache architecture migrated from Python:
//! - Entity Layer:    {namespace}:entity:{type}    -> Service/Tool/Agent data
//! - Relation Layer:  {namespace}:relations:{type}  -> Agent-Service / Service-Tool mappings
//! - State Layer:     {namespace}:state:{type}     -> Connection/Health status
//! - Event Layer:     {namespace}:event:{type}     -> Event storage
//!
//! P0 priority for Rust migration. Uses openkeyv-backed storage with
//! serde_json::Value as the universal business value type.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock as SyncRwLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock as AsyncRwLock;

use crate::cache::metrics::{CacheRequestMetrics, CacheRequestMetricsSnapshot};
use crate::cache::CacheStore;

pub use crate::cache::snapshot::CacheSnapshot;

// ==================== Error Type ====================

#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("value must be a JSON object (dict), got: {0}")]
    NotAnObject(String),
    #[error("KV store error: {0}")]
    StoreError(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("cache write conflict: {0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, CacheError>;

// ==================== CacheLayerManager ====================

/// Central cache manager with four logical layers over a single openkeyv store.
pub struct CacheLayerManager {
    pub(in crate::cache) store: AsyncRwLock<Arc<dyn CacheStore>>,
    pub(in crate::cache) namespace: SyncRwLock<String>,
    pub(in crate::cache) last_empty_log: AsyncRwLock<HashMap<String, Instant>>,
    pub(in crate::cache) last_state_snapshot: AsyncRwLock<HashMap<String, serde_json::Value>>,
    pub(in crate::cache) metrics: CacheRequestMetrics,
    pub(in crate::cache) log_interval: Duration,
}

impl CacheLayerManager {
    pub(crate) fn new(store: Arc<dyn CacheStore>, namespace: impl Into<String>) -> Self {
        Self {
            store: AsyncRwLock::new(store),
            namespace: SyncRwLock::new(namespace.into()),
            last_empty_log: AsyncRwLock::new(HashMap::new()),
            last_state_snapshot: AsyncRwLock::new(HashMap::new()),
            metrics: CacheRequestMetrics::default(),
            log_interval: Duration::from_secs(60),
        }
    }

    pub fn namespace(&self) -> String {
        self.namespace
            .read()
            .expect("cache namespace lock poisoned")
            .clone()
    }

    pub fn request_metrics_snapshot(&self) -> CacheRequestMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub fn reset_request_metrics(&self) {
        self.metrics.reset();
    }

    pub(in crate::cache) fn record_request(
        &self,
        started_at: Instant,
        hit: Option<bool>,
        success: bool,
    ) {
        self.metrics.record(started_at.elapsed(), hit, !success);
    }

    // ---------- Logging helpers ----------

    pub(in crate::cache) async fn log_empty_collection(&self, collection: &str) {
        let now = Instant::now();
        let mut log = self.last_empty_log.write().await;
        match log.get(collection) {
            Some(last) if now.duration_since(*last) < self.log_interval => {}
            _ => {
                log.insert(collection.to_string(), now);
                tracing::debug!(
                    "[CACHE] [EMPTY] Collection is empty: collection={}",
                    collection
                );
            }
        }
    }

    pub(in crate::cache) async fn get_all_from_collection(
        &self,
        collection: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let store = self.store.read().await;
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

    pub(in crate::cache) async fn has_state_changed(
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
}
