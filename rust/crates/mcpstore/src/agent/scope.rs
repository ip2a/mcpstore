use std::collections::HashMap;

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
        if self.is_data_plane() {
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
        // handshake_mode is a definition-level override, not per-scope: pull it
        // out of the descriptor before storing so it does not leak into scope
        // state, then apply it to the definition extension below.
        let handshake_override = descriptor.handshake_mode.take();
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
        if let Some(mode) = handshake_override {
            extension.handshake_mode = Some(mode);
        }
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

    pub async fn remove_service_scope(&self, service_name: &str, scope: &ScopeRef) -> Result<String> {
        if self.is_data_plane() {
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
        self.auth_coordinator.remove_status(instance_id).await;
        self.sync_definition_projection(service_name, &server, chrono::Utc::now().timestamp())
            .await?;
        self.cache_instance_removed(instance_id).await?;
        Ok(String::new())
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

    /// 作用域注册表：root + store + 各 agent，每项带运行时服务数（来自 registry）。
    pub async fn list_scopes(&self) -> Result<Vec<ScopeSummary>> {
        self.refresh_from_db_if_needed().await?;
        let instances = self.registry.list_instances().await;
        let mut agent_counts: HashMap<String, usize> = HashMap::new();
        let mut store_count = 0usize;
        for instance in &instances {
            match &instance.scope {
                ScopeRef::Store => store_count += 1,
                ScopeRef::Agent { agent_id } => {
                    *agent_counts.entry(agent_id.clone()).or_default() += 1;
                }
            }
        }
        let mut summaries = Vec::with_capacity(2 + agent_counts.len());
        summaries.push(ScopeSummary {
            scope: ScopeView::Root,
            service_count: instances.len(),
        });
        summaries.push(ScopeSummary {
            scope: ScopeView::Store,
            service_count: store_count,
        });
        let mut agent_entries: Vec<(String, usize)> = agent_counts.into_iter().collect();
        agent_entries.sort_by(|left, right| left.0.cmp(&right.0));
        for (agent_id, service_count) in agent_entries {
            summaries.push(ScopeSummary {
                scope: ScopeView::Agent { agent_id },
                service_count,
            });
        }
        Ok(summaries)
    }

    /// 单个作用域摘要（root = 全部服务的聚合视图）；未声明返回 None。
    pub async fn scope_info(&self, view: &ScopeView) -> Result<Option<ScopeSummary>> {
        let scopes = self.list_scopes().await?;
        Ok(scopes.into_iter().find(|summary| &summary.scope == view))
    }

    /// 单个 agent 实体（agent_id + 其下实例 id）；不存在返回 None。
    pub async fn find_agent(&self, agent_id: &str) -> Result<Option<AgentInfo>> {
        self.refresh_from_db_if_needed().await?;
        let instance_ids = self.registry.list_agent_instance_ids(agent_id).await;
        if instance_ids.is_empty() {
            return Ok(None);
        }
        Ok(Some(AgentInfo {
            agent_id: agent_id.to_string(),
            instance_ids,
        }))
    }
}
