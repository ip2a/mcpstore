//! MCPStore Cache Layer
//!
//! Three-layer cache architecture migrated from Python:
//! - Entity Layer:    {namespace}:entity:{type}    -> Service/Tool/Agent data
//! - Relation Layer:  {namespace}:relations:{type}  -> Agent-Service / Service-Tool mappings
//! - State Layer:     {namespace}:state:{type}     -> Connection/Health status
//! - Event Layer:     {namespace}:event:{type}     -> Event storage
//!
//! P0 priority for Rust migration. Replaces py-key-value-aio with native Rust
//! KV abstractions, using serde_json::Value as the universal value type.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock as SyncRwLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock as AsyncRwLock;

use crate::cache::{models, serializer, CacheStore};

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
    store: AsyncRwLock<Arc<dyn CacheStore>>,
    namespace: SyncRwLock<String>,
    last_empty_log: AsyncRwLock<HashMap<String, Instant>>,
    last_state_snapshot: AsyncRwLock<HashMap<String, serde_json::Value>>,
    log_interval: Duration,
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

    fn entity_collection(&self, entity_type: &str) -> String {
        Self::entity_collection_with_namespace(&self.namespace(), entity_type)
    }

    fn entity_collection_with_namespace(namespace: &str, entity_type: &str) -> String {
        format!("{namespace}:entity:{entity_type}")
    }

    fn validate_entity_type(entity_type: &str) -> Result<()> {
        const ENTITY_TYPES: &[&str] = &["agents", "clients", "services", "store", "tools"];
        if ENTITY_TYPES.contains(&entity_type) {
            return Ok(());
        }
        Err(CacheError::Validation(Self::invalid_entity_type_message(
            entity_type,
            &ENTITY_TYPES.join(", "),
        )))
    }

    fn invalid_entity_type_message(entity_type: &str, allowed: &str) -> String {
        format!("Unknown entity_type '{entity_type}'. Allowed entity types: {allowed}")
    }

    fn relation_collection(&self, relation_type: &str) -> String {
        Self::relation_collection_with_namespace(&self.namespace(), relation_type)
    }

    fn relation_collection_with_namespace(namespace: &str, relation_type: &str) -> String {
        format!("{namespace}:relations:{relation_type}")
    }

    fn state_collection(&self, state_type: &str) -> String {
        Self::state_collection_with_namespace(&self.namespace(), state_type)
    }

    fn state_collection_with_namespace(namespace: &str, state_type: &str) -> String {
        format!("{namespace}:state:{state_type}")
    }

    fn event_collection(&self, event_type: &str) -> String {
        Self::event_collection_with_namespace(&self.namespace(), event_type)
    }

    fn event_collection_with_namespace(namespace: &str, event_type: &str) -> String {
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

    async fn log_empty_collection(&self, collection: &str) {
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

    async fn has_state_changed(&self, key: &str, value: &Option<serde_json::Value>) -> bool {
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

    // ---------- Entity layer ----------

    pub async fn put_entity(
        &self,
        entity_type: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "entity_type={entity_type}, key={key}"
            )));
        }
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        self.store.read().await.put(key, value, &collection).await
    }

    pub async fn get_entity(
        &self,
        entity_type: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        self.store.read().await.get(key, &collection).await
    }

    pub async fn delete_entity(&self, entity_type: &str, key: &str) -> Result<()> {
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        self.store.read().await.delete(key, &collection).await
    }

    pub async fn get_all_entities_async(
        &self,
        entity_type: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        let store = self.store.read().await;
        let keys = store.keys(&collection).await?;
        if keys.is_empty() {
            self.log_empty_collection(&collection).await;
            return Ok(HashMap::new());
        }
        let results = store.get_many(&keys, &collection).await?;
        let mut entities = HashMap::with_capacity(keys.len());
        for (i, key) in keys.iter().enumerate() {
            if let Some(Some(value)) = results.get(i) {
                entities.insert(key.clone(), value.clone());
            }
        }
        Ok(entities)
    }

    // get_all_entities_sync is intentionally omitted from async Rust core;
    // sync wrappers live in the PyO3 layer or caller bridges.

    // ---------- Relation layer ----------

    pub async fn put_relation(
        &self,
        relation_type: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "relation_type={relation_type}, key={key}"
            )));
        }
        let collection = self.relation_collection(relation_type);
        self.store.read().await.put(key, value, &collection).await
    }

    pub async fn get_relation(
        &self,
        relation_type: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let collection = self.relation_collection(relation_type);
        self.store.read().await.get(key, &collection).await
    }

    pub async fn delete_relation(&self, relation_type: &str, key: &str) -> Result<()> {
        let collection = self.relation_collection(relation_type);
        self.store.read().await.delete(key, &collection).await
    }

    pub async fn get_all_relations_async(
        &self,
        relation_type: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let collection = self.relation_collection(relation_type);
        let store = self.store.read().await;
        let keys = store.keys(&collection).await?;
        if keys.is_empty() {
            self.log_empty_collection(&collection).await;
            return Ok(HashMap::new());
        }
        let results = store.get_many(&keys, &collection).await?;
        let mut relations = HashMap::with_capacity(keys.len());
        for (i, key) in keys.iter().enumerate() {
            if let Some(Some(value)) = results.get(i) {
                relations.insert(key.clone(), value.clone());
            }
        }
        Ok(relations)
    }

    // ---------- State layer ----------

    pub async fn put_state(
        &self,
        state_type: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "state_type={state_type}, key={key}"
            )));
        }
        let collection = self.state_collection(state_type);
        let state_key = format!("{state_type}:{key}");
        let changed = self
            .has_state_changed(&state_key, &Some(value.clone()))
            .await;
        if changed {
            tracing::debug!(
                "[CACHE] [STATE] [PUT_VALUE] collection={collection}, key={key}, value={value}"
            );
        }
        self.store.read().await.put(key, value, &collection).await
    }

    pub async fn get_state(
        &self,
        state_type: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let collection = self.state_collection(state_type);
        let result = self.store.read().await.get(key, &collection).await?;
        let state_key = format!("{state_type}:{key}");
        let changed = self.has_state_changed(&state_key, &result).await;
        if changed {
            tracing::debug!(
                "[CACHE] [STATE] [GET_RESULT] collection={collection}, key={key}, result={result:?}"
            );
        }
        Ok(result)
    }

    pub async fn delete_state(&self, state_type: &str, key: &str) -> Result<()> {
        let collection = self.state_collection(state_type);
        self.store.read().await.delete(key, &collection).await
    }

    pub async fn get_all_states_async(
        &self,
        state_type: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let collection = self.state_collection(state_type);
        let store = self.store.read().await;
        let keys = store.keys(&collection).await?;
        if keys.is_empty() {
            self.log_empty_collection(&collection).await;
            return Ok(HashMap::new());
        }
        let results = store.get_many(&keys, &collection).await?;
        let mut states = HashMap::with_capacity(keys.len());
        for (i, key) in keys.iter().enumerate() {
            if let Some(Some(value)) = results.get(i) {
                states.insert(key.clone(), value.clone());
            }
        }
        Ok(states)
    }

    // ---------- Event layer ----------

    pub async fn put_event(
        &self,
        event_type: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "event_type={event_type}, key={key}"
            )));
        }
        let collection = self.event_collection(event_type);
        self.store.read().await.put(key, value, &collection).await
    }

    pub async fn get_event(
        &self,
        event_type: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let collection = self.event_collection(event_type);
        self.store.read().await.get(key, &collection).await
    }

    pub async fn delete_event(&self, event_type: &str, key: &str) -> Result<()> {
        let collection = self.event_collection(event_type);
        self.store.read().await.delete(key, &collection).await
    }

    pub async fn get_all_events_async(
        &self,
        event_type: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let collection = self.event_collection(event_type);
        let store = self.store.read().await;
        let keys = store.keys(&collection).await?;
        if keys.is_empty() {
            self.log_empty_collection(&collection).await;
            return Ok(HashMap::new());
        }
        let results = store.get_many(&keys, &collection).await?;
        let mut events = HashMap::with_capacity(keys.len());
        for (i, key) in keys.iter().enumerate() {
            if let Some(Some(value)) = results.get(i) {
                events.insert(key.clone(), value.clone());
            }
        }
        Ok(events)
    }

    // ---------- Agent helpers ----------

    pub async fn create_agent(
        &self,
        agent_id: &str,
        created_time: i64,
        is_global: bool,
    ) -> Result<()> {
        if agent_id.is_empty() {
            return Err(CacheError::Validation("Agent ID cannot be empty".into()));
        }
        if self.get_entity("agents", agent_id).await?.is_some() {
            return Err(CacheError::Validation(format!(
                "Agent already exists: agent_id={agent_id}"
            )));
        }
        let entity = models::AgentEntity {
            agent_id: agent_id.to_string(),
            created_time,
            last_active: created_time,
            is_global,
        };
        let value = serializer::to_value(&entity)?;
        self.put_entity("agents", agent_id, value).await?;
        tracing::info!(
            "[CACHE] [AGENT] Created Agent entity: agent_id={agent_id}, is_global={is_global}"
        );
        Ok(())
    }

    pub async fn get_agent(&self, agent_id: &str) -> Result<Option<serde_json::Value>> {
        if agent_id.is_empty() {
            return Err(CacheError::Validation("Agent ID cannot be empty".into()));
        }
        self.get_entity("agents", agent_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::memory_cache_store;

    #[tokio::test]
    async fn test_openkeyv_memory_store_basic() {
        let store = memory_cache_store();
        store
            .put("k1", serde_json::json!({"a": 1}), "c1")
            .await
            .unwrap();
        let v = store.get("k1", "c1").await.unwrap();
        assert_eq!(v, Some(serde_json::json!({"a": 1})));
    }

    #[tokio::test]
    async fn test_cache_layer_manager_entity() {
        let store = memory_cache_store();
        let mgr = CacheLayerManager::new(store, "test");

        mgr.put_entity("services", "svc1", serde_json::json!({"url": "http://x"}))
            .await
            .unwrap();
        let v = mgr.get_entity("services", "svc1").await.unwrap();
        assert_eq!(v, Some(serde_json::json!({"url": "http://x"})));
    }

    #[tokio::test]
    async fn test_cache_layer_rejects_removed_client_configs_entity() {
        let store = memory_cache_store();
        let mgr = CacheLayerManager::new(store, "test");

        let err = mgr
            .get_entity("client_configs", "client")
            .await
            .unwrap_err()
            .to_string();

        assert!(err.contains("Unknown entity_type 'client_configs'"));
    }

    #[tokio::test]
    async fn test_replace_store_with_snapshot_migrates_all_layers() {
        let first = memory_cache_store();
        let second = memory_cache_store();
        let mgr = CacheLayerManager::new(first, "test");

        mgr.put_entity("services", "svc", serde_json::json!({"name": "svc"}))
            .await
            .unwrap();
        mgr.put_relation(
            "agent_services",
            "agent",
            serde_json::json!({"services": []}),
        )
        .await
        .unwrap();
        mgr.put_state("service_status", "svc", serde_json::json!({"status": "ok"}))
            .await
            .unwrap();
        mgr.put_event(
            "service",
            "svc:added",
            serde_json::json!({"event": "added"}),
        )
        .await
        .unwrap();

        let snapshot = mgr
            .replace_store_with_snapshot_and_namespace(second, "test")
            .await
            .unwrap();
        assert_eq!(snapshot.entities["services"].len(), 1);
        assert_eq!(snapshot.relations["agent_services"].len(), 1);
        assert_eq!(snapshot.states["service_status"].len(), 1);
        assert_eq!(snapshot.events["service"].len(), 1);

        assert!(mgr.get_entity("services", "svc").await.unwrap().is_some());
        assert!(mgr
            .get_relation("agent_services", "agent")
            .await
            .unwrap()
            .is_some());
        assert!(mgr
            .get_state("service_status", "svc")
            .await
            .unwrap()
            .is_some());
        assert!(mgr
            .get_event("service", "svc:added")
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn test_create_agent() {
        let store = memory_cache_store();
        let mgr = CacheLayerManager::new(store, "test");

        mgr.create_agent("a1", 1234567890, false).await.unwrap();
        let agent = mgr.get_agent("a1").await.unwrap().unwrap();
        assert_eq!(agent["agent_id"], "a1");
        assert_eq!(agent["is_global"], false);
    }
}
