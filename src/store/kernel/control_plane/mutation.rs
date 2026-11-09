use serde_json::Value;

use crate::store::prelude::*;
use crate::store::{ControlPlane, MCPStore};

impl ControlPlane {
    pub async fn remove_service(&self, store: &MCPStore, service_name: &str) -> Result<()> {
        if store.definition_from_kv(service_name).await?.is_none()
            && !store.seed_file_has_service(service_name)
        {
            return Err(Error::new(
                FailureCode::ServiceNotFound,
                service_name.to_string(),
            ));
        }
        store
            .enqueue_service_event("remove", service_name, None)
            .await
    }

    pub(crate) async fn finish_remove_service(
        &self,
        store: &MCPStore,
        service_name: &str,
    ) -> Result<()> {
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
        Ok(())
    }

    pub async fn update_service(
        &self,
        store: &MCPStore,
        service_name: &str,
        mut config: ServerConfig,
    ) -> Result<()> {
        if config.mcpstore.is_some() {
            return Err(Error::new(
                FailureCode::Internal,
                "Use scope APIs to modify _mcpstore metadata or declarations".to_string(),
            ));
        }

        let mut current = store
            .definition_server_config(service_name)
            .await?
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

        store
            .enqueue_service_event("update", service_name, Some(&config))
            .await
    }

    pub async fn patch_service(
        &self,
        store: &MCPStore,
        service_name: &str,
        updates: Value,
    ) -> Result<()> {
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

        let current = store
            .definition_server_config(service_name)
            .await?
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, service_name.to_string()))?;
        let merged = crate::config::merge_config(&current.base_config(), updates);
        let config = serde_json::from_value(Value::Object(merged))
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        self.update_service(store, service_name, config).await
    }
}
