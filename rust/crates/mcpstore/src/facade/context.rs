use std::sync::Arc;

use serde_json::{Map, Value};

use crate::config::{
    McpConfig, McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig,
};
use crate::identity::{InstanceId, ScopeRef};
use crate::perspective::{resolve_tool, AvailableTool};
use crate::state::ServiceState;
use crate::store::{MCPStore, Result, ScopedServiceEntry, ScopedToolEntry};
use crate::transport::ToolCallResult;

#[derive(Clone)]
pub struct StoreContextFacade {
    store: Arc<MCPStore>,
    scope: ScopeRef,
}

impl StoreContextFacade {
    pub(crate) fn new(store: Arc<MCPStore>, scope: ScopeRef) -> Self {
        Self { store, scope }
    }

    pub fn scope(&self) -> &ScopeRef {
        &self.scope
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
        let declares_current_scope = declares_scope(&config, &self.scope);

        self.store.add_service(service_name, config).await?;
        if !declares_current_scope {
            self.store
                .declare_service_scope(service_name, &self.scope, ScopeDescriptor::default())
                .await?;
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
        service_name: &str,
        timeout_secs: u64,
    ) -> Result<ServiceState> {
        let instance_id = self
            .store
            .instance_id_for_scope(service_name, &self.scope)
            .await?;
        self.store
            .wait_instance_ready(instance_id, timeout_secs)
            .await
    }

    pub async fn list_services(&self) -> Result<Vec<ScopedServiceEntry>> {
        self.store.list_service_entries_scoped(&self.scope).await
    }

    pub async fn list_tools(&self) -> Result<Vec<ScopedToolEntry>> {
        self.store.list_tool_entries_scoped(&self.scope).await
    }

    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<ToolCallResult> {
        let tools = self.store.list_tool_entries_scoped(&self.scope).await?;
        let available_tools = tools
            .into_iter()
            .map(|tool| AvailableTool {
                instance_id: tool.instance_id,
                service_name: tool.service_name,
                scope: tool.scope,
                tool_name: tool.tool_name,
                name: tool.name,
            })
            .collect::<Vec<_>>();
        let resolution = resolve_tool(tool_name, &available_tools)?;
        self.store
            .call_tool(resolution.instance_id, &resolution.tool_name, args)
            .await
    }
}

impl MCPStore {
    pub fn for_store(self: &Arc<Self>) -> StoreContextFacade {
        StoreContextFacade::new(Arc::clone(self), ScopeRef::Store)
    }

    pub fn for_agent(self: &Arc<Self>, agent_id: impl Into<String>) -> StoreContextFacade {
        StoreContextFacade::new(
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
