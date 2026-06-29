use std::collections::HashMap;

use crate::cache::{CacheError, CacheLayerManager, Result};

impl CacheLayerManager {
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

    pub async fn compare_and_put_state(
        &self,
        state_type: &str,
        key: &str,
        expected_version: Option<u64>,
        value: serde_json::Value,
    ) -> Result<()> {
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "state_type={state_type}, key={key}"
            )));
        }
        let collection = self.state_collection(state_type);
        self.store
            .read()
            .await
            .compare_and_put(key, expected_version, value, &collection)
            .await
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
        self.get_all_from_collection(&collection).await
    }
}
