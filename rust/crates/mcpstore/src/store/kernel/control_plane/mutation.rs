use serde_json::Value;

use crate::store::prelude::*;
use crate::store::{ControlPlane, MCPStore};

impl ControlPlane {
    pub async fn remove_service(&self, store: &MCPStore, service_name: &str) -> Result<String> {
        if store.is_data_plane() {
            return store
                .queue_control_request(
                    "ServiceRemoveRequested",
                    serde_json::json!({ "service_name": service_name }),
                )
                .await;
        }

        if store.kernel.runtime.source_mode == SourceMode::Local {
            let mut config = store.kernel.control.config_manager.load_or_empty()?;
            if config.mcp_servers.remove(service_name).is_some() {
                store.kernel.control.config_manager.save(&config)?;
            } else if store.get_openapi_import(service_name).await?.is_none() {
                return Err(Error::new(
                    FailureCode::ServiceNotFound,
                    service_name.to_string(),
                ));
            }
        } else if store
            .kernel
            .control
            .registry
            .find_definition(service_name)
            .await
            .is_none()
        {
            return Err(Error::new(
                FailureCode::ServiceNotFound,
                service_name.to_string(),
            ));
        }

        let instance_ids = store
            .kernel
            .control
            .registry
            .unregister_definition(service_name)
            .await;
        for instance_id in instance_ids {
            store.kernel.execution.pool.remove(instance_id).await.ok();
            store
                .kernel
                .runtime
                .applied_openapi_configs
                .write()
                .await
                .remove(&instance_id);
            store.kernel.control.auth.remove_status(instance_id).await;
            store.cache_instance_removed(instance_id).await?;
        }
        store.cache_definition_removed(service_name).await?;
        store.clear_openapi_import_for_service(service_name).await?;

        store
            .kernel
            .execution
            .event_bus
            .publish(
                Event::new(
                    "SERVICE_REMOVED",
                    serde_json::json!({ "service_name": service_name }),
                ),
                true,
            )
            .await;
        Ok(String::new())
    }

    pub async fn update_service(
        &self,
        store: &MCPStore,
        service_name: &str,
        mut config: ServerConfig,
    ) -> Result<String> {
        if config.mcpstore.is_some() {
            return Err(Error::new(
                FailureCode::Internal,
                "Use scope APIs to modify _mcpstore metadata or declarations".to_string(),
            ));
        }
        if store.is_data_plane() {
            return store
                .queue_control_request(
                    "ServiceUpdateRequested",
                    serde_json::json!({
                        "service_name": service_name,
                        "config": config,
                    }),
                )
                .await;
        }

        let mut current = if store.kernel.runtime.source_mode == SourceMode::Local {
            store
                .kernel
                .control
                .config_manager
                .load_or_empty()?
                .mcp_servers
                .get(service_name)
                .cloned()
        } else {
            store
                .get_definition_config(service_name)
                .await?
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?
        }
        .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, service_name.to_string()))?;

        current.ensure_native_scopes();
        let base_changed = current.base_config() != config.base_config();
        config.mcpstore = current.mcpstore.clone();
        let extension = config
            .mcpstore
            .as_mut()
            .expect("current definition must have materialized _mcpstore scopes");
        extension.revision = if base_changed {
            current.definition_revision().saturating_add(1)
        } else {
            current.definition_revision()
        };

        if store.kernel.runtime.source_mode == SourceMode::Local {
            let mut stored = store.kernel.control.config_manager.load_or_empty()?;
            stored
                .mcp_servers
                .insert(service_name.to_string(), config.clone());
            store.kernel.control.config_manager.save(&stored)?;
        }
        store
            .register_configured_definition(service_name, &config)
            .await
            .map(|_| String::new())
    }

    pub async fn patch_service(
        &self,
        store: &MCPStore,
        service_name: &str,
        updates: Value,
    ) -> Result<String> {
        let updates = updates.as_object().ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                "Service base config patch must be a JSON object".to_string(),
            )
        })?;
        if updates.contains_key("_mcpstore") {
            return Err(Error::new(
                FailureCode::Internal,
                "Use scope APIs to modify _mcpstore metadata or declarations".to_string(),
            ));
        }

        if store.is_data_plane() {
            return store
                .queue_control_request(
                    "ServicePatchRequested",
                    serde_json::json!({
                        "service_name": service_name,
                        "updates": updates,
                    }),
                )
                .await;
        }

        let current = store
            .get_definition_config(service_name)
            .await?
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, service_name.to_string()))?;
        let mut config: ServerConfig = serde_json::from_value(current)
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        let merged = crate::config::merge_config(&config.base_config(), updates);
        config = serde_json::from_value(Value::Object(merged))
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        self.update_service(store, service_name, config).await
    }
}
