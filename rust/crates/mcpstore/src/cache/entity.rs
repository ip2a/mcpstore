use std::collections::HashMap;
use std::time::Instant;

use crate::cache::{models, serializer, CacheError, CacheLayerManager, Result};

impl CacheLayerManager {
    pub async fn put_entity(
        &self,
        entity_type: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        self.ensure_current_schema().await?;
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "entity_type={entity_type}, key={key}"
            )));
        }
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        let started_at = Instant::now();
        let result = self.store.read().await.put(key, value, &collection).await;
        self.record_request(started_at, None, result.is_ok());
        result
    }

    pub async fn compare_and_put_entity(
        &self,
        entity_type: &str,
        key: &str,
        expected_version: Option<u64>,
        value: serde_json::Value,
    ) -> Result<()> {
        self.ensure_current_schema().await?;
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "entity_type={entity_type}, key={key}"
            )));
        }
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
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

    pub async fn get_entity(
        &self,
        entity_type: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        self.ensure_current_schema().await?;
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        let started_at = Instant::now();
        let result = self.store.read().await.get(key, &collection).await;
        let hit = result.as_ref().ok().map(|value| value.is_some());
        self.record_request(started_at, hit, result.is_ok());
        result
    }

    pub async fn delete_entity(&self, entity_type: &str, key: &str) -> Result<()> {
        self.ensure_current_schema().await?;
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        let started_at = Instant::now();
        let result = self.store.read().await.delete(key, &collection).await;
        self.record_request(started_at, None, result.is_ok());
        result
    }

    pub async fn get_all_entities_async(
        &self,
        entity_type: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        self.get_all_from_collection(&collection).await
    }

    // get_all_entities_sync is intentionally omitted from async Rust core;
    // sync wrappers live in the PyO3 layer or caller bridges.

    pub async fn create_agent(&self, agent_id: &str, created_time: i64) -> Result<()> {
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
        };
        let value = serializer::to_value(&entity)?;
        self.put_entity("agents", agent_id, value).await?;
        tracing::info!("[CACHE] [AGENT] Created Agent entity: agent_id={agent_id}");
        Ok(())
    }

    pub async fn get_agent(&self, agent_id: &str) -> Result<Option<serde_json::Value>> {
        if agent_id.is_empty() {
            return Err(CacheError::Validation("Agent ID cannot be empty".into()));
        }
        self.get_entity("agents", agent_id).await
    }
}
