use std::collections::HashMap;
use std::sync::Arc;

use crate::cache::{CacheLayerManager, CacheStore, Result};

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheSnapshot {
    pub entities: HashMap<String, HashMap<String, serde_json::Value>>,
    pub relations: HashMap<String, HashMap<String, serde_json::Value>>,
    pub states: HashMap<String, HashMap<String, serde_json::Value>>,
    pub events: HashMap<String, HashMap<String, serde_json::Value>>,
}

impl CacheLayerManager {
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
}
