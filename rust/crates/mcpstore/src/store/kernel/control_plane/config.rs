use crate::store::prelude::*;
use crate::store::{ControlPlane, MCPStore};

impl ControlPlane {
    pub async fn reset_config(&self, store: &MCPStore) -> Result<String> {
        if store.is_data_plane() {
            return store
                .queue_control_request("StoreResetRequested", serde_json::json!({}))
                .await;
        }

        if store.kernel.runtime.source_mode == SourceMode::Local {
            store
                .kernel
                .control
                .config_manager
                .save(&crate::config::McpConfig::default())?;
        }
        store.kernel.execution.pool.clear().await;
        store
            .kernel
            .runtime
            .applied_openapi_configs
            .write()
            .await
            .clear();
        store.kernel.control.registry.clear().await;
        store.kernel.control.auth.clear_statuses().await;
        let snapshot = store.kernel.persistence.cache.snapshot().await?;
        for (entity_type, entries) in snapshot.entities {
            for key in entries.keys() {
                store
                    .kernel
                    .persistence
                    .cache
                    .delete_entity(&entity_type, key)
                    .await?;
            }
        }
        for (relation_type, entries) in snapshot.relations {
            for key in entries.keys() {
                store
                    .kernel
                    .persistence
                    .cache
                    .delete_relation(&relation_type, key)
                    .await?;
            }
        }
        for (state_type, entries) in snapshot.states {
            if state_type == crate::cache::layer::CACHE_SCHEMA_STATE {
                continue;
            }
            for key in entries.keys() {
                store
                    .kernel
                    .persistence
                    .cache
                    .delete_state(&state_type, key)
                    .await?;
            }
        }
        for (event_type, entries) in snapshot.events {
            for key in entries.keys() {
                store
                    .kernel
                    .persistence
                    .cache
                    .delete_event(&event_type, key)
                    .await?;
            }
        }
        Ok(String::new())
    }

    pub async fn reset_scope(&self, store: &MCPStore, scope: &ScopeRef) -> Result<String> {
        if store.is_data_plane() {
            return store
                .queue_control_request("ScopeResetRequested", serde_json::json!({ "scope": scope }))
                .await;
        }

        let mut config = store.show_config_entry().await?;
        let mut removed = Vec::new();
        let mut changed_definitions = Vec::new();
        for (service_name, server) in &mut config.mcp_servers {
            let Some(extension) = server.mcpstore.as_mut() else {
                if matches!(scope, ScopeRef::Store) {
                    server.ensure_native_scopes();
                    if let Some(extension) = server.mcpstore.as_mut() {
                        extension.scopes.store = None;
                    }
                    removed.push(ServiceInstanceKey::new(
                        service_name.clone(),
                        ScopeRef::Store,
                    ));
                    changed_definitions.push((service_name.clone(), server.clone()));
                }
                continue;
            };
            let existed = match scope {
                ScopeRef::Store => extension.scopes.store.take().is_some(),
                ScopeRef::Agent { agent_id } => extension.scopes.agents.remove(agent_id).is_some(),
            };
            if existed {
                removed.push(ServiceInstanceKey::new(service_name.clone(), scope.clone()));
                changed_definitions.push((service_name.clone(), server.clone()));
            }
        }
        if store.kernel.runtime.source_mode == SourceMode::Local {
            store.kernel.control.config_manager.save(&config)?;
        }

        let instance_ids = removed
            .into_iter()
            .map(|key| key.instance_id())
            .collect::<Vec<_>>();
        for instance_id in &instance_ids {
            store.kernel.execution.pool.remove(*instance_id).await.ok();
            store
                .kernel
                .runtime
                .applied_openapi_configs
                .write()
                .await
                .remove(instance_id);
            store
                .kernel
                .control
                .registry
                .unregister_instance(*instance_id)
                .await;
            store.kernel.control.auth.remove_status(*instance_id).await;
        }

        let now = chrono::Utc::now().timestamp();
        for (service_name, server) in changed_definitions {
            store
                .sync_definition_projection(&service_name, &server, now)
                .await?;
        }
        for instance_id in instance_ids {
            store.cache_instance_removed(instance_id).await?;
        }
        Ok(String::new())
    }
}
