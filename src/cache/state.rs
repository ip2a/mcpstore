use std::collections::HashMap;
use std::time::Instant;

use crate::cache::{KvError, KvLayerManager, Result};

impl KvLayerManager {
    pub async fn put_state(
        &self,
        state_type: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        self.ensure_current_schema().await?;
        if !value.is_object() {
            return Err(KvError::NotAnObject(format!(
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
                "[KV] [STATE] [PUT_VALUE] collection={collection}, key={key}, value={value}"
            );
        }
        let started_at = Instant::now();
        let (_route, store) = self.write_store().await;
        let result = store.put(key, value, &collection).await;
        self.record_request(started_at, None, result.is_ok());
        result
    }

    pub async fn compare_and_put_state(
        &self,
        state_type: &str,
        key: &str,
        expected_version: Option<u64>,
        value: serde_json::Value,
    ) -> Result<()> {
        self.ensure_current_schema().await?;
        if !value.is_object() {
            return Err(KvError::NotAnObject(format!(
                "state_type={state_type}, key={key}"
            )));
        }
        let collection = self.state_collection(state_type);
        let started_at = Instant::now();
        let (_route, store) = self.write_store().await;
        let result = store
            .compare_and_put(key, expected_version, value, &collection)
            .await;
        self.record_request(started_at, None, result.is_ok());
        result
    }

    pub async fn get_state(
        &self,
        state_type: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        self.ensure_current_schema().await?;
        let collection = self.state_collection(state_type);
        let started_at = Instant::now();
        let result = self.active_store().get(key, &collection).await;
        let hit = result.as_ref().ok().map(|value| value.is_some());
        self.record_request(started_at, hit, result.is_ok());
        let result = result?;
        let state_key = format!("{state_type}:{key}");
        let changed = self.has_state_changed(&state_key, &result).await;
        if changed {
            tracing::debug!(
                "[KV] [STATE] [GET_RESULT] collection={collection}, key={key}, result={result:?}"
            );
        }
        Ok(result)
    }

    pub async fn delete_state(&self, state_type: &str, key: &str) -> Result<()> {
        self.ensure_current_schema().await?;
        let collection = self.state_collection(state_type);
        let started_at = Instant::now();
        let (_route, store) = self.write_store().await;
        let result = store.delete(key, &collection).await;
        self.record_request(started_at, None, result.is_ok());
        result
    }

    /// Batched state read: values come back in key order, absent keys as
    /// `None`. One store round trip instead of one per key.
    pub async fn get_states_many(
        &self,
        state_type: &str,
        keys: &[String],
    ) -> Result<Vec<Option<serde_json::Value>>> {
        self.ensure_current_schema().await?;
        let collection = self.state_collection(state_type);
        let started_at = Instant::now();
        let result = self.active_store().get_many(keys, &collection).await;
        let hit = result
            .as_ref()
            .ok()
            .map(|values| values.iter().any(Option::is_some));
        self.record_request(started_at, hit, result.is_ok());
        result
    }

    pub async fn get_all_states_async(
        &self,
        state_type: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let collection = self.state_collection(state_type);
        self.get_all_from_collection(&collection).await
    }
}
