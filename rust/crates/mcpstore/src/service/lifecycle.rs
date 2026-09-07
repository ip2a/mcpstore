use crate::store::prelude::*;
use crate::store::ControlPlane;

impl ControlPlane {
    pub async fn add_service(
        &self,
        store: &MCPStore,
        service_name: &str,
        mut config: ServerConfig,
    ) -> Result<String> {
        if store.is_data_plane() {
            return store
                .queue_control_request(
                    "ServiceAddRequested",
                    serde_json::json!({
                        "service_name": service_name,
                        "config": config,
                    }),
                )
                .await;
        }

        if store
            .kernel
            .control
            .registry
            .find_definition(service_name)
            .await
            .is_some()
        {
            return Err(Error::new(
                FailureCode::Internal,
                format!("Service definition already exists: {service_name}"),
            ));
        }

        config.ensure_native_scopes();
        if store.kernel.runtime.source_mode == SourceMode::Local {
            let mut stored = store.kernel.control.config_manager.load_or_empty()?;
            if stored.mcp_servers.contains_key(service_name) {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("Service definition already exists: {service_name}"),
                ));
            }
            stored
                .mcp_servers
                .insert(service_name.to_string(), config.clone());
            store.kernel.control.config_manager.save(&stored)?;
        }

        store
            .register_configured_definition(service_name, &config)
            .await?;
        store
            .kernel
            .execution
            .event_bus
            .publish(
                Event::new(
                    "SERVICE_ADD_REQUESTED",
                    serde_json::json!({ "service_name": service_name }),
                ),
                true,
            )
            .await;
        Ok(String::new())
    }
}
