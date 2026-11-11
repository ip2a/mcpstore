use std::collections::HashMap;
use std::time::Instant;

use crate::kv::{models, serializer, KvError, KvLayerManager, Result};

impl KvLayerManager {
    pub async fn put_entity(
        &self,
        entity_type: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        self.ensure_current_schema().await?;
        if !value.is_object() {
            return Err(KvError::NotAnObject(format!(
                "entity_type={entity_type}, key={key}"
            )));
        }
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        let started_at = Instant::now();
        let (_route, store) = self.write_store().await;
        let result = store.put(key, value, &collection).await;
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
            return Err(KvError::NotAnObject(format!(
                "entity_type={entity_type}, key={key}"
            )));
        }
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        let started_at = Instant::now();
        let (_route, store) = self.write_store().await;
        let result = store
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
        let result = self.active_store().get(key, &collection).await;
        let hit = result.as_ref().ok().map(|value| value.is_some());
        self.record_request(started_at, hit, result.is_ok());
        result
    }

    /// Batched entity read: values come back in key order, absent keys as
    /// `None`. One store round trip instead of one per key — the listing
    /// paths hydrate whole tool sets through this.
    pub async fn get_entities_many(
        &self,
        entity_type: &str,
        keys: &[String],
    ) -> Result<Vec<Option<serde_json::Value>>> {
        self.ensure_current_schema().await?;
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        let started_at = Instant::now();
        let result = self.active_store().get_many(keys, &collection).await;
        let hit = result
            .as_ref()
            .ok()
            .map(|values| values.iter().any(Option::is_some));
        self.record_request(started_at, hit, result.is_ok());
        result
    }

    pub async fn delete_entity(&self, entity_type: &str, key: &str) -> Result<()> {
        self.ensure_current_schema().await?;
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        let started_at = Instant::now();
        let (_route, store) = self.write_store().await;
        let result = store.delete(key, &collection).await;
        self.record_request(started_at, None, result.is_ok());
        result
    }

    /// Atomically take one entity: exactly one caller gets the value.
    pub async fn claim_entity(
        &self,
        entity_type: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        self.ensure_current_schema().await?;
        Self::validate_entity_type(entity_type)?;
        let collection = self.entity_collection(entity_type);
        let started_at = Instant::now();
        let (_route, store) = self.write_store().await;
        let result = store.claim(key, &collection).await;
        let hit = result.as_ref().ok().map(|value| value.is_some());
        self.record_request(started_at, hit, result.is_ok());
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
            return Err(KvError::Validation("Agent ID cannot be empty".into()));
        }
        // Reserve "all" as a system scope name
        if agent_id.eq_ignore_ascii_case("all") {
            return Err(KvError::Validation(
                "Agent ID 'all' is reserved for system use".into(),
            ));
        }
        if self.get_entity("agents", agent_id).await?.is_some() {
            return Err(KvError::Validation(format!(
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
        tracing::info!("[KV] [AGENT] Created Agent entity: agent_id={agent_id}");
        Ok(())
    }

    pub async fn get_agent(&self, agent_id: &str) -> Result<Option<serde_json::Value>> {
        if agent_id.is_empty() {
            return Err(KvError::Validation("Agent ID cannot be empty".into()));
        }
        self.get_entity("agents", agent_id).await
    }
}
