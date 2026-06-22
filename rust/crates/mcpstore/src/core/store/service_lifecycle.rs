use super::*;

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

    pub(super) async fn add_service_with_identity(
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

    pub async fn connect_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceConnectRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }
        self.connect_service_internal(name, false).await
    }

    pub(super) async fn connect_service_internal(
        &self,
        name: &str,
        automatic_retry: bool,
    ) -> Result<()> {
        if self.registry.find_service(name).await.is_none() {
            return Err(StoreError::ServiceNotFound(name.to_string()));
        }
        if self.pool.is_connected(name).await {
            self.registry
                .update_status(name, ConnectionStatus::Connected)
                .await;
            return Ok(());
        }

        let retry_state = self.sync_retry_window(name).await?;
        let now = Self::now_timestamp_f64();
        if automatic_retry {
            if let Some(status) = retry_state.as_ref() {
                if Self::retry_exhausted(status, now) {
                    return Err(StoreError::Other(format!(
                        "Service automatic retry exhausted: {name}"
                    )));
                }
                if let Some(retry_in_secs) = Self::retry_wait_seconds(status, now) {
                    return Err(StoreError::Other(format!(
                        "Service reconnect backoff active: {name}, retry_in={retry_in_secs}s"
                    )));
                }
            }
        }

        self.registry
            .update_status(name, ConnectionStatus::Connecting)
            .await;
        let previous_status = retry_state
            .as_ref()
            .map(|status| status.health_status.clone());
        let mut startup = retry_state.unwrap_or_else(|| {
            self.service_status_payload(name, HealthStatus::Startup, None, Vec::new())
        });
        startup.health_status = HealthStatus::Startup;
        startup.last_health_check = Self::now_timestamp();
        startup.next_retry_time = None;
        startup.lease_deadline = if matches!(previous_status, Some(HealthStatus::HalfOpen)) {
            Some(now + self.runtime_config.half_open_lease_secs as f64)
        } else {
            None
        };
        self.put_service_status_payload(&startup).await?;
        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_CONNECTION_REQUESTED",
                    serde_json::json!({ "name": name }),
                ),
                true,
            )
            .await;

        if let Err(error) = self.pool.connect(name).await {
            let message = format!("Connection failed: {error}");
            self.pool.disconnect(name).await.ok();
            self.registry
                .update_status(name, ConnectionStatus::Error)
                .await;
            self.mark_service_retryable_failure(name, message.clone())
                .await?;
            return Err(error.into());
        }
        self.registry
            .update_status(name, ConnectionStatus::Connected)
            .await;

        let tools = match self.pool.list_tools(name).await {
            Ok(tools) => tools,
            Err(error) => {
                let message = format!("Tool discovery failed: {error}");
                self.pool.disconnect(name).await.ok();
                self.registry
                    .update_status(name, ConnectionStatus::Error)
                    .await;
                self.mark_service_retryable_failure(name, message.clone())
                    .await?;
                return Err(error.into());
            }
        };
        let tool_infos: Vec<crate::registry::ToolInfo> = tools
            .into_iter()
            .map(|t| crate::registry::ToolInfo {
                name: t.name,
                description: t.description,
                schema: t.input_schema,
            })
            .collect();

        let tool_count = tool_infos.len();
        if let Some(mut entry) = self.registry.find_service(name).await {
            entry.tools = tool_infos;
            entry.status = ConnectionStatus::Connected;
            self.registry.register(entry).await;
        }

        let tools = self.registry.list_service_tools(name).await;
        self.cache_service_connected(name, &tools).await?;

        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_CONNECTED",
                    serde_json::json!({
                        "name": name, "tools_count": tool_count
                    }),
                ),
                true,
            )
            .await;

        tracing::info!("[STORE] Service connected: {} (tools={})", name, tool_count);
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

    pub async fn list_services(&self) -> Vec<ServiceEntry> {
        self.refresh_from_db_if_needed().await.ok();
        let mut services = self.registry.list_services().await;
        if self.source_mode == SourceMode::Db {
            for service in &mut services {
                self.hydrate_service_status_from_cache(service).await.ok();
            }
        }
        services
    }

    pub async fn find_service(&self, name: &str) -> Option<ServiceEntry> {
        self.refresh_from_db_if_needed().await.ok();
        let mut service = self.registry.find_service(name).await?;
        if self.source_mode == SourceMode::Db {
            self.hydrate_service_status_from_cache(&mut service)
                .await
                .ok();
        }
        Some(service)
    }

    pub async fn list_tools(&self, service_name: &str) -> Result<Vec<ToolDescription>> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_service(service_name).await.is_none() {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        }
        Ok(self
            .registry
            .list_service_tools(service_name)
            .await
            .into_iter()
            .map(|tool| ToolDescription {
                name: tool.name,
                description: tool.description,
                input_schema: tool.schema,
            })
            .collect())
    }

    pub async fn list_resources(&self, service_name: &str) -> Result<Vec<serde_json::Value>> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .list_resources(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn list_resource_templates(
        &self,
        service_name: &str,
    ) -> Result<Vec<serde_json::Value>> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .list_resource_templates(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn read_resource(&self, service_name: &str, uri: &str) -> Result<serde_json::Value> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .read_resource(service_name, uri)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn list_prompts(&self, service_name: &str) -> Result<Vec<serde_json::Value>> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .list_prompts(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn get_prompt(
        &self,
        service_name: &str,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .get_prompt(service_name, prompt_name, arguments)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn list_all_tools(&self) -> Vec<crate::registry::ToolInfo> {
        self.refresh_from_db_if_needed().await.ok();
        self.registry.list_all_tools().await
    }

    pub async fn list_agents(&self) -> Result<Vec<serde_json::Value>> {
        self.refresh_from_db_if_needed().await?;
        let mut agent_ids = self.registry.list_agent_ids().await;
        let cached = self.cache.get_all_relations_async("agent_services").await?;
        for agent_id in cached.keys() {
            if !agent_ids.contains(agent_id) {
                agent_ids.push(agent_id.clone());
            }
        }
        agent_ids.sort();
        agent_ids.dedup();

        let mut agents = Vec::with_capacity(agent_ids.len());
        for agent_id in agent_ids {
            agents.push(serde_json::json!({
                "agent_id": agent_id,
                "services": self.list_agent_service_names(&agent_id).await?,
            }));
        }
        Ok(agents)
    }

    pub async fn call_tool(
        &self,
        service_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<crate::transport::ToolCallResult> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_service(service_name).await.is_none() {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        }
        if !self.pool.is_connected(service_name).await {
            self.connect_service_internal(service_name, true).await?;
        }
        let event_args = args.clone();
        let started_at = std::time::Instant::now();
        match self.pool.call_tool(service_name, tool_name, args).await {
            Ok(result) => {
                let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                self.record_health_check_result(service_name, true, Some(latency_ms), None)
                    .await?;
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_COMPLETED",
                            serde_json::json!({
                                "service_name": service_name,
                                "tool_name": tool_name,
                                "arguments": event_args,
                                "latency_ms": latency_ms,
                                "is_error": result.is_error,
                                "status": if result.is_error { "error" } else { "success" },
                            }),
                        ),
                        true,
                    )
                    .await;
                Ok(result)
            }
            Err(error) => {
                let message = format!("Tool call failed: {error}");
                let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                self.pool.disconnect(service_name).await.ok();
                self.registry
                    .update_status(service_name, ConnectionStatus::Error)
                    .await;
                self.mark_service_retryable_failure(service_name, message)
                    .await?;
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_FAILED",
                            serde_json::json!({
                                "service_name": service_name,
                                "tool_name": tool_name,
                                "arguments": event_args,
                                "latency_ms": latency_ms,
                                "is_error": true,
                                "status": "error",
                                "error": error.to_string(),
                            }),
                        ),
                        true,
                    )
                    .await;
                Err(StoreError::Transport(error))
            }
        }
    }

    pub async fn resolve_tool_for_agent(
        &self,
        agent_id: &str,
        user_input: &str,
    ) -> Result<ToolResolution> {
        self.refresh_from_db_if_needed().await?;
        let service_names = if agent_id == GLOBAL_AGENT_STORE {
            self.registry
                .list_services()
                .await
                .into_iter()
                .map(|service| service.name)
                .collect::<Vec<_>>()
        } else {
            self.list_agent_service_names(agent_id).await?
        };
        let mut available = Vec::new();
        for global_service_name in service_names {
            let service = match self.registry.find_service(&global_service_name).await {
                Some(service) => service,
                None => continue,
            };
            let local_service_name = if agent_id == GLOBAL_AGENT_STORE {
                service.name.clone()
            } else {
                service.original_name.clone()
            };
            for tool in service.tools {
                let global_tool_name = generate_tool_global_name(&service.name, &tool.name)?;
                available.push(AvailableTool {
                    name: format!("{}_{}", local_service_name, tool.name),
                    original_name: Some(tool.name),
                    service_name: Some(local_service_name.clone()),
                    global_service_name: Some(service.name.clone()),
                    global_tool_name: Some(global_tool_name),
                });
            }
        }
        resolve_tool(agent_id, user_input, &available, "canonical", true)
    }

    pub async fn disconnect_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceDisconnectRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.pool.disconnect(name).await?;
        self.registry
            .update_status(name, ConnectionStatus::Disconnected)
            .await;
        self.set_service_status(name, HealthStatus::Disconnected, None, Vec::new())
            .await?;
        self.event_bus
            .publish(
                Event::new("SERVICE_DISCONNECTED", serde_json::json!({ "name": name })),
                true,
            )
            .await;
        tracing::info!("[STORE] Service disconnected: {}", name);
        Ok(())
    }

    pub async fn show_config(&self) -> Result<serde_json::Value> {
        let config = self.show_config_entry().await?;
        serde_json::to_value(config)
            .map_err(|e| StoreError::Other(format!("Config serialization failed: {e}")))
    }

    pub async fn show_config_entry(&self) -> Result<crate::config::McpConfig> {
        if self.source_mode == SourceMode::Db {
            self.refresh_from_db_if_needed().await?;

            let mut config = crate::config::McpConfig::default();
            for service in self.registry.list_services().await {
                let service_config: ServerConfig = serde_json::from_value(service.config.clone())
                    .map_err(|e| {
                    StoreError::Other(format!(
                        "Service config deserialization failed during show_config: {e}"
                    ))
                })?;
                config.mcp_servers.insert(service.name, service_config);
            }

            let mut agent_ids = self.registry.list_agent_ids().await;
            agent_ids.sort();
            agent_ids.dedup();
            for agent_id in agent_ids {
                if agent_id == GLOBAL_AGENT_STORE {
                    continue;
                }
                let mut services = self.registry.list_agent_services(&agent_id).await;
                services.sort();
                services.dedup();
                if !services.is_empty() {
                    config.agents.insert(agent_id, services);
                }
            }

            return Ok(config);
        }

        Ok(self.config_manager.load_or_default())
    }

    pub async fn reset_config(&self) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request("StoreResetRequested", serde_json::json!({}))
                .await;
        }

        self.pool.clear().await;
        self.registry.clear().await;
        let snapshot = self.cache.snapshot().await?;
        for (entity_type, entries) in snapshot.entities {
            for key in entries.keys() {
                self.cache.delete_entity(&entity_type, key).await?;
            }
        }
        for (relation_type, entries) in snapshot.relations {
            for key in entries.keys() {
                self.cache.delete_relation(&relation_type, key).await?;
            }
        }
        for (state_type, entries) in snapshot.states {
            for key in entries.keys() {
                self.cache.delete_state(&state_type, key).await?;
            }
        }
        for (event_type, entries) in snapshot.events {
            for key in entries.keys() {
                self.cache.delete_event(&event_type, key).await?;
            }
        }

        if self.source_mode == SourceMode::Local {
            self.config_manager
                .save(&crate::config::McpConfig::default())?;
        }
        Ok(())
    }

    pub async fn load_from_config(&self) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self.load_from_db().await;
        }

        let cfg = self.config_manager.load_or_default();
        for (name, server_config) in cfg.mcp_servers {
            if self.registry.find_service(&name).await.is_none() {
                self.register_configured_service(&name, server_config)
                    .await?;
            }
        }
        let cfg = self.config_manager.load_or_default();
        for (agent_id, services) in cfg.agents {
            for service_name in services {
                if self.registry.find_service(&service_name).await.is_some() {
                    self.registry
                        .add_to_agent_scope(&agent_id, &service_name)
                        .await;
                }
            }
            self.cache_agent_scope(&agent_id).await?;
        }
        Ok(())
    }

    pub async fn load_from_source(&self) -> Result<()> {
        self.load_from_config().await
    }

    pub async fn restart_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceRestartRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.disconnect_service(name).await.ok();
        self.connect_service(name).await
    }

    pub async fn get_service_config(&self, name: &str) -> Result<Option<serde_json::Value>> {
        self.refresh_from_db_if_needed().await?;
        if let Some(entry) = self.registry.find_service(name).await {
            return Ok(Some(entry.config));
        }
        let Some(value) = self.cache.get_entity("services", name).await? else {
            return Ok(None);
        };
        Ok(value.get("config").cloned())
    }

    async fn register_configured_service(&self, name: &str, config: ServerConfig) -> Result<()> {
        let parsed = parse_agent_scoped(name)?;
        let agent_id = parsed
            .agent_id
            .unwrap_or_else(|| GLOBAL_AGENT_STORE.to_string());
        let original_name = parsed.local_name;
        let transport = config.infer_transport().to_string();
        let entry = ServiceEntry {
            name: name.to_string(),
            original_name: original_name.clone(),
            agent_id: agent_id.clone(),
            transport,
            url: config.url.clone(),
            command: config.command.clone(),
            status: ConnectionStatus::Disconnected,
            tools: Vec::new(),
            config: serde_json::to_value(&config).unwrap_or_default(),
            added_time: chrono::Utc::now().timestamp(),
        };
        self.registry.register(entry).await;
        self.pool.add(name.to_string(), config.clone()).await;
        if self.cache.get_entity("services", name).await?.is_none() {
            self.cache_service_added(
                name,
                &original_name,
                &agent_id,
                &config,
                chrono::Utc::now().timestamp(),
            )
            .await?;
        }
        if agent_id != GLOBAL_AGENT_STORE {
            self.registry.add_to_agent_scope(&agent_id, name).await;
            if self.source_mode == SourceMode::Local {
                self.cache_agent_scope(&agent_id).await?;
            }
        }
        Ok(())
    }
}
