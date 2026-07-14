use serde_json::Value;

use crate::config::McpStoreExtension;
use crate::store::prelude::*;

impl MCPStore {
    pub async fn show_config(&self) -> Result<Value> {
        let config = self.show_config_entry().await?;
        project_config(&config, ConfigFormat::Native)
    }

    pub async fn export_instance_config(
        &self,
        instance_id: InstanceId,
        format: ConfigFormat,
    ) -> Result<Value> {
        if format == ConfigFormat::Native {
            return Err(StoreError::Other(
                "Native export is definition-based; use show_config".to_string(),
            ));
        }
        self.refresh_from_db_if_needed().await?;
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let mut config = crate::config::McpConfig::default();
        let server: ServerConfig = serde_json::from_value(Value::Object(instance.effective_config))
            .map_err(|error| {
                StoreError::Other(format!(
                    "Effective config for instance {instance_id} cannot be exported: {error}"
                ))
            })?;
        config.mcp_servers.insert(instance.service_name, server);
        project_config(&config, format)
    }

    pub async fn show_config_entry(&self) -> Result<crate::config::McpConfig> {
        if self.source_mode != SourceMode::Db {
            return self.config_manager.load_or_empty().map_err(Into::into);
        }

        self.refresh_from_db_if_needed().await?;
        let mut config = crate::config::McpConfig::default();
        for definition in self.registry.list_definitions().await {
            let server = Self::server_config_from_definition(&definition)?;
            config
                .mcp_servers
                .insert(definition.service_name.clone(), server);
        }
        Ok(config)
    }

    pub async fn show_scope_config(&self, scope: &ScopeRef) -> Result<Value> {
        let mut config = self.show_config_entry().await?;
        config
            .mcp_servers
            .retain(|_, server| server.scopes().descriptor(scope).is_some());
        project_config(&config, ConfigFormat::Native)
    }

    pub async fn reset_config(&self) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request("StoreResetRequested", serde_json::json!({}))
                .await;
        }

        if self.source_mode == SourceMode::Local {
            self.config_manager
                .save(&crate::config::McpConfig::default())?;
        }
        self.pool.clear().await;
        self.applied_openapi_configs.write().await.clear();
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
        Ok(())
    }

    pub async fn reset_agent_config(&self, agent_id: &str) -> Result<()> {
        self.reset_scope(&ScopeRef::Agent {
            agent_id: agent_id.to_string(),
        })
        .await
    }

    pub async fn reset_scope(&self, scope: &ScopeRef) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request("ScopeResetRequested", serde_json::json!({ "scope": scope }))
                .await;
        }

        let mut config = self.config_manager.load_or_empty()?;
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
        self.config_manager.save(&config)?;

        let now = chrono::Utc::now().timestamp();
        for (service_name, server) in changed_definitions {
            self.sync_definition_projection(&service_name, &server, now)
                .await?;
        }

        for key in removed {
            let instance_id = key.instance_id();
            self.pool.remove(instance_id).await.ok();
            self.applied_openapi_configs
                .write()
                .await
                .remove(&instance_id);
            self.registry.unregister_instance(instance_id).await;
            self.cache_instance_removed(instance_id).await?;
        }
        Ok(())
    }

    pub async fn load_from_config(&self) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self.load_from_db().await;
        }

        let config = self.config_manager.load_or_empty()?;
        self.pool.clear().await;
        self.applied_openapi_configs.write().await.clear();
        self.registry.clear().await;

        for (service_name, server) in &config.mcp_servers {
            self.register_configured_definition(service_name, server)
                .await?;
        }

        for instance in self.registry.list_instances().await {
            let status = self
                .rebuild_observed_status_for_startup(instance.instance_id)
                .await?;
            let lifecycle = self
                .resolved_instance_lifecycle(instance.instance_id)
                .await?;
            if lifecycle.startup_policy != StartupPolicy::OnStoreStart {
                continue;
            }
            let manually_stopped = status.lifecycle_state.manual_stop_persistent;
            if lifecycle.restart_policy.is_unless_stopped() && manually_stopped {
                continue;
            }
            self.clear_lifecycle_manual_stop(instance.instance_id)
                .await?;
            if let Err(error) = self
                .connect_service_internal(instance.instance_id, false)
                .await
            {
                tracing::warn!(
                    "[STORE] on-store-start instance connection failed: {} ({})",
                    instance.instance_id,
                    error
                );
            }
        }
        Ok(())
    }

    pub async fn load_from_source(&self) -> Result<()> {
        self.load_from_config().await
    }

    pub async fn get_definition_config(&self, service_name: &str) -> Result<Option<Value>> {
        self.refresh_from_db_if_needed().await?;
        let Some(definition) = self.registry.find_definition(service_name).await else {
            return Ok(None);
        };
        Ok(Some(
            serde_json::to_value(Self::server_config_from_definition(&definition)?)
                .map_err(|error| StoreError::Other(error.to_string()))?,
        ))
    }

    pub async fn get_effective_config(
        &self,
        service_name: &str,
        scope: &ScopeRef,
    ) -> Result<Option<Value>> {
        self.refresh_from_db_if_needed().await?;
        Ok(self
            .registry
            .find_instance_by_key(service_name, scope)
            .await
            .map(|instance| Value::Object(instance.effective_config)))
    }

    pub(crate) async fn resolved_instance_lifecycle(
        &self,
        instance_id: InstanceId,
    ) -> Result<crate::config::ResolvedServiceLifecycle> {
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let definition = self
            .registry
            .find_definition(&instance.service_name)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance.service_name.clone()))?;
        let config = Self::server_config_from_definition(&definition)?;
        Ok(config.resolved_lifecycle_for_scope(
            &instance.scope,
            &self.runtime_config.service_lifecycle_defaults,
        ))
    }

    pub(crate) async fn ensure_service_auto_start_allowed(
        &self,
        instance_id: InstanceId,
    ) -> Result<()> {
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        if instance.status == ConnectionStatus::Connected {
            return Ok(());
        }
        let lifecycle = self.resolved_instance_lifecycle(instance_id).await?;
        if lifecycle.startup_policy == StartupPolicy::Manual {
            return Err(StoreError::Other(format!(
                "Service instance {instance_id} uses startup_policy=manual; start it explicitly before use"
            )));
        }
        Ok(())
    }

    pub(crate) async fn register_configured_definition(
        &self,
        service_name: &str,
        config: &ServerConfig,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let scopes = config.scopes();
        let existing_instances = self
            .registry
            .list_instances()
            .await
            .into_iter()
            .filter(|instance| instance.service_name == service_name)
            .map(|instance| (instance.instance_id, instance))
            .collect::<std::collections::HashMap<_, _>>();
        self.sync_definition_projection(service_name, config, now)
            .await?;

        let mut declared_instance_ids = std::collections::HashSet::new();
        for scope in scopes.scopes() {
            let scope_revision = config.scope_revision(&scope).unwrap_or(1);
            let effective_config = config.effective_config(&scope).map_err(StoreError::Other)?;
            let transport = effective_config
                .get("transport")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    if effective_config.contains_key("url") {
                        "streamable-http".to_string()
                    } else if effective_config.contains_key("command") {
                        "stdio".to_string()
                    } else {
                        "unknown".to_string()
                    }
                });
            let url = effective_config
                .get("url")
                .and_then(Value::as_str)
                .map(str::to_string);
            let command = effective_config
                .get("command")
                .and_then(Value::as_str)
                .map(str::to_string);
            let instance_id =
                ServiceInstanceKey::new(service_name.to_string(), scope.clone()).instance_id();
            declared_instance_ids.insert(instance_id);
            let mut instance = ServiceInstance {
                instance_id,
                service_name: service_name.to_string(),
                scope: scope.clone(),
                transport,
                url,
                command,
                status: ConnectionStatus::Disconnected,
                tools: Vec::new(),
                effective_config,
                config_revision: ConfigRevision {
                    base_revision: config.definition_revision(),
                    scope_revision,
                },
                applied_config_revision: None,
                added_time: now,
            };
            if let Some(existing) = existing_instances.get(&instance_id) {
                instance.status = existing.status;
                instance.tools = existing.tools.clone();
                instance.applied_config_revision = existing.applied_config_revision;
                instance.added_time = existing.added_time;
            }
            self.registry.register_instance(instance).await;
            self.cache_instance_added(instance_id).await?;
        }

        for instance_id in existing_instances.keys().copied() {
            if declared_instance_ids.contains(&instance_id) {
                continue;
            }
            self.pool.remove(instance_id).await.ok();
            self.applied_openapi_configs
                .write()
                .await
                .remove(&instance_id);
            self.registry.unregister_instance(instance_id).await;
            self.cache_instance_removed(instance_id).await?;
        }
        Ok(())
    }

    pub(crate) async fn sync_definition_projection(
        &self,
        service_name: &str,
        config: &ServerConfig,
        now: i64,
    ) -> Result<()> {
        let extension = config.mcpstore.as_ref();
        let added_time = self
            .registry
            .find_definition(service_name)
            .await
            .map(|definition| definition.added_time)
            .unwrap_or(now);
        let definition = ServiceDefinition {
            service_name: service_name.to_string(),
            base_config: config.base_config(),
            scopes: config.scopes(),
            lifecycle: extension.and_then(|value| value.lifecycle.clone()),
            base_revision: config.definition_revision(),
            metadata: extension
                .map(|value| value.extra.clone())
                .unwrap_or_default(),
            added_time,
        };
        self.registry.register_definition(definition.clone()).await;
        self.cache_definition(&definition).await
    }

    fn server_config_from_definition(definition: &ServiceDefinition) -> Result<ServerConfig> {
        let mut config: ServerConfig = serde_json::from_value(Value::Object(
            definition.base_config.clone(),
        ))
        .map_err(|error| {
            StoreError::Other(format!(
                "Service definition '{}' cannot be decoded: {error}",
                definition.service_name
            ))
        })?;
        config.mcpstore = Some(McpStoreExtension {
            scopes: definition.scopes.clone(),
            lifecycle: definition.lifecycle.clone(),
            revision: definition.base_revision,
            extra: definition.metadata.clone(),
        });
        Ok(config)
    }
}
