use std::collections::HashMap;

use crate::cache::models::{
    HealthStatus, InstanceStatus, InstanceToolRelation, ServiceDefinitionEntity,
    ServiceInstanceEntity, ToolEntity,
};
use crate::config::ServerConfig;
use crate::registry::{ConnectionStatus, ServiceDefinition, ServiceInstance, ToolInfo};
use crate::store::{MCPStore, SourceMode};
use crate::{Result, ServiceInstanceKey, StoreError};

impl MCPStore {
    pub(crate) async fn load_from_db(&self) -> Result<()> {
        let definition_values = self
            .cache
            .get_all_entities_async("service_definitions")
            .await?;
        let instance_values = self
            .cache
            .get_all_entities_async("service_instances")
            .await?;
        let tool_values = self.cache.get_all_entities_async("tools").await?;
        let tool_relation_values = self.cache.get_all_relations_async("instance_tools").await?;
        let status_values = self.cache.get_all_states_async("instance_status").await?;

        let mut definitions = HashMap::with_capacity(definition_values.len());
        for (key, value) in definition_values {
            let entity: ServiceDefinitionEntity =
                serde_json::from_value(value).map_err(|error| {
                    StoreError::Other(format!(
                        "Service definition entity deserialization failed: {error}"
                    ))
                })?;
            if key != entity.service_name {
                return Err(StoreError::Other(format!(
                    "Service definition cache key '{key}' does not match service_name '{}'",
                    entity.service_name
                )));
            }
            definitions.insert(key, ServiceDefinition::from(entity));
        }

        let mut tool_entities = HashMap::with_capacity(tool_values.len());
        for (key, value) in tool_values {
            let entity: ToolEntity = serde_json::from_value(value).map_err(|error| {
                StoreError::Other(format!("Tool entity deserialization failed: {error}"))
            })?;
            let expected_key = format!("{}:{}", entity.instance_id, entity.tool_name);
            if key != expected_key {
                return Err(StoreError::Other(format!(
                    "Tool cache key '{key}' does not match instance/tool identity '{expected_key}'"
                )));
            }
            tool_entities.insert(key, entity);
        }

        let mut tool_relations = HashMap::with_capacity(tool_relation_values.len());
        for (key, value) in tool_relation_values {
            let relation: InstanceToolRelation =
                serde_json::from_value(value).map_err(|error| {
                    StoreError::Other(format!(
                        "Instance tool relation deserialization failed: {error}"
                    ))
                })?;
            if key != relation.instance_id.to_string() {
                return Err(StoreError::Other(format!(
                    "Instance tool relation key '{key}' does not match instance_id '{}'",
                    relation.instance_id
                )));
            }
            tool_relations.insert(relation.instance_id, relation);
        }

        let mut statuses = HashMap::with_capacity(status_values.len());
        for (key, value) in status_values {
            let status: InstanceStatus = serde_json::from_value(value).map_err(|error| {
                StoreError::Other(format!("Instance status deserialization failed: {error}"))
            })?;
            if key != status.instance_id.to_string() {
                return Err(StoreError::Other(format!(
                    "Instance status key '{key}' does not match instance_id '{}'",
                    status.instance_id
                )));
            }
            statuses.insert(status.instance_id, status);
        }

        let mut instances = Vec::with_capacity(instance_values.len());
        for (key, value) in instance_values {
            let entity: ServiceInstanceEntity = serde_json::from_value(value).map_err(|error| {
                StoreError::Other(format!(
                    "Service instance entity deserialization failed: {error}"
                ))
            })?;
            if key != entity.instance_id.to_string() {
                return Err(StoreError::Other(format!(
                    "Service instance cache key '{key}' does not match instance_id '{}'",
                    entity.instance_id
                )));
            }
            let definition = definitions.get(&entity.service_name).ok_or_else(|| {
                StoreError::Other(format!(
                    "Service instance '{}' references missing definition '{}'",
                    entity.instance_id, entity.service_name
                ))
            })?;
            if definition.scopes.descriptor(&entity.scope).is_none() {
                return Err(StoreError::Other(format!(
                    "Service instance '{}' uses undeclared scope {:?}",
                    entity.instance_id, entity.scope
                )));
            }
            let expected_id =
                ServiceInstanceKey::new(&entity.service_name, entity.scope.clone()).instance_id();
            if entity.instance_id != expected_id {
                return Err(StoreError::Other(format!(
                    "Service instance '{}' does not match deterministic identity '{}'",
                    entity.instance_id, expected_id
                )));
            }

            let tools = match tool_relations.get(&entity.instance_id) {
                Some(relation) => {
                    if relation.service_name != entity.service_name
                        || relation.scope != entity.scope
                    {
                        return Err(StoreError::Other(format!(
                            "Instance tool relation '{}' identity does not match its instance",
                            entity.instance_id
                        )));
                    }
                    let mut tools = Vec::with_capacity(relation.tools.len());
                    for tool_name in &relation.tools {
                        let tool_key = format!("{}:{tool_name}", entity.instance_id);
                        let tool = tool_entities.get(&tool_key).ok_or_else(|| {
                            StoreError::Other(format!(
                                "Instance '{}' references missing tool '{tool_name}'",
                                entity.instance_id
                            ))
                        })?;
                        if tool.instance_id != entity.instance_id
                            || tool.service_name != entity.service_name
                            || tool.scope != entity.scope
                            || tool.tool_name != *tool_name
                        {
                            return Err(StoreError::Other(format!(
                                "Tool '{tool_key}' identity does not match its instance relation"
                            )));
                        }
                        tools.push(ToolInfo {
                            name: tool.tool_name.clone(),
                            title: tool.title.clone(),
                            description: tool.description.clone(),
                            input_schema: tool.input_schema.clone(),
                            output_schema: tool.output_schema.clone(),
                            annotations: tool.annotations.clone(),
                            meta: tool.meta.clone(),
                        });
                    }
                    tools
                }
                None => Vec::new(),
            };

            let status = statuses
                .get(&entity.instance_id)
                .map(|status| {
                    if status.service_name != entity.service_name || status.scope != entity.scope {
                        return Err(StoreError::Other(format!(
                            "Instance status '{}' identity does not match its instance",
                            entity.instance_id
                        )));
                    }
                    Ok(match status.health_status {
                        HealthStatus::Healthy | HealthStatus::Ready => ConnectionStatus::Connected,
                        HealthStatus::Startup | HealthStatus::HalfOpen => {
                            ConnectionStatus::Connecting
                        }
                        HealthStatus::Init | HealthStatus::Disconnected => {
                            ConnectionStatus::Disconnected
                        }
                        HealthStatus::Degraded | HealthStatus::CircuitOpen => {
                            ConnectionStatus::Error
                        }
                    })
                })
                .transpose()?
                .unwrap_or(ConnectionStatus::Disconnected);
            let transport_config: ServerConfig =
                serde_json::from_value(serde_json::Value::Object(entity.effective_config.clone()))
                    .map_err(|error| {
                        StoreError::Other(format!(
                            "Service instance '{}' effective config cannot be decoded: {error}",
                            entity.instance_id
                        ))
                    })?;
            instances.push((
                ServiceInstance {
                    instance_id: entity.instance_id,
                    service_name: entity.service_name,
                    scope: entity.scope,
                    transport: entity.transport,
                    url: entity.url,
                    command: entity.command,
                    status,
                    tools,
                    effective_config: entity.effective_config,
                    config_revision: entity.config_revision,
                    applied_config_revision: entity.applied_config_revision,
                    added_time: entity.added_time,
                },
                transport_config,
            ));
        }

        self.pool.clear().await;
        self.applied_openapi_configs.write().await.clear();
        self.registry.clear().await;
        for definition in definitions.into_values() {
            self.registry.register_definition(definition).await;
        }
        for (instance, transport_config) in instances {
            let instance_id = instance.instance_id;
            self.pool.add(instance_id, transport_config).await;
            self.registry.register_instance(instance).await;
        }
        Ok(())
    }

    pub(crate) async fn refresh_from_db_if_needed(&self) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            self.load_from_db().await?;
        }
        Ok(())
    }
}
