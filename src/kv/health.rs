use crate::store::prelude::*;

/// StatsReport carries no serde derive upstream; this is the wire mapping
/// (all 12 fields, mechanically one-to-one). `None` when the handle carries
/// no stats capability or the engine query fails — callers omit the block.
pub(crate) async fn engine_stats_from_handle(
    handle: &openkeyv::StoreHandle,
) -> Option<serde_json::Value> {
    if !handle.capabilities.stats {
        return None;
    }
    let stats = handle.stats.as_ref()?.stats().await.ok()?;
    Some(serde_json::json!({
        "available": stats.available,
        "live_entries": stats.live_entries,
        "tombstones": stats.tombstones,
        "index_entries": stats.index_entries,
        "max_index_entries": stats.max_index_entries,
        "arena_high_water": stats.arena_high_water,
        "arena_capacity": stats.arena_capacity,
        "live_payload_bytes": stats.live_payload_bytes,
        "fragmentation_ratio": stats.fragmentation_ratio,
        "free_by_class": stats.free_by_class,
        "expired_total": stats.expired_total,
        "change_revision": stats.change_revision,
    }))
}

impl MCPStore {
    pub async fn kv_health_check(&self) -> Result<serde_json::Value> {
        let report = self.kv_health_report().await?;
        let mut value = serde_json::json!({
            "schema_version": crate::kv::layer::KV_SCHEMA_VERSION,
            "namespace": report.namespace,
            "store": report.store,
            "entities": report.entities,
            "relations": report.relations,
            "states": report.states,
            "events": report.events,
        });
        if let Some(engine) = self.engine_stats_json().await {
            value["engine"] = engine;
        }
        Ok(value)
    }

    /// Engine stats for `/kv/health`. `None` = the active backend does not
    /// advertise the stats capability; the field is omitted rather than
    /// zero-filled (fail-loud: no engine block means unsupported, not empty).
    async fn engine_stats_json(&self) -> Option<serde_json::Value> {
        // Take the handle at the call site — a store swap swaps the
        // generation under us.
        engine_stats_from_handle(&self.kernel.persistence.kv.active_store().handle()).await
    }

    pub async fn kv_health_report(&self) -> Result<KvHealthReport> {
        let namespace = self.namespace();
        let snapshot = self.kernel.persistence.kv.snapshot().await?;
        Ok(KvHealthReport {
            namespace,
            store: self.current_store_name().await,
            entities: snapshot.entities.keys().cloned().collect(),
            relations: snapshot.relations.keys().cloned().collect(),
            states: snapshot.states.keys().cloned().collect(),
            events: snapshot.events.keys().cloned().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Memory handles carry no stats capability by convention — the engine
    /// block stays absent (fail-loud: absent means unsupported, not empty).
    #[tokio::test]
    async fn memory_handle_has_no_engine_block() {
        let store = crate::kv::storage::memory_kv_store();
        let handle = store.handle();
        assert!(!handle.capabilities.stats);
        assert!(engine_stats_from_handle(&handle).await.is_none());
    }
}
