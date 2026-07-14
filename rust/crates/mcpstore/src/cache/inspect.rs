use crate::store::prelude::*;

const ENTITY_TYPES: &[&str] = &[
    "agents",
    "clients",
    "service_definitions",
    "service_instances",
    "sessions",
    "store",
    "tools",
];
const RELATION_TYPES: &[&str] = &[
    "agent_instances",
    "instance_tools",
    "session_services",
    "session_tools",
];
const STATE_TYPES: &[&str] = &[
    "cache_schema",
    "instance_status",
    "instance_metadata",
    "session_status",
    "session_state",
    "session_context",
    "context_tool_visibility",
    "tool_preferences",
    "tool_transforms",
    "openapi_import_context",
    "openapi_imports",
];
const EVENT_TYPES: &[&str] = &["session_events", "openapi_imports"];

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
        for event_type in EVENT_TYPES {
            let entries = snapshot
                .events
                .get(*event_type)
                .cloned()
                .unwrap_or_default();
            collections.push(format!("{namespace}:event:{event_type}"));
            event_counts.insert((*event_type).to_string(), serde_json::json!(entries.len()));
            for (key, value) in entries {
                events.push(wrap_cache_item(
                    &key,
                    event_type,
                    &format!("{namespace}:event:{event_type}"),
                    value,
                ));
            }
        }
        for (event_type, entries) in snapshot.events {
            if EVENT_TYPES.contains(&event_type.as_str()) {
                continue;
            }
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
            "scope": "store",
            "request_metrics": self.cache.request_metrics_snapshot(),
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

    pub async fn reset_cache_request_metrics(&self) -> Result<()> {
        self.cache.reset_request_metrics();
        Ok(())
    }
}
