use crate::cache::{CacheError, CacheLayerManager, Result};

impl CacheLayerManager {
    pub(in crate::cache) fn entity_collection(&self, entity_type: &str) -> String {
        Self::entity_collection_with_namespace(&self.namespace(), entity_type)
    }

    pub(in crate::cache) fn entity_collection_with_namespace(
        namespace: &str,
        entity_type: &str,
    ) -> String {
        format!("{namespace}:entity:{entity_type}")
    }

    pub(in crate::cache) fn validate_entity_type(entity_type: &str) -> Result<()> {
        const ENTITY_TYPES: &[&str] = &[
            "agents", "clients", "services", "sessions", "store", "tools",
        ];
        if ENTITY_TYPES.contains(&entity_type) {
            return Ok(());
        }
        Err(CacheError::Validation(Self::invalid_entity_type_message(
            entity_type,
            &ENTITY_TYPES.join(", "),
        )))
    }

    pub(in crate::cache) fn invalid_entity_type_message(
        entity_type: &str,
        allowed: &str,
    ) -> String {
        format!("Unknown entity_type '{entity_type}'. Allowed entity types: {allowed}")
    }

    pub(in crate::cache) fn relation_collection(&self, relation_type: &str) -> String {
        Self::relation_collection_with_namespace(&self.namespace(), relation_type)
    }

    pub(in crate::cache) fn relation_collection_with_namespace(
        namespace: &str,
        relation_type: &str,
    ) -> String {
        format!("{namespace}:relations:{relation_type}")
    }

    pub(in crate::cache) fn state_collection(&self, state_type: &str) -> String {
        Self::state_collection_with_namespace(&self.namespace(), state_type)
    }

    pub(in crate::cache) fn state_collection_with_namespace(
        namespace: &str,
        state_type: &str,
    ) -> String {
        format!("{namespace}:state:{state_type}")
    }

    pub(in crate::cache) fn event_collection(&self, event_type: &str) -> String {
        Self::event_collection_with_namespace(&self.namespace(), event_type)
    }

    pub(in crate::cache) fn event_collection_with_namespace(
        namespace: &str,
        event_type: &str,
    ) -> String {
        format!("{namespace}:event:{event_type}")
    }
}
