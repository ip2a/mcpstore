use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::config::{
    McpConfig, McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig,
};
use crate::identity::{InstanceId, ScopeRef};
use crate::overrides::{
    PromptOverridePatch, PromptOverrideRule, ResourceOverridePatch, ResourceOverrideRule,
    ResourceTemplateOverridePatch, ResourceTemplateOverrideRule, ToolOverridePatch,
    ToolOverrideRule,
};
use crate::perspective::{resolve_tool, AvailableTool};
use crate::state::ServiceState;
use crate::store::{MCPStore, Result, ScopedToolEntry};
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

#[derive(Clone)]
pub struct Service {
    context: ScopeContext,
    instance_id: InstanceId,
}

#[derive(Clone)]
pub struct Tool {
    context: ScopeContext,
    instance_id: InstanceId,
    tool_name: String,
}

#[derive(Clone)]
pub struct Prompt {
    context: ScopeContext,
    instance_id: InstanceId,
    prompt_name: String,
}

#[derive(Clone)]
pub struct Resource {
    context: ScopeContext,
    instance_id: InstanceId,
    uri: String,
}

#[derive(Clone)]
pub struct ResourceTemplate {
    context: ScopeContext,
    instance_id: InstanceId,
    uri_template: String,
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

    pub async fn reset_config(&self) -> Result<bool> {
        match self.scope {
            ScopeRef::Store => self.store.reset_config().await?,
            ScopeRef::Agent { .. } => self.store.reset_scope(&self.scope).await?,
        }
        Ok(true)
    }

    pub async fn add_service_config(
        &self,
        service_name: &str,
        mut config: ServerConfig,
    ) -> Result<Self> {
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
        Ok(self.clone())
    }

    pub async fn add_service(&self, config: McpConfig) -> Result<Self> {
        for (service_name, server_config) in config.mcp_servers {
            self.add_service_config(&service_name, server_config)
                .await?;
        }
        Ok(self.clone())
    }

    pub async fn wait_service(&self, target: ServiceTarget<'_>, timeout: Duration) -> Result<Self> {
        let (_, instance_id) = self.resolve_service(target).await?;
        self.store.wait_instance_ready(instance_id, timeout).await?;
        Ok(self.clone())
    }

    pub async fn list_services(&self) -> Result<Vec<Service>> {
        self.store
            .list_service_entries_scoped(&self.scope)
            .await?
            .into_iter()
            .map(|entry| Service::new(self.clone(), entry.instance.instance_id))
            .collect()
    }

    pub async fn find_service(&self, target: ServiceTarget<'_>) -> Result<Service> {
        let (_, instance_id) = self.resolve_service(target).await?;
        Service::new(self.clone(), instance_id)
    }

    pub async fn remove_service(&self, target: ServiceTarget<'_>) -> Result<bool> {
        let (service_name, _) = self.resolve_service(target).await?;
        self.store
            .remove_service_scope(&service_name, &self.scope)
            .await?;
        Ok(true)
    }

    pub async fn disconnect_service(&self, target: ServiceTarget<'_>) -> Result<Self> {
        let (_, instance_id) = self.resolve_service(target).await?;
        self.store.disconnect_service(instance_id).await?;
        Ok(self.clone())
    }

    pub async fn restart_service(&self, target: ServiceTarget<'_>) -> Result<Self> {
        let (_, instance_id) = self.resolve_service(target).await?;
        self.store.restart_service(instance_id).await?;
        Ok(self.clone())
    }

    pub async fn patch_service(&self, target: ServiceTarget<'_>, updates: Value) -> Result<Self> {
        let (service_name, _) = self.resolve_service(target).await?;
        self.store.patch_service(&service_name, updates).await?;
        Ok(self.clone())
    }

    pub async fn update_service(
        &self,
        target: ServiceTarget<'_>,
        config: ServerConfig,
    ) -> Result<Self> {
        let (service_name, _) = self.resolve_service(target).await?;
        self.store.update_service(&service_name, config).await?;
        Ok(self.clone())
    }

    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        self.store
            .list_tool_entries_scoped(&self.scope)
            .await?
            .into_iter()
            .map(|entry| Tool::new(self.clone(), entry.instance_id, entry.tool_name))
            .collect()
    }

    pub async fn find_tool(&self, tool_name: &str) -> Result<Tool> {
        let tool = self.resolve_tool_entry(tool_name).await?;
        Tool::new(self.clone(), tool.instance_id, tool.tool_name)
    }

    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<ToolCallResult> {
        let tool = self.resolve_tool_entry(tool_name).await?;
        self.store
            .call_tool(tool.instance_id, &tool.tool_name, args)
            .await
    }

    pub async fn list_resources(&self) -> Result<Vec<Value>> {
        self.store.list_resources_scoped(&self.scope).await
    }

    pub async fn list_resource_templates(&self) -> Result<Vec<Value>> {
        self.store.list_resource_templates_scoped(&self.scope).await
    }

    pub async fn list_prompts(&self) -> Result<Vec<Value>> {
        self.store.list_prompts_scoped(&self.scope).await
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

    async fn resolve_tool_entry(&self, tool_name: &str) -> Result<ScopedToolEntry> {
        resolve_tool_entry(
            tool_name,
            self.store.list_tool_entries_scoped(&self.scope).await?,
        )
    }
}

impl Service {
    fn new(context: ScopeContext, instance_id: InstanceId) -> Result<Self> {
        Ok(Self {
            context,
            instance_id,
        })
    }

    pub async fn info(&self) -> Result<Value> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .service_info_scoped(self.instance_id)
            .await
    }

    pub async fn state(&self) -> Result<ServiceState> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .service_state_entry(self.instance_id)
            .await
    }

    pub async fn config(&self) -> Result<Value> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let instance = self
            .context
            .store
            .find_instance(self.instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(self.instance_id.to_string()))?;
        Ok(Value::Object(instance.effective_config))
    }

    pub async fn wait_service(&self, timeout: Duration) -> Result<Self> {
        self.context
            .wait_service(ServiceTarget::InstanceId(self.instance_id), timeout)
            .await?;
        Ok(self.clone())
    }

    pub async fn disconnect_service(&self) -> Result<Self> {
        self.context
            .disconnect_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        Ok(self.clone())
    }

    pub async fn restart_service(&self) -> Result<Self> {
        self.context
            .restart_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        Ok(self.clone())
    }

    pub async fn patch_service(&self, updates: Value) -> Result<Self> {
        self.context
            .patch_service(ServiceTarget::InstanceId(self.instance_id), updates)
            .await?;
        Ok(self.clone())
    }

    pub async fn update_service(&self, config: ServerConfig) -> Result<Self> {
        self.context
            .update_service(ServiceTarget::InstanceId(self.instance_id), config)
            .await?;
        Ok(self.clone())
    }

    pub async fn remove_service(&self) -> Result<bool> {
        self.context
            .remove_service(ServiceTarget::InstanceId(self.instance_id))
            .await
    }

    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .list_tool_entries_for_instance(self.instance_id)
            .await?
            .into_iter()
            .map(|entry| Tool::new(self.context.clone(), entry.instance_id, entry.tool_name))
            .collect()
    }

    pub async fn find_tool(&self, tool_name: &str) -> Result<Tool> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let entry = resolve_tool_entry(
            tool_name,
            self.context
                .store
                .list_tool_entries_for_instance(self.instance_id)
                .await?,
        )?;
        Tool::new(self.context.clone(), entry.instance_id, entry.tool_name)
    }

    pub async fn list_resources(&self) -> Result<Vec<Value>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .list_resources_for_instance(self.instance_id)
            .await
    }

    pub async fn list_resource_templates(&self) -> Result<Vec<Value>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .list_resource_templates_for_instance(self.instance_id)
            .await
    }

    pub async fn read_resource(&self, uri: &str) -> Result<Value> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .read_resource_scoped(self.instance_id, uri)
            .await
    }

    pub async fn list_prompts(&self) -> Result<Vec<Value>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .list_prompts_for_instance(self.instance_id)
            .await
    }

    pub async fn get_prompt(&self, prompt_name: &str, arguments: Value) -> Result<Value> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .get_prompt_scoped(self.instance_id, prompt_name, arguments)
            .await
    }

    pub async fn find_prompt(&self, prompt_name: &str) -> Result<Prompt> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let found = self
            .context
            .store
            .list_prompts_for_instance(self.instance_id)
            .await?
            .into_iter()
            .any(|prompt| prompt.get("name").and_then(Value::as_str) == Some(prompt_name));
        if !found {
            return Err(StoreError::Other(format!(
                "Prompt '{prompt_name}' not found in instance {}",
                self.instance_id
            )));
        }
        Prompt::new(
            self.context.clone(),
            self.instance_id,
            prompt_name.to_string(),
        )
    }

    pub async fn find_resource(&self, uri: &str) -> Result<Resource> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let found = self
            .context
            .store
            .list_resources_for_instance(self.instance_id)
            .await?
            .into_iter()
            .any(|resource| resource.get("uri").and_then(Value::as_str) == Some(uri));
        if !found {
            return Err(StoreError::Other(format!(
                "Resource '{uri}' not found in instance {}",
                self.instance_id
            )));
        }
        Resource::new(self.context.clone(), self.instance_id, uri.to_string())
    }

    pub async fn find_resource_template(&self, uri_template: &str) -> Result<ResourceTemplate> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let found = self
            .context
            .store
            .list_resource_templates_for_instance(self.instance_id)
            .await?
            .into_iter()
            .any(|template| {
                template.get("uriTemplate").and_then(Value::as_str) == Some(uri_template)
                    || template.get("uri_template").and_then(Value::as_str) == Some(uri_template)
            });
        if !found {
            return Err(StoreError::Other(format!(
                "Resource template '{uri_template}' not found in instance {}",
                self.instance_id
            )));
        }
        ResourceTemplate::new(
            self.context.clone(),
            self.instance_id,
            uri_template.to_string(),
        )
    }

    pub async fn list_tool_overrides(&self) -> Result<Vec<ToolOverrideRule>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        Ok(self
            .context
            .store
            .list_tool_overrides()
            .await?
            .into_iter()
            .filter(|rule| rule.instance_id == self.instance_id)
            .collect())
    }

    pub async fn list_prompt_overrides(&self) -> Result<Vec<PromptOverrideRule>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        Ok(self
            .context
            .store
            .list_prompt_overrides()
            .await?
            .into_iter()
            .filter(|rule| rule.instance_id == self.instance_id)
            .collect())
    }

    pub async fn list_resource_overrides(&self) -> Result<Vec<ResourceOverrideRule>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        Ok(self
            .context
            .store
            .list_resource_overrides()
            .await?
            .into_iter()
            .filter(|rule| rule.instance_id == self.instance_id)
            .collect())
    }

    pub async fn list_resource_template_overrides(
        &self,
    ) -> Result<Vec<ResourceTemplateOverrideRule>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        Ok(self
            .context
            .store
            .list_resource_template_overrides()
            .await?
            .into_iter()
            .filter(|rule| rule.instance_id == self.instance_id)
            .collect())
    }
}

impl Tool {
    fn new(context: ScopeContext, instance_id: InstanceId, tool_name: String) -> Result<Self> {
        Ok(Self {
            context,
            instance_id,
            tool_name,
        })
    }

    pub async fn info(&self) -> Result<ScopedToolEntry> {
        self.entry().await
    }

    pub async fn call(&self, args: Value) -> Result<ToolCallResult> {
        let tool = self.entry().await?;
        self.context
            .store
            .call_tool(tool.instance_id, &tool.tool_name, args)
            .await
    }

    pub async fn set_override(&self, patch: ToolOverridePatch) -> Result<ToolOverrideRule> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .set_tool_override(self.instance_id, &self.tool_name, patch)
            .await
    }

    pub async fn get_override(&self) -> Result<Option<ToolOverrideRule>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .get_tool_override(self.instance_id, &self.tool_name)
            .await
    }

    pub async fn delete_override(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .delete_tool_override(self.instance_id, &self.tool_name)
            .await
    }

    pub async fn enable(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .enable_tool(self.instance_id, &self.tool_name)
            .await
    }

    pub async fn disable(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .disable_tool(self.instance_id, &self.tool_name)
            .await
    }

    async fn entry(&self) -> Result<ScopedToolEntry> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .list_tool_entries_for_instance(self.instance_id)
            .await?
            .into_iter()
            .find(|entry| entry.tool_name == self.tool_name)
            .ok_or_else(|| StoreError::ServiceNotFound(self.tool_name.clone()))
    }
}

impl Prompt {
    fn new(context: ScopeContext, instance_id: InstanceId, prompt_name: String) -> Result<Self> {
        Ok(Self {
            context,
            instance_id,
            prompt_name,
        })
    }

    pub async fn info(&self) -> Result<Value> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .list_prompts_for_instance(self.instance_id)
            .await?
            .into_iter()
            .find(|prompt| prompt.get("name").and_then(Value::as_str) == Some(&self.prompt_name))
            .ok_or_else(|| {
                StoreError::Other(format!(
                    "Prompt '{}' not found in instance {}",
                    self.prompt_name, self.instance_id
                ))
            })
    }

    pub async fn get(&self, args: Value) -> Result<Value> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .get_prompt_scoped(self.instance_id, &self.prompt_name, args)
            .await
    }

    pub async fn set_override(&self, patch: PromptOverridePatch) -> Result<PromptOverrideRule> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Prompt,
                self.instance_id,
                &self.prompt_name,
            )
            .await?;
        self.context
            .store
            .set_prompt_override(self.instance_id, &key, patch)
            .await
    }
    pub async fn get_override(&self) -> Result<Option<PromptOverrideRule>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Prompt,
                self.instance_id,
                &self.prompt_name,
            )
            .await?;
        self.context
            .store
            .get_prompt_override(self.instance_id, &key)
            .await
    }
    pub async fn delete_override(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Prompt,
                self.instance_id,
                &self.prompt_name,
            )
            .await?;
        self.context
            .store
            .delete_prompt_override(self.instance_id, &key)
            .await
    }
    pub async fn enable(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Prompt,
                self.instance_id,
                &self.prompt_name,
            )
            .await?;
        self.context
            .store
            .enable_prompt(self.instance_id, &key)
            .await
    }
    pub async fn disable(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Prompt,
                self.instance_id,
                &self.prompt_name,
            )
            .await?;
        self.context
            .store
            .disable_prompt(self.instance_id, &key)
            .await
    }
}

impl Resource {
    fn new(context: ScopeContext, instance_id: InstanceId, uri: String) -> Result<Self> {
        Ok(Self {
            context,
            instance_id,
            uri,
        })
    }
    pub async fn info(&self) -> Result<Value> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .list_resources_for_instance(self.instance_id)
            .await?
            .into_iter()
            .find(|resource| resource.get("uri").and_then(Value::as_str) == Some(&self.uri))
            .ok_or_else(|| {
                StoreError::Other(format!(
                    "Resource '{}' not found in instance {}",
                    self.uri, self.instance_id
                ))
            })
    }
    pub async fn read(&self) -> Result<Value> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .read_resource_scoped(self.instance_id, &self.uri)
            .await
    }
    pub async fn set_override(&self, patch: ResourceOverridePatch) -> Result<ResourceOverrideRule> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Resource,
                self.instance_id,
                &self.uri,
            )
            .await?;
        self.context
            .store
            .set_resource_override(self.instance_id, &key, patch)
            .await
    }
    pub async fn get_override(&self) -> Result<Option<ResourceOverrideRule>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Resource,
                self.instance_id,
                &self.uri,
            )
            .await?;
        self.context
            .store
            .get_resource_override(self.instance_id, &key)
            .await
    }
    pub async fn delete_override(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Resource,
                self.instance_id,
                &self.uri,
            )
            .await?;
        self.context
            .store
            .delete_resource_override(self.instance_id, &key)
            .await
    }
    pub async fn enable(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Resource,
                self.instance_id,
                &self.uri,
            )
            .await?;
        self.context
            .store
            .enable_resource(self.instance_id, &key)
            .await
    }
    pub async fn disable(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::Resource,
                self.instance_id,
                &self.uri,
            )
            .await?;
        self.context
            .store
            .disable_resource(self.instance_id, &key)
            .await
    }
}

impl ResourceTemplate {
    fn new(context: ScopeContext, instance_id: InstanceId, uri_template: String) -> Result<Self> {
        Ok(Self {
            context,
            instance_id,
            uri_template,
        })
    }
    pub async fn info(&self) -> Result<Value> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        self.context
            .store
            .list_resource_templates_for_instance(self.instance_id)
            .await?
            .into_iter()
            .find(|template| {
                template.get("uriTemplate").and_then(Value::as_str) == Some(&self.uri_template)
                    || template.get("uri_template").and_then(Value::as_str)
                        == Some(&self.uri_template)
            })
            .ok_or_else(|| {
                StoreError::Other(format!(
                    "Resource template '{}' not found in instance {}",
                    self.uri_template, self.instance_id
                ))
            })
    }
    pub async fn set_override(
        &self,
        patch: ResourceTemplateOverridePatch,
    ) -> Result<ResourceTemplateOverrideRule> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::ResourceTemplate,
                self.instance_id,
                &self.uri_template,
            )
            .await?;
        self.context
            .store
            .set_resource_template_override(self.instance_id, &key, patch)
            .await
    }
    pub async fn get_override(&self) -> Result<Option<ResourceTemplateOverrideRule>> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::ResourceTemplate,
                self.instance_id,
                &self.uri_template,
            )
            .await?;
        self.context
            .store
            .get_resource_template_override(self.instance_id, &key)
            .await
    }
    pub async fn delete_override(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::ResourceTemplate,
                self.instance_id,
                &self.uri_template,
            )
            .await?;
        self.context
            .store
            .delete_resource_template_override(self.instance_id, &key)
            .await
    }
    pub async fn enable(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::ResourceTemplate,
                self.instance_id,
                &self.uri_template,
            )
            .await?;
        self.context
            .store
            .enable_resource_template(self.instance_id, &key)
            .await
    }
    pub async fn disable(&self) -> Result<()> {
        self.context
            .resolve_service(ServiceTarget::InstanceId(self.instance_id))
            .await?;
        let key = self
            .context
            .store
            .resolve_component_override_key(
                crate::overrides::ComponentKind::ResourceTemplate,
                self.instance_id,
                &self.uri_template,
            )
            .await?;
        self.context
            .store
            .disable_resource_template(self.instance_id, &key)
            .await
    }
}

fn resolve_tool_entry(tool_name: &str, tools: Vec<ScopedToolEntry>) -> Result<ScopedToolEntry> {
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
        handshake_mode: None,
        revision: 1,
        extra: Map::new(),
    }
}
