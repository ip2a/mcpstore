use crate::store::prelude::*;

impl MCPStore {
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

    pub async fn show_config_scoped(&self, agent_id: Option<&str>) -> Result<serde_json::Value> {
        let mut config = self.show_config_entry().await?;
        let Some(agent_id) = agent_id else {
            return serde_json::to_value(config)
                .map_err(|e| StoreError::Other(format!("Config serialization failed: {e}")));
        };

        let service_names = config.agents.get(agent_id).cloned().unwrap_or_default();
        let service_set = service_names
            .iter()
            .collect::<std::collections::HashSet<_>>();
        config
            .mcp_servers
            .retain(|service_name, _| service_set.contains(service_name));
        config.agents.clear();
        config.agents.insert(agent_id.to_string(), service_names);

        serde_json::to_value(config)
            .map_err(|e| StoreError::Other(format!("Config serialization failed: {e}")))
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

    pub async fn reset_agent_config(&self, agent_id: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "AgentResetRequested",
                    serde_json::json!({ "agent_id": agent_id }),
                )
                .await;
        }

        self.registry.clear_agent_scope(agent_id).await;
        self.cache
            .delete_relation("agent_services", agent_id)
            .await?;
        if self.source_mode == SourceMode::Local {
            let mut cfg = self.config_manager.load_or_default();
            cfg.agents.remove(agent_id);
            self.config_manager.save(&cfg)?;
        }
        Ok(())
    }

    pub async fn reset_mcp_json_scope(&self, scope: Option<&str>) -> Result<()> {
        let scope = scope.unwrap_or("all");
        if scope == "all" {
            return self.reset_config().await;
        }
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "McpJsonResetRequested",
                    serde_json::json!({ "scope": scope }),
                )
                .await;
        }

        let mut cfg = self.config_manager.load_or_default();
        let service_names: Vec<String> = if scope == GLOBAL_AGENT_STORE {
            cfg.mcp_servers
                .keys()
                .filter(|name| {
                    parse_agent_scoped(name)
                        .map(|parsed| parsed.agent_id.is_none())
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        } else {
            cfg.mcp_servers
                .keys()
                .filter(|name| {
                    parse_agent_scoped(name)
                        .map(|parsed| parsed.agent_id.as_deref() == Some(scope))
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        };

        for service_name in service_names {
            self.pool.remove(&service_name).await.ok();
            self.registry.unregister(&service_name).await;
            self.cache_service_removed(&service_name).await?;
            cfg.mcp_servers.remove(&service_name);
        }
        if scope != GLOBAL_AGENT_STORE {
            self.registry.clear_agent_scope(scope).await;
            self.cache.delete_relation("agent_services", scope).await?;
            cfg.agents.remove(scope);
        } else {
            let remaining_services = cfg
                .mcp_servers
                .keys()
                .cloned()
                .collect::<std::collections::HashSet<_>>();
            cfg.agents.retain(|_, services| {
                services.retain(|service_name| remaining_services.contains(service_name));
                !services.is_empty()
            });
        }
        self.config_manager.save(&cfg)?;
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
