use crate::store::prelude::*;

fn merge_json_object(target: &mut serde_json::Value, updates: serde_json::Value) -> Result<()> {
    let target_object = target.as_object_mut().ok_or_else(|| {
        StoreError::Other("Service config is not a JSON object, cannot patch".to_string())
    })?;
    let updates_object = updates.as_object().ok_or_else(|| {
        StoreError::Other("Service config patch must be a JSON object".to_string())
    })?;

    for (key, value) in updates_object {
        match (target_object.get_mut(key), value) {
            (Some(existing @ serde_json::Value::Object(_)), serde_json::Value::Object(_)) => {
                merge_json_object(existing, value.clone())?;
            }
            _ => {
                target_object.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(())
}

impl MCPStore {
    pub async fn add_service(&self, name: &str, config: ServerConfig) -> Result<()> {
        self.add_service_with_identity(name, name, GLOBAL_AGENT_STORE, config)
            .await
    }

    pub async fn add_service_for_agent(
        &self,
        agent_id: &str,
        local_name: &str,
        config: ServerConfig,
    ) -> Result<String> {
        let resolution = normalize_service_name(agent_id, local_name, "global", true)?;
        if self.source_mode == SourceMode::Db {
            self.queue_service_add_request(
                &resolution.global_name,
                &resolution.local_name,
                agent_id,
                &config,
            )
            .await?;
            return Ok(resolution.global_name);
        }
        self.add_service_with_identity(
            &resolution.global_name,
            &resolution.local_name,
            agent_id,
            config,
        )
        .await?;
        self.assign_service_to_agent(agent_id, &resolution.global_name)
            .await?;
        Ok(resolution.global_name)
    }

    pub(crate) async fn add_service_with_identity(
        &self,
        name: &str,
        original_name: &str,
        agent_id: &str,
        config: ServerConfig,
    ) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_service_add_request(name, original_name, agent_id, &config)
                .await;
        }

        let transport = config.infer_transport().to_string();

        let entry = ServiceEntry {
            name: name.to_string(),
            original_name: original_name.to_string(),
            agent_id: agent_id.to_string(),
            transport: transport.clone(),
            url: config.url.clone(),
            command: config.command.clone(),
            status: ConnectionStatus::Disconnected,
            tools: Vec::new(),
            config: serde_json::to_value(&config).unwrap_or_default(),
            added_time: chrono::Utc::now().timestamp(),
        };

        self.registry.register(entry).await;
        self.pool.add(name.to_string(), config.clone()).await;
        self.cache_service_added(
            name,
            original_name,
            agent_id,
            &config,
            chrono::Utc::now().timestamp(),
        )
        .await?;

        self.event_bus
            .publish(
                Event::new("SERVICE_ADD_REQUESTED", serde_json::json!({ "name": name })),
                true,
            )
            .await;

        if self.source_mode == SourceMode::Local {
            let mut cfg = self.config_manager.load_or_default();
            cfg.mcp_servers.insert(name.to_string(), config);
            self.config_manager.save(&cfg)?;
        }

        tracing::info!("[STORE] Service added: {} (transport={})", name, transport);
        Ok(())
    }

    pub async fn remove_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceRemoveRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.pool.remove(name).await.ok();
        self.registry.unregister(name).await;
        self.cache_service_removed(name).await?;

        if self.source_mode == SourceMode::Local {
            let mut cfg = self.config_manager.load_or_default();
            cfg.mcp_servers.remove(name);
            self.config_manager.save(&cfg)?;
        }

        self.event_bus
            .publish(
                Event::new("SERVICE_REMOVED", serde_json::json!({ "name": name })),
                true,
            )
            .await;

        tracing::info!("[STORE] Service removed: {}", name);
        Ok(())
    }

    pub async fn update_service(&self, name: &str, config: ServerConfig) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceUpdateRequested",
                    serde_json::json!({
                        "service_name": name,
                        "config": config,
                    }),
                )
                .await;
        }

        let existing = self
            .registry
            .find_service(name)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(name.to_string()))?;
        self.pool.remove(name).await.ok();
        self.registry.unregister(name).await;
        self.cache_service_removed(name).await?;
        self.add_service_with_identity(name, &existing.original_name, &existing.agent_id, config)
            .await?;
        if existing.agent_id != GLOBAL_AGENT_STORE {
            self.registry
                .add_to_agent_scope(&existing.agent_id, name)
                .await;
            self.cache_agent_scope(&existing.agent_id).await?;
        }
        Ok(())
    }

    pub async fn patch_service(&self, name: &str, updates: serde_json::Value) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServicePatchRequested",
                    serde_json::json!({
                        "service_name": name,
                        "updates": updates,
                    }),
                )
                .await;
        }

        let mut config = self
            .get_service_config(name)
            .await?
            .ok_or_else(|| StoreError::ServiceNotFound(name.to_string()))?;
        merge_json_object(&mut config, updates)?;
        let config: ServerConfig = serde_json::from_value(config).map_err(|e| {
            StoreError::Other(format!(
                "Post-patch service config deserialization failed: {e}"
            ))
        })?;
        self.update_service(name, config).await
    }
}
