use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;

use crate::cache::layer::KV_SCHEMA_VERSION;
#[cfg(test)]
use crate::cache::KvError;
use crate::cache::{KvLayerManager, KvStore, Result};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KvSnapshot {
    pub schema_version: u32,
    pub entities: HashMap<String, HashMap<String, serde_json::Value>>,
    pub relations: HashMap<String, HashMap<String, serde_json::Value>>,
    pub states: HashMap<String, HashMap<String, serde_json::Value>>,
    pub events: HashMap<String, HashMap<String, serde_json::Value>>,
}

impl KvLayerManager {
    #[cfg(test)]
    pub(crate) async fn replace_store_with_snapshot_and_namespace(
        &self,
        store: Arc<KvStore>,
        namespace: impl Into<String>,
    ) -> Result<KvSnapshot> {
        self.ensure_current_schema().await?;
        let current_namespace = self.namespace();
        let next_namespace = namespace.into();
        let current = self.active_store();
        let snapshot = self
            .snapshot_from_store_with_namespace(current.as_ref(), &current_namespace)
            .await?;
        Self::clear_namespace(store.as_ref(), &next_namespace).await?;
        self.restore_to_store_with_namespace(store.as_ref(), &snapshot, &next_namespace)
            .await?;
        *self
            .namespace
            .write()
            .expect("kv namespace lock poisoned") = next_namespace;
        self.activate_store(store).await;
        Ok(snapshot)
    }

    pub async fn snapshot(&self) -> Result<KvSnapshot> {
        self.ensure_current_schema().await?;
        let namespace = self.namespace();
        let store = self.active_store();
        self.snapshot_from_store_with_namespace(store.as_ref(), &namespace)
            .await
    }

    async fn snapshot_from_store_with_namespace(
        &self,
        store: &KvStore,
        namespace: &str,
    ) -> Result<KvSnapshot> {
        let collections = store.collections().await?;
        Ok(KvSnapshot {
            schema_version: KV_SCHEMA_VERSION,
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

    #[cfg(test)]
    async fn restore_to_store_with_namespace(
        &self,
        store: &KvStore,
        snapshot: &KvSnapshot,
        namespace: &str,
    ) -> Result<()> {
        if snapshot.schema_version != KV_SCHEMA_VERSION {
            return Err(KvError::Validation(format!(
                "Unsupported kv snapshot schema version: expected {}, got {}",
                KV_SCHEMA_VERSION, snapshot.schema_version
            )));
        }
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
        store: &KvStore,
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

    #[cfg(test)]
    async fn restore_layer_to_store(
        &self,
        store: &KvStore,
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
