use crate::store::prelude::*;

impl MCPStore {
    pub async fn cache_health_check(&self) -> Result<serde_json::Value> {
        let report = self.cache_health_report().await?;
        Ok(serde_json::json!({
            "schema_version": crate::cache::layer::CACHE_SCHEMA_VERSION,
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
}
