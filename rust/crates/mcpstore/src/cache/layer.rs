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

use crate::cache::CacheStore;

// ==================== Error Type ====================

#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("value must be a JSON object (dict), got: {0}")]
    NotAnObject(String),
    #[error("KV store error: {0}")]
    StoreError(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("{0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, CacheError>;

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheSnapshot {
    pub entities: HashMap<String, HashMap<String, serde_json::Value>>,
    pub relations: HashMap<String, HashMap<String, serde_json::Value>>,
    pub states: HashMap<String, HashMap<String, serde_json::Value>>,
    pub events: HashMap<String, HashMap<String, serde_json::Value>>,
}

// ==================== CacheLayerManager ====================

/// Central cache manager with four logical layers over a single KV backend.
pub struct CacheLayerManager {
    pub(in crate::cache) store: AsyncRwLock<Arc<dyn CacheStore>>,
    pub(in crate::cache) namespace: SyncRwLock<String>,
    pub(in crate::cache) last_empty_log: AsyncRwLock<HashMap<String, Instant>>,
    pub(in crate::cache) last_state_snapshot: AsyncRwLock<HashMap<String, serde_json::Value>>,
    pub(in crate::cache) log_interval: Duration,
}

impl CacheLayerManager {
    pub(crate) fn new(store: Arc<dyn CacheStore>, namespace: impl Into<String>) -> Self {
        Self {
            store: AsyncRwLock::new(store),
            namespace: SyncRwLock::new(namespace.into()),
            last_empty_log: AsyncRwLock::new(HashMap::new()),
            last_state_snapshot: AsyncRwLock::new(HashMap::new()),
            log_interval: Duration::from_secs(60),
        }
    }

    // ---------- Collection naming ----------

    pub fn namespace(&self) -> String {
        self.namespace
            .read()
            .expect("cache namespace lock poisoned")
            .clone()
    }

    pub(in crate::cache) fn entity_collection(&self, entity_type: &str) -> String {
        Self::entity_collection_with_namespace(&self.namespace(), entity_type)
    }

    pub(in crate::cache) fn entity_collection_with_namespace(
        namespace: &str,
        entity_type: &str,
    ) -> String {
        format!("{namespace}:entity:{entity_type}")
    }

    pub(in crate::cache) fn validate_entity_type(entity_type: &str) -> Result<()> {
        const ENTITY_TYPES: &[&str] = &["agents", "clients", "services", "store", "tools"];
        if ENTITY_TYPES.contains(&entity_type) {
            return Ok(());
        }
        Err(CacheError::Validation(Self::invalid_entity_type_message(
            entity_type,
            &ENTITY_TYPES.join(", "),
        )))
    }

    pub(in crate::cache) fn invalid_entity_type_message(
        entity_type: &str,
        allowed: &str,
    ) -> String {
        format!("Unknown entity_type '{entity_type}'. Allowed entity types: {allowed}")
    }

    pub(in crate::cache) fn relation_collection(&self, relation_type: &str) -> String {
        Self::relation_collection_with_namespace(&self.namespace(), relation_type)
    }

    pub(in crate::cache) fn relation_collection_with_namespace(
        namespace: &str,
        relation_type: &str,
    ) -> String {
        format!("{namespace}:relations:{relation_type}")
    }

    pub(in crate::cache) fn state_collection(&self, state_type: &str) -> String {
        Self::state_collection_with_namespace(&self.namespace(), state_type)
    }

    pub(in crate::cache) fn state_collection_with_namespace(
        namespace: &str,
        state_type: &str,
    ) -> String {
        format!("{namespace}:state:{state_type}")
    }

    pub(in crate::cache) fn event_collection(&self, event_type: &str) -> String {
        Self::event_collection_with_namespace(&self.namespace(), event_type)
    }

    pub(in crate::cache) fn event_collection_with_namespace(
        namespace: &str,
        event_type: &str,
    ) -> String {
        format!("{namespace}:event:{event_type}")
    }

    pub(crate) async fn replace_store_with_snapshot_and_namespace(
        &self,
        store: Arc<dyn CacheStore>,
        namespace: impl Into<String>,
    ) -> Result<CacheSnapshot> {
        let current_namespace = self.namespace();
        let next_namespace = namespace.into();
        let mut current = self.store.write().await;
        let snapshot = self
            .snapshot_from_store_with_namespace(current.as_ref(), &current_namespace)
            .await?;
        self.restore_to_store_with_namespace(store.as_ref(), &snapshot, &next_namespace)
            .await?;
        *current = store;
        *self
            .namespace
            .write()
            .expect("cache namespace lock poisoned") = next_namespace;
        self.last_state_snapshot.write().await.clear();
        Ok(snapshot)
    }

    pub async fn snapshot(&self) -> Result<CacheSnapshot> {
        let namespace = self.namespace();
        let store = self.store.read().await;
        self.snapshot_from_store_with_namespace(store.as_ref(), &namespace)
            .await
    }

    async fn snapshot_from_store_with_namespace(
        &self,
        store: &dyn CacheStore,
        namespace: &str,
    ) -> Result<CacheSnapshot> {
        let collections = store.collections().await?;
        Ok(CacheSnapshot {
            entities: self
                .snapshot_layer_from_store(store, &collections, namespace, "entity")
                .await?,
            relations: self
                .snapshot_layer_from_store(store, &collections, namespace, "relations")
                .await?,
            states: self
                .snapshot_layer_from_store(store, &collections, namespace, "state")
                .await?,
            events: self
                .snapshot_layer_from_store(store, &collections, namespace, "event")
                .await?,
        })
    }

    async fn restore_to_store_with_namespace(
        &self,
        store: &dyn CacheStore,
        snapshot: &CacheSnapshot,
        namespace: &str,
    ) -> Result<()> {
        self.restore_layer_to_store(store, namespace, "entity", &snapshot.entities)
            .await?;
        self.restore_layer_to_store(store, namespace, "relations", &snapshot.relations)
            .await?;
        self.restore_layer_to_store(store, namespace, "state", &snapshot.states)
            .await?;
        self.restore_layer_to_store(store, namespace, "event", &snapshot.events)
            .await?;
        Ok(())
    }

    async fn snapshot_layer_from_store(
        &self,
        store: &dyn CacheStore,
        collections: &[String],
        namespace: &str,
        layer: &str,
    ) -> Result<HashMap<String, HashMap<String, serde_json::Value>>> {
        let prefix = format!("{namespace}:{layer}:");
        let mut output = HashMap::new();
        for collection in collections {
            if !collection.starts_with(&prefix) {
                continue;
            }
            let suffix = collection[prefix.len()..].to_string();
            let keys = store.keys(collection).await?;
            let values = store.get_many(&keys, collection).await?;
            let mut entries = HashMap::with_capacity(keys.len());
            for (index, key) in keys.iter().enumerate() {
                if let Some(Some(value)) = values.get(index) {
                    entries.insert(key.clone(), value.clone());
                }
            }
            output.insert(suffix, entries);
        }
        Ok(output)
    }

    async fn restore_layer_to_store(
        &self,
        store: &dyn CacheStore,
        namespace: &str,
        layer: &str,
        data: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> Result<()> {
        for (suffix, entries) in data {
            let collection = format!("{namespace}:{layer}:{suffix}");
            for (key, value) in entries {
                store.put(key, value.clone(), &collection).await?;
            }
        }
        Ok(())
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
        let keys = store.keys(collection).await?;
        if keys.is_empty() {
            self.log_empty_collection(collection).await;
            return Ok(HashMap::new());
        }
        let results = store.get_many(&keys, collection).await?;
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
