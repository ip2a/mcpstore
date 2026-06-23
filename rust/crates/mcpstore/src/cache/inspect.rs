use crate::store::prelude::*;

const ENTITY_TYPES: &[&str] = &["agents", "clients", "services", "store", "tools"];
const RELATION_TYPES: &[&str] = &["agent_services", "service_tools"];
const STATE_TYPES: &[&str] = &["service_status", "service_metadata"];

impl MCPStore {
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
