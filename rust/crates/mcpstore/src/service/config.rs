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
            return Err(Error::new(
                FailureCode::Internal,
                "Native export is definition-based; use show_config".to_string(),
            ));
        }
        let instance = self
            .instance_from_kv(instance_id)
            .await?
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        let mut config = crate::config::McpConfig::default();
        let server: ServerConfig = serde_json::from_value(Value::Object(instance.effective_config))
            .map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!(
                        "Effective config for instance {instance_id} cannot be exported: {error}"
                    ),
                )
            })?;
        config.mcp_servers.insert(instance.service_name, server);
        project_config(&config, format)
    }

    pub async fn show_config_entry(&self) -> Result<crate::config::McpConfig> {
        let mut config = crate::config::McpConfig::default();
        for definition in self.definitions_from_kv().await? {
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

    pub async fn show_session_config(&self, session_key: &str) -> Result<Value> {
        let session = self.get_session(session_key).await?.ok_or_else(|| {
            Error::new(
                FailureCode::SessionNotFound,
                format!("session not found: session_key={session_key}"),
            )
        })?;
        let scope = match session.scope {
            crate::cache::models::SessionScope::Store => ScopeRef::Store,
            crate::cache::models::SessionScope::Agent => ScopeRef::Agent {
                agent_id: session.agent_id.ok_or_else(|| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Agent-scoped session is missing agent_id: {session_key}"),
                    )
                })?,
            },
        };
        self.show_scope_config(&scope).await
    }

    pub async fn load_from_config(&self) -> Result<()> {
        // Unified model: kv is the source of truth. The control panel reads the seed once at startup (mcp.json wins at boot),
        // writes made while we were down are replayed via unconsumed service_events; afterwards the file serves only
        // as the post-consume flush target. The data panel has no seed, no hydration, no subscription — queries read kv directly,
        // calls route per placement.
        if !matches!(
            self.kernel.runtime.panel_role,
            crate::store::PanelRole::ControlPanel
        ) {
            return Ok(());
        }

        let seeded = self.seed_from_config_file().await?;
        if !seeded {
            self.load_from_db().await?;
        }
        self.spawn_service_event_feed_from_ref();
        self.spawn_tool_call_request_feed_from_ref();
        self.connect_desired_running_instances(None).await;
        Ok(())
    }

    /// Startup seed: the control panel pours mcp.json into the source of truth (registered into kv + registry).
    /// Returns false = nothing to seed (no config_path / empty file); the caller falls
    /// back to hydrating from the shared store.
    async fn seed_from_config_file(&self) -> Result<bool> {
        if !self.kernel.runtime.sync_config_file {
            // Shared backend without an explicit config_path: no local file is read as seed.
            return Ok(false);
        }
        let config = self.kernel.control.config_manager.load_or_empty()?;
        if config.mcp_servers.is_empty() {
            return Ok(false);
        }
        self.kernel.execution.pool.clear().await;
        self.kernel
            .runtime
            .applied_openapi_configs
            .write()
            .await
            .clear();
        self.kernel.control.registry.clear().await;
        self.kernel.control.auth.clear_statuses().await;
        for (service_name, server) in &config.mcp_servers {
            self.register_configured_definition(service_name, server)
                .await?;
        }
        Ok(true)
    }

    pub async fn load_from_source(&self) -> Result<()> {
        self.load_from_config().await
    }

    /// Whether the seed file contains this service (only meaningful when the file is this store's seed/flush target).
    pub(crate) fn seed_file_has_service(&self, service_name: &str) -> bool {
        self.kernel.runtime.sync_config_file
            && self
                .kernel
                .control
                .config_manager
                .load_or_empty()
                .map(|config| config.mcp_servers.contains_key(service_name))
                .unwrap_or(false)
    }

    pub async fn get_definition_config(&self, service_name: &str) -> Result<Option<Value>> {
        Ok(self
            .definition_server_config(service_name)
            .await?
            .map(|server| {
                serde_json::to_value(server)
                    .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))
            })
            .transpose()?)
    }

    /// For shared-store write paths. Reads this one definition only; never flushes the whole table into the caller's registry.
    pub(crate) async fn definition_server_config(
        &self,
        service_name: &str,
    ) -> Result<Option<ServerConfig>> {
        let Some(definition) = self.definition_from_kv(service_name).await? else {
            return Ok(None);
        };
        Ok(Some(Self::server_config_from_definition(&definition)?))
    }

    pub async fn get_effective_config(
        &self,
        service_name: &str,
        scope: &ScopeRef,
    ) -> Result<Option<Value>> {
        let instance_id =
            ServiceInstanceKey::new(service_name.to_string(), scope.clone()).instance_id();
        Ok(self
            .instance_from_kv(instance_id)
            .await?
            .map(|instance| Value::Object(instance.effective_config)))
    }

    pub(crate) async fn resolved_instance_lifecycle(
        &self,
        instance_id: InstanceId,
    ) -> Result<crate::config::ResolvedServiceLifecycle> {
        let instance = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        let definition = self
            .kernel
            .control
            .registry
            .find_definition(&instance.service_name)
            .await
            .ok_or_else(|| {
                Error::new(FailureCode::ServiceNotFound, instance.service_name.clone())
            })?;
        let config = Self::server_config_from_definition(&definition)?;
        Ok(config.resolved_lifecycle_for_scope(
            &instance.scope,
            &self
                .kernel
                .runtime
                .runtime_config
                .service_lifecycle_defaults,
        ))
    }

    pub(crate) async fn ensure_service_auto_start_allowed(
        &self,
        instance_id: InstanceId,
    ) -> Result<()> {
        let state = self
            .kernel
            .control
            .state
            .get(instance_id)
            .await?
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        if state.phase == crate::state::RuntimePhase::Running {
            return Ok(());
        }
        let lifecycle = self.resolved_instance_lifecycle(instance_id).await?;
        if lifecycle.startup_policy == StartupPolicy::Manual {
            return Err(Error::new(FailureCode::Internal, format!(
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
            .kernel
            .control
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
            let effective_config = config
                .effective_config(&scope)
                .map_err(|message| Error::new(FailureCode::ConfigInvalid, message))?;
            let effective_auth =
                serde_json::from_value::<ServerConfig>(Value::Object(effective_config.clone()))
                    .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?
                    .auth;
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
                instance.tools = existing.tools.clone();
                instance.applied_config_revision = existing.applied_config_revision;
                instance.added_time = existing.added_time;
            }
            self.kernel
                .control
                .registry
                .register_instance(instance)
                .await;
            self.cache_instance_added(instance_id).await?;
            self.kernel
                .control
                .auth
                .initialize_status(instance_id, &effective_auth)
                .await;
        }

        for instance_id in existing_instances.keys().copied() {
            if declared_instance_ids.contains(&instance_id) {
                continue;
            }
            self.kernel.execution.pool.remove(instance_id).await.ok();
            self.kernel
                .runtime
                .applied_openapi_configs
                .write()
                .await
                .remove(&instance_id);
            self.kernel
                .control
                .registry
                .unregister_instance(instance_id)
                .await;
            self.kernel.control.auth.remove_status(instance_id).await;
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
            .kernel
            .control
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
            handshake_mode: extension.and_then(|value| value.handshake_mode),
            placement: extension
                .map(|value| value.placement.clone())
                .unwrap_or_default(),
            base_revision: config.definition_revision(),
            metadata: extension
                .map(|value| value.extra.clone())
                .unwrap_or_default(),
            added_time,
        };
        self.kernel
            .control
            .registry
            .register_definition(definition.clone())
            .await;
        self.cache_definition(&definition).await
    }

    pub(crate) fn server_config_from_definition(
        definition: &ServiceDefinition,
    ) -> Result<ServerConfig> {
        let mut config: ServerConfig = serde_json::from_value(Value::Object(
            definition.base_config.clone(),
        ))
        .map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!(
                    "Service definition '{}' cannot be decoded: {error}",
                    definition.service_name
                ),
            )
        })?;
        config.mcpstore = Some(McpStoreExtension {
            scopes: definition.scopes.clone(),
            lifecycle: definition.lifecycle.clone(),
            handshake_mode: definition.handshake_mode,
            placement: definition.placement.clone(),
            revision: definition.base_revision,
            extra: definition.metadata.clone(),
        });
        Ok(config)
    }
}
