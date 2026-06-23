use crate::store::prelude::*;

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
}
