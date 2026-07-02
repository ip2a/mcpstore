use std::collections::HashMap;
use std::time::Instant;

use crate::cache::{CacheError, CacheLayerManager, Result};

impl CacheLayerManager {
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
        let started_at = Instant::now();
        let result = self.store.read().await.put(key, value, &collection).await;
        self.record_request(started_at, None, result.is_ok());
        result
    }

    pub async fn compare_and_put_relation(
        &self,
        relation_type: &str,
        key: &str,
        expected_version: Option<u64>,
        value: serde_json::Value,
    ) -> Result<()> {
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "relation_type={relation_type}, key={key}"
            )));
        }
        let collection = self.relation_collection(relation_type);
        let started_at = Instant::now();
        let result = self
            .store
            .read()
            .await
            .compare_and_put(key, expected_version, value, &collection)
            .await;
        self.record_request(started_at, None, result.is_ok());
        result
    }

    pub async fn get_relation(
        &self,
        relation_type: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let collection = self.relation_collection(relation_type);
        let started_at = Instant::now();
        let result = self.store.read().await.get(key, &collection).await;
        let hit = result.as_ref().ok().map(|value| value.is_some());
        self.record_request(started_at, hit, result.is_ok());
        result
    }

    pub async fn delete_relation(&self, relation_type: &str, key: &str) -> Result<()> {
        let collection = self.relation_collection(relation_type);
        let started_at = Instant::now();
        let result = self.store.read().await.delete(key, &collection).await;
        self.record_request(started_at, None, result.is_ok());
        result
    }

    pub async fn get_all_relations_async(
        &self,
        relation_type: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let collection = self.relation_collection(relation_type);
        self.get_all_from_collection(&collection).await
    }
}
