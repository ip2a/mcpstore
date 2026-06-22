use std::sync::Arc;

use crate::cache::{
    openkeyv_memory_backend, openkeyv_redis_backend, CacheSnapshot, KvStore, MemoryStore,
    RedisStore,
};
use crate::events::Event;
use crate::store::prelude::*;

const ENTITY_TYPES: &[&str] = &["agents", "clients", "services", "store", "tools"];
const RELATION_TYPES: &[&str] = &["agent_services", "service_tools"];
const STATE_TYPES: &[&str] = &["service_status", "service_metadata"];

impl MCPStore {
    pub(crate) fn build_backend(
        cache_storage: &CacheStorage,
        redis_url: &str,
        namespace: &str,
    ) -> Result<Arc<dyn KvStore>> {
        match cache_storage {
            CacheStorage::Redis => Ok(Arc::new(RedisStore::with_namespace(redis_url, namespace)?)),
            CacheStorage::Memory => Ok(Arc::new(MemoryStore::new())),
            CacheStorage::OpenKeyvMemory => Ok(openkeyv_memory_backend()),
            CacheStorage::OpenKeyvRedis => Ok(openkeyv_redis_backend(redis_url)),
        }
    }

    pub async fn current_cache_storage(&self) -> CacheStorage {
        self.cache_storage.read().await.clone()
    }

    pub async fn current_backend(&self) -> BackendKind {
        self.current_cache_storage().await
    }

    pub async fn switch_cache_storage(
        &self,
        cache_storage: CacheStorage,
        redis_url: Option<String>,
        namespace: Option<String>,
    ) -> Result<CacheSnapshot> {
        let resolved_redis_url = match redis_url {
            Some(url) => url,
            None => self
                .redis_url
                .read()
                .await
                .clone()
                .unwrap_or_else(|| "redis://127.0.0.1/".to_string()),
        };
        let resolved_namespace = namespace.unwrap_or_else(|| self.namespace());
        let cache_store =
            Self::build_backend(&cache_storage, &resolved_redis_url, &resolved_namespace)?;
        let snapshot = self
            .cache
            .replace_store_with_snapshot_and_namespace(cache_store, resolved_namespace.clone())
            .await?;

        *self.cache_storage.write().await = cache_storage;
        *self.redis_url.write().await = Some(resolved_redis_url);
        *self
            .namespace
            .write()
            .expect("store namespace lock poisoned") = resolved_namespace;
        Ok(snapshot)
    }

    pub async fn switch_backend(
        &self,
        backend_kind: BackendKind,
        redis_url: Option<String>,
        namespace: Option<String>,
    ) -> Result<CacheSnapshot> {
        self.switch_cache_storage(backend_kind, redis_url, namespace)
            .await
    }

    pub async fn publish_event(
        &self,
        event_type: &str,
        payload: serde_json::Value,
        wait: bool,
    ) -> Result<()> {
        self.event_bus
            .publish(Event::new(event_type, payload), wait)
            .await;
        Ok(())
    }

    pub async fn event_history(&self, count: usize) -> Vec<Event> {
        self.event_bus.get_history(count).await
    }

    pub async fn event_capability_report(&self) -> serde_json::Value {
        let report = self.event_capability_report_entry().await;
        serde_json::json!({
            "event_bus": report.event_bus,
            "history": report.history,
            "history_capacity": report.history_capacity,
            "cache_event_layer": report.cache_event_layer,
        })
    }

    pub async fn event_capability_report_entry(&self) -> EventCapabilityReport {
        EventCapabilityReport {
            event_bus: true,
            history: true,
            history_capacity: 1000,
            cache_event_layer: true,
        }
    }

    pub async fn cache_health_check(&self) -> Result<serde_json::Value> {
        let report = self.cache_health_report().await?;
        Ok(serde_json::json!({
            "namespace": report.namespace,
            "backend": report.backend,
            "entities": report.entities,
            "relations": report.relations,
            "states": report.states,
            "events": report.events,
        }))
    }

    pub async fn cache_health_report(&self) -> Result<CacheHealthReport> {
        let namespace = self.namespace();
        let snapshot = self.cache.snapshot().await?;
        Ok(CacheHealthReport {
            namespace,
            backend: self.current_cache_storage().await.as_str().to_string(),
            entities: snapshot.entities.keys().cloned().collect(),
            relations: snapshot.relations.keys().cloned().collect(),
            states: snapshot.states.keys().cloned().collect(),
            events: snapshot.events.keys().cloned().collect(),
        })
    }

    pub async fn cache_inspect(&self) -> Result<serde_json::Value> {
        let namespace = self.namespace();
        let snapshot = self.cache.snapshot().await?;
        let mut collections = Vec::new();
        let mut entities = Vec::new();
        let mut relations = Vec::new();
        let mut states = Vec::new();
        let mut events = Vec::new();
        let mut entity_counts = serde_json::Map::new();
        let mut relation_counts = serde_json::Map::new();
        let mut state_counts = serde_json::Map::new();
        let mut event_counts = serde_json::Map::new();

        for entity_type in ENTITY_TYPES {
            let entries = snapshot
                .entities
                .get(*entity_type)
                .cloned()
                .unwrap_or_default();
            collections.push(format!("{namespace}:entity:{entity_type}"));
            entity_counts.insert((*entity_type).to_string(), serde_json::json!(entries.len()));
            for (key, value) in entries {
                entities.push(wrap_cache_item(
                    &key,
                    entity_type,
                    &format!("{namespace}:entity:{entity_type}"),
                    value,
                ));
            }
        }
        for relation_type in RELATION_TYPES {
            let entries = snapshot
                .relations
                .get(*relation_type)
                .cloned()
                .unwrap_or_default();
            collections.push(format!("{namespace}:relations:{relation_type}"));
            relation_counts.insert(
                (*relation_type).to_string(),
                serde_json::json!(entries.len()),
            );
            for (key, value) in entries {
                relations.push(wrap_cache_item(
                    &key,
                    relation_type,
                    &format!("{namespace}:relations:{relation_type}"),
                    value,
                ));
            }
        }
        for state_type in STATE_TYPES {
            let entries = snapshot
                .states
                .get(*state_type)
                .cloned()
                .unwrap_or_default();
            collections.push(format!("{namespace}:state:{state_type}"));
            state_counts.insert((*state_type).to_string(), serde_json::json!(entries.len()));
            for (key, value) in entries {
                states.push(wrap_cache_item(
                    &key,
                    state_type,
                    &format!("{namespace}:state:{state_type}"),
                    value,
                ));
            }
        }
        for (event_type, entries) in snapshot.events {
            collections.push(format!("{namespace}:event:{event_type}"));
            event_counts.insert(event_type.clone(), serde_json::json!(entries.len()));
            for (key, value) in entries {
                events.push(wrap_cache_item(
                    &key,
                    &event_type,
                    &format!("{namespace}:event:{event_type}"),
                    value,
                ));
            }
        }
        collections.sort();
        collections.dedup();

        Ok(serde_json::json!({
            "backend": self.current_cache_storage().await.as_str(),
            "namespace": namespace,
            "scope": "global",
            "counts": {
                "entities": entity_counts,
                "relations": relation_counts,
                "states": state_counts,
                "events": event_counts,
            },
            "collections": collections,
            "entities": entities,
            "relations": relations,
            "states": states,
            "events": events,
        }))
    }
}
