use crate::config::ScopeDescriptor;
use crate::store::prelude::*;
use serde_json::Value;

impl MCPStore {
    pub async fn declare_service_scope(
        &self,
        service_name: &str,
        scope: &ScopeRef,
        mut descriptor: ScopeDescriptor,
    ) -> Result<InstanceId> {
        let instance_id =
            ServiceInstanceKey::new(service_name.to_string(), scope.clone()).instance_id();
        if self.source_mode == SourceMode::Db {
            self.queue_control_request(
                "ServiceScopeDeclareRequested",
                serde_json::json!({
                    "service_name": service_name,
                    "scope": scope,
                    "descriptor": descriptor,
                }),
            )
            .await?;
            return Ok(instance_id);
        }

        let mut config = self.config_manager.load_or_empty()?;
        let server = config
            .mcp_servers
            .get_mut(service_name)
            .ok_or_else(|| StoreError::ServiceNotFound(service_name.to_string()))?;
        server.ensure_native_scopes();
        let extension = server
            .mcpstore
            .as_mut()
            .expect("ensure_native_scopes must materialize _mcpstore");
        descriptor.revision = match extension.scopes.descriptor(scope) {
            Some(existing)
                if existing.config == descriptor.config
                    && existing.lifecycle == descriptor.lifecycle =>
            {
                existing.revision.max(1)
            }
            Some(existing) => existing.revision.max(1).saturating_add(1),
            None => 1,
        };
        match scope {
            ScopeRef::Store => extension.scopes.store = Some(descriptor),
            ScopeRef::Agent { agent_id } => {
                extension.scopes.agents.insert(agent_id.clone(), descriptor);
            }
        }

        let server = server.clone();
        self.config_manager.save(&config)?;

        let effective_config = server.effective_config(scope).map_err(StoreError::Other)?;
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
        let previous = self.registry.find_instance(instance_id).await;
        let now = chrono::Utc::now().timestamp();
        let instance = ServiceInstance {
            instance_id,
            service_name: service_name.to_string(),
            scope: scope.clone(),
            transport,
            url,
            command,
            status: previous
                .as_ref()
                .map(|instance| instance.status)
                .unwrap_or(ConnectionStatus::Disconnected),
            tools: previous
                .as_ref()
                .map(|instance| instance.tools.clone())
                .unwrap_or_default(),
            effective_config,
            config_revision: ConfigRevision {
                base_revision: server.definition_revision(),
                scope_revision: server.scope_revision(scope).unwrap_or(1),
            },
            applied_config_revision: previous
                .as_ref()
                .and_then(|instance| instance.applied_config_revision),
            added_time: previous
                .as_ref()
                .map(|instance| instance.added_time)
                .unwrap_or(now),
        };
        self.registry.register_instance(instance).await;
        self.sync_definition_projection(service_name, &server, now)
            .await?;
        self.cache_instance_added(instance_id).await?;
        Ok(instance_id)
    }

    pub async fn remove_service_scope(&self, service_name: &str, scope: &ScopeRef) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceScopeRemoveRequested",
                    serde_json::json!({
                        "service_name": service_name,
                        "scope": scope,
                    }),
                )
                .await;
        }

        let mut config = self.config_manager.load_or_empty()?;
        let server = config
            .mcp_servers
            .get_mut(service_name)
            .ok_or_else(|| StoreError::ServiceNotFound(service_name.to_string()))?;
        server.ensure_native_scopes();
        let extension = server
            .mcpstore
            .as_mut()
            .expect("ensure_native_scopes must materialize _mcpstore");
        let removed = match scope {
            ScopeRef::Store => extension.scopes.store.take(),
            ScopeRef::Agent { agent_id } => extension.scopes.agents.remove(agent_id),
        };
        if removed.is_none() {
            return Err(StoreError::Other(format!(
                "Scope {scope:?} is not declared for service '{service_name}'"
            )));
        }

        let server = server.clone();
        self.config_manager.save(&config)?;

        let instance_id =
            ServiceInstanceKey::new(service_name.to_string(), scope.clone()).instance_id();
        self.pool.remove(instance_id).await.ok();
        self.applied_openapi_configs
            .write()
            .await
            .remove(&instance_id);
        self.registry.unregister_instance(instance_id).await;
        self.sync_definition_projection(service_name, &server, chrono::Utc::now().timestamp())
            .await?;
        self.cache_instance_removed(instance_id).await?;
        Ok(())
    }

    pub async fn list_scope_instances(&self, scope: &ScopeRef) -> Result<Vec<ServiceInstance>> {
        self.refresh_from_db_if_needed().await?;
        let mut instances = match scope {
            ScopeRef::Store => self
                .registry
                .list_instances()
                .await
                .into_iter()
                .filter(|instance| instance.scope == ScopeRef::Store)
                .collect(),
            ScopeRef::Agent { agent_id } => self.registry.list_agent_instances(agent_id).await,
        };
        instances.sort_by(|left, right| {
            left.service_name
                .cmp(&right.service_name)
                .then_with(|| left.instance_id.cmp(&right.instance_id))
        });
        Ok(instances)
    }

    pub async fn instance_id_for_scope(
        &self,
        service_name: &str,
        scope: &ScopeRef,
    ) -> Result<InstanceId> {
        self.refresh_from_db_if_needed().await?;
        self.registry
            .instance_id(service_name, scope)
            .await
            .ok_or_else(|| {
                StoreError::Other(format!(
                    "Scope {scope:?} is not declared for service '{service_name}'"
                ))
            })
    }
}
