use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::config::{
    McpConfig, McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig,
};
use crate::identity::{InstanceId, ScopeRef};
use crate::perspective::{resolve_tool, AvailableTool};
use crate::state::ServiceState;
use crate::store::{MCPStore, Result, ScopedServiceEntry, ScopedToolEntry};
use crate::transport::ToolCallResult;
use crate::StoreError;

#[derive(Clone, Copy)]
pub enum ServiceTarget<'a> {
    ServiceName(&'a str),
    InstanceId(InstanceId),
}

#[derive(Clone)]
pub struct ScopeContext {
    store: Arc<MCPStore>,
    scope: ScopeRef,
}

impl ScopeContext {
    pub(crate) fn new(store: Arc<MCPStore>, scope: ScopeRef) -> Self {
        Self { store, scope }
    }

    pub fn scope(&self) -> &ScopeRef {
        &self.scope
    }

    pub async fn show_config(&self) -> Result<Value> {
        self.store.show_scope_config(&self.scope).await
    }

    pub async fn reset_config(&self) -> Result<()> {
        match self.scope {
            ScopeRef::Store => self.store.reset_config().await,
            ScopeRef::Agent { .. } => self.store.reset_scope(&self.scope).await,
        }
    }

    pub async fn add_service_config(
        &self,
        service_name: &str,
        mut config: ServerConfig,
    ) -> Result<InstanceId> {
        if config.mcpstore.is_none() {
            config.mcpstore = Some(extension_for_scope(&self.scope));
        }
        let existing = self
            .store
            .get_definition_config(service_name)
            .await?
            .map(serde_json::from_value::<ServerConfig>)
            .transpose()
            .map_err(|error| StoreError::Other(error.to_string()))?;

        if let Some(existing) = existing {
            if existing.base_config() != config.base_config() {
                return Err(StoreError::Other(format!(
                    "Service definition already exists with a different base config: {service_name}"
                )));
            }
            if declares_scope(&existing, &self.scope) {
                return Err(StoreError::Other(format!(
                    "Scope {:?} is already declared for service '{service_name}'",
                    self.scope
                )));
            }
            self.store
                .declare_service_scope(service_name, &self.scope, ScopeDescriptor::default())
                .await?;
        } else {
            self.store.add_service(service_name, config).await?;
        }
        self.store
            .instance_id_for_scope(service_name, &self.scope)
            .await
    }

    pub async fn add_service(&self, config: McpConfig) -> Result<Vec<InstanceId>> {
        let mut instance_ids = Vec::with_capacity(config.mcp_servers.len());
        for (service_name, server_config) in config.mcp_servers {
            instance_ids.push(
                self.add_service_config(&service_name, server_config)
                    .await?,
            );
        }
        Ok(instance_ids)
    }

    pub async fn wait_service(
        &self,
        target: ServiceTarget<'_>,
        timeout: Duration,
    ) -> Result<ServiceState> {
        let (_, instance_id) = self.resolve_service(target).await?;
        self.store.wait_instance_ready(instance_id, timeout).await
    }

    pub async fn list_services(&self) -> Result<Vec<ScopedServiceEntry>> {
        self.store.list_service_entries_scoped(&self.scope).await
    }

    pub async fn find_service(&self, target: ServiceTarget<'_>) -> Result<ScopedServiceEntry> {
        let (_, instance_id) = self.resolve_service(target).await?;
        self.store
            .list_service_entries_scoped(&self.scope)
            .await?
            .into_iter()
            .find(|entry| entry.instance.instance_id == instance_id)
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))
    }

    pub async fn remove_service(&self, target: ServiceTarget<'_>) -> Result<()> {
        let (service_name, _) = self.resolve_service(target).await?;
        self.store
            .remove_service_scope(&service_name, &self.scope)
            .await
    }

    pub async fn disconnect_service(&self, target: ServiceTarget<'_>) -> Result<()> {
        let (_, instance_id) = self.resolve_service(target).await?;
        self.store.disconnect_service(instance_id).await
    }

    pub async fn restart_service(&self, target: ServiceTarget<'_>) -> Result<()> {
        let (_, instance_id) = self.resolve_service(target).await?;
        self.store.restart_service(instance_id).await
    }

    pub async fn patch_service(&self, target: ServiceTarget<'_>, updates: Value) -> Result<()> {
        let (service_name, _) = self.resolve_service(target).await?;
        self.store.patch_service(&service_name, updates).await
    }

    pub async fn update_service(
        &self,
        target: ServiceTarget<'_>,
        config: ServerConfig,
    ) -> Result<()> {
        let (service_name, _) = self.resolve_service(target).await?;
        self.store.update_service(&service_name, config).await
    }

    async fn resolve_service(&self, target: ServiceTarget<'_>) -> Result<(String, InstanceId)> {
        match target {
            ServiceTarget::ServiceName(service_name) => Ok((
                service_name.to_string(),
                self.store
                    .instance_id_for_scope(service_name, &self.scope)
                    .await?,
            )),
            ServiceTarget::InstanceId(instance_id) => {
                let instance = self
                    .store
                    .find_instance(instance_id)
                    .await
                    .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
                if instance.scope != self.scope {
                    return Err(StoreError::Other(format!(
                        "Instance {instance_id} does not belong to scope {:?}",
                        self.scope
                    )));
                }
                Ok((instance.service_name, instance_id))
            }
        }
    }

    pub async fn list_tools(&self) -> Result<Vec<ScopedToolEntry>> {
        self.store.list_tool_entries_scoped(&self.scope).await
    }

    pub async fn find_tool(&self, tool_name: &str) -> Result<ScopedToolEntry> {
        self.resolve_tool_entry(tool_name).await
    }

    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<ToolCallResult> {
        let tool = self.resolve_tool_entry(tool_name).await?;
        self.store
            .call_tool(tool.instance_id, &tool.tool_name, args)
            .await
    }

    async fn resolve_tool_entry(&self, tool_name: &str) -> Result<ScopedToolEntry> {
        let tools = self.store.list_tool_entries_scoped(&self.scope).await?;
        let available_tools = tools
            .iter()
            .map(|tool| AvailableTool {
                instance_id: tool.instance_id,
                service_name: tool.service_name.clone(),
                scope: tool.scope.clone(),
                tool_name: tool.tool_name.clone(),
                name: tool.name.clone(),
            })
            .collect::<Vec<_>>();
        let resolution = resolve_tool(tool_name, &available_tools)?;
        tools
            .into_iter()
            .find(|tool| {
                tool.instance_id == resolution.instance_id && tool.tool_name == resolution.tool_name
            })
            .ok_or_else(|| StoreError::ServiceNotFound(resolution.tool_name))
    }
}

impl MCPStore {
    pub fn for_store(self: &Arc<Self>) -> ScopeContext {
        ScopeContext::new(Arc::clone(self), ScopeRef::Store)
    }

    pub fn for_agent(self: &Arc<Self>, agent_id: impl Into<String>) -> ScopeContext {
        ScopeContext::new(
            Arc::clone(self),
            ScopeRef::Agent {
                agent_id: agent_id.into(),
            },
        )
    }
}

fn declares_scope(config: &ServerConfig, scope: &ScopeRef) -> bool {
    config
        .mcpstore
        .as_ref()
        .map(|extension| extension.scopes.descriptor(scope).is_some())
        .unwrap_or(false)
}

fn extension_for_scope(scope: &ScopeRef) -> McpStoreExtension {
    let mut scopes = ScopeDeclarations::default();
    match scope {
        ScopeRef::Store => scopes.store = Some(ScopeDescriptor::default()),
        ScopeRef::Agent { agent_id } => {
            scopes
                .agents
                .insert(agent_id.clone(), ScopeDescriptor::default());
        }
    }
    McpStoreExtension {
        scopes,
        lifecycle: None,
        revision: 1,
        extra: Map::new(),
    }
}
