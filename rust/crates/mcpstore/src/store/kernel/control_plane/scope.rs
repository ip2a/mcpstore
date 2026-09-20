use serde_json::Value;

use crate::config::ScopeDescriptor;
use crate::store::prelude::*;
use crate::store::{ControlPlane, MCPStore};

impl ControlPlane {
    pub async fn declare_service_scope(
        &self,
        store: &MCPStore,
        service_name: &str,
        scope: &ScopeRef,
        mut descriptor: ScopeDescriptor,
    ) -> Result<InstanceId> {
        let instance_id =
            ServiceInstanceKey::new(service_name.to_string(), scope.clone()).instance_id();
        if store.is_data_plane() {
            store
                .queue_control_request(
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

        let mut config = store.show_config_entry().await?;
        let server = config
            .mcp_servers
            .get_mut(service_name)
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, service_name.to_string()))?;
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
        if store.kernel.runtime.source_mode == SourceMode::Local {
            store.kernel.control.config_manager.save(&config)?;
        }

        let effective_config = server
            .effective_config(scope)
            .map_err(|message| Error::new(FailureCode::ConfigInvalid, message))?;
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
        let previous = store
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await;
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
        store
            .kernel
            .control
            .registry
            .register_instance(instance)
            .await;
        store
            .sync_definition_projection(service_name, &server, now)
            .await?;
        store.cache_instance_added(instance_id).await?;
        Ok(instance_id)
    }

    pub async fn remove_service_scope(
        &self,
        store: &MCPStore,
        service_name: &str,
        scope: &ScopeRef,
    ) -> Result<String> {
        if store.is_data_plane() {
            return store
                .queue_control_request(
                    "ServiceScopeRemoveRequested",
                    serde_json::json!({
                        "service_name": service_name,
                        "scope": scope,
                    }),
                )
                .await;
        }

        let mut config = store.show_config_entry().await?;
        let server = config
            .mcp_servers
            .get_mut(service_name)
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, service_name.to_string()))?;
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
            return Err(Error::new(
                FailureCode::Internal,
                format!("Scope {scope:?} is not declared for service '{service_name}'"),
            ));
        }

        let server = server.clone();
        if store.kernel.runtime.source_mode == SourceMode::Local {
            store.kernel.control.config_manager.save(&config)?;
        }

        let instance_id =
            ServiceInstanceKey::new(service_name.to_string(), scope.clone()).instance_id();
        store.kernel.execution.pool.remove(instance_id).await.ok();
        store
            .kernel
            .runtime
            .applied_openapi_configs
            .write()
            .await
            .remove(&instance_id);
        store
            .kernel
            .control
            .registry
            .unregister_instance(instance_id)
            .await;
        store.kernel.control.auth.remove_status(instance_id).await;
        store
            .sync_definition_projection(service_name, &server, chrono::Utc::now().timestamp())
            .await?;
        store.cache_instance_removed(instance_id).await?;
        Ok(String::new())
    }
}
