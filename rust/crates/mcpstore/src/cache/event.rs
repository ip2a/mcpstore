use std::collections::HashMap;

use crate::cache::{CacheError, CacheLayerManager, Result};

impl CacheLayerManager {
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
        self.get_all_from_collection(&collection).await
    }
}
