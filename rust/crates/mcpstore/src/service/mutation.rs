use serde_json::Value;

use crate::store::prelude::*;

impl MCPStore {
    pub async fn remove_service(&self, service_name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceRemoveRequested",
                    serde_json::json!({ "service_name": service_name }),
                )
                .await;
        }

        if self.source_mode == SourceMode::Local {
            let mut config = self.config_manager.load_or_empty()?;
            if config.mcp_servers.remove(service_name).is_none() {
                return Err(StoreError::ServiceNotFound(service_name.to_string()));
            }
            self.config_manager.save(&config)?;
        } else if self.registry.find_definition(service_name).await.is_none() {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        }

        let instance_ids = self.registry.unregister_definition(service_name).await;
        for instance_id in instance_ids {
            self.pool.remove(instance_id).await.ok();
            self.applied_openapi_configs
                .write()
                .await
                .remove(&instance_id);
            self.cache_instance_removed(instance_id).await?;
        }
        self.cache_definition_removed(service_name).await?;
        self.clear_openapi_import_for_service(service_name).await?;

        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_REMOVED",
                    serde_json::json!({ "service_name": service_name }),
                ),
                true,
            )
            .await;
        Ok(())
    }

    pub async fn update_service(&self, service_name: &str, mut config: ServerConfig) -> Result<()> {
        if config.mcpstore.is_some() {
            return Err(StoreError::Other(
                "Use scope APIs to modify _mcpstore metadata or declarations".to_string(),
            ));
        }
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceUpdateRequested",
                    serde_json::json!({
                        "service_name": service_name,
                        "config": config,
                    }),
                )
                .await;
        }

        let mut current = if self.source_mode == SourceMode::Local {
            self.config_manager
                .load_or_empty()?
                .mcp_servers
                .get(service_name)
                .cloned()
        } else {
            self.get_definition_config(service_name)
                .await?
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| StoreError::Other(error.to_string()))?
        }
        .ok_or_else(|| StoreError::ServiceNotFound(service_name.to_string()))?;

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

        if self.source_mode == SourceMode::Local {
            let mut stored = self.config_manager.load_or_empty()?;
            stored
                .mcp_servers
                .insert(service_name.to_string(), config.clone());
            self.config_manager.save(&stored)?;
        }
        self.register_configured_definition(service_name, &config)
            .await
    }

    pub async fn patch_service(&self, service_name: &str, updates: Value) -> Result<()> {
        let updates = updates.as_object().ok_or_else(|| {
            StoreError::Other("Service base config patch must be a JSON object".to_string())
        })?;
        if updates.contains_key("_mcpstore") {
            return Err(StoreError::Other(
                "Use scope APIs to modify _mcpstore metadata or declarations".to_string(),
            ));
        }

        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServicePatchRequested",
                    serde_json::json!({
                        "service_name": service_name,
                        "updates": updates,
                    }),
                )
                .await;
        }

        let current = self
            .get_definition_config(service_name)
            .await?
            .ok_or_else(|| StoreError::ServiceNotFound(service_name.to_string()))?;
        let mut config: ServerConfig = serde_json::from_value(current)
            .map_err(|error| StoreError::Other(error.to_string()))?;
        let merged = crate::config::merge_config(&config.base_config(), updates);
        config = serde_json::from_value(Value::Object(merged))
            .map_err(|error| StoreError::Other(error.to_string()))?;
        self.update_service(service_name, config).await
    }
}
