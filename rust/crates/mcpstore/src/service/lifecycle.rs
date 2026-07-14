use crate::store::prelude::*;

impl MCPStore {
    pub async fn add_service(&self, service_name: &str, mut config: ServerConfig) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceAddRequested",
                    serde_json::json!({
                        "service_name": service_name,
                        "config": config,
                    }),
                )
                .await;
        }

        if self.registry.find_definition(service_name).await.is_some() {
            return Err(StoreError::Other(format!(
                "Service definition already exists: {service_name}"
            )));
        }

        config.ensure_native_scopes();
        if self.source_mode == SourceMode::Local {
            let mut stored = self.config_manager.load_or_empty()?;
            if stored.mcp_servers.contains_key(service_name) {
                return Err(StoreError::Other(format!(
                    "Service definition already exists: {service_name}"
                )));
            }
            stored
                .mcp_servers
                .insert(service_name.to_string(), config.clone());
            self.config_manager.save(&stored)?;
        }

        self.register_configured_definition(service_name, &config)
            .await?;
        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_ADD_REQUESTED",
                    serde_json::json!({ "service_name": service_name }),
                ),
                true,
            )
            .await;
        Ok(())
    }
}
