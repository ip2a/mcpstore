use std::hash::{Hash, Hasher};

use crate::cache::models::{
    HealthStatus, InstanceStatus, InstanceToolRelation, ServiceDefinitionEntity,
    ServiceInstanceEntity, ServiceLifecycleState, ToolAvailability, ToolEntity, ToolStatusItem,
};
use crate::identity::{InstanceId, ScopeRef};
use crate::registry::{ServiceDefinition, ServiceInstance, ToolInfo};
use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn cache_instance_added(&self, instance_id: InstanceId) -> Result<()> {
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

        self.cache_definition(&definition).await?;
        self.cache
            .put_entity(
                "service_instances",
                &instance_id.to_string(),
                serde_json::to_value(Self::instance_entity(&instance)).unwrap_or_default(),
            )
            .await?;

        if let ScopeRef::Agent { agent_id } = &instance.scope {
            self.upsert_agent_instance_relation(agent_id, &instance, instance.added_time)
                .await?;
        }

        if self.cached_instance_status(instance_id).await?.is_none() {
            let status =
                self.instance_status_payload(&instance, HealthStatus::Init, None, Vec::new());
            self.put_instance_status(&status).await?;
        }
        self.cache
            .put_event(
                "instance",
                &format!("{instance_id}:added:{}", instance.added_time),
                serde_json::json!({
                    "event": "instance_added",
                    "instance_id": instance_id,
                    "service_name": instance.service_name,
                    "scope": instance.scope,
                    "timestamp": instance.added_time
                }),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn cache_instance_connected(
        &self,
        instance_id: InstanceId,
        tools: &[ToolInfo],
    ) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return Ok(());
        }

        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let now = chrono::Utc::now().timestamp();
        let mut relation_tools = Vec::with_capacity(tools.len());
        let mut status_tools = Vec::with_capacity(tools.len());
        for tool in tools {
            let entity = ToolEntity {
                instance_id,
                service_name: instance.service_name.clone(),
                scope: instance.scope.clone(),
                tool_name: tool.name.clone(),
                title: tool.title.clone(),
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(),
                output_schema: tool.output_schema.clone(),
                annotations: tool.annotations.clone(),
                meta: tool.meta.clone(),
                created_time: now,
                tool_hash: Self::tool_content_hash(instance_id, tool),
            };
            self.cache
                .put_entity(
                    "tools",
                    &Self::instance_tool_key(instance_id, &tool.name),
                    serde_json::to_value(entity).unwrap_or_default(),
                )
                .await?;
            relation_tools.push(tool.name.clone());
            status_tools.push(ToolStatusItem {
                tool_name: tool.name.clone(),
                status: ToolAvailability::Available,
            });
        }

        let relation = InstanceToolRelation {
            instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            tools: relation_tools,
        };
        self.cache
            .put_relation(
                "instance_tools",
                &instance_id.to_string(),
                serde_json::to_value(relation).unwrap_or_default(),
            )
            .await?;
        let status =
            self.instance_status_payload(&instance, HealthStatus::Healthy, None, status_tools);
        self.put_instance_status(&status).await?;
        self.cache
            .put_event(
                "instance",
                &format!("{instance_id}:connected:{now}"),
                serde_json::json!({
                    "event": "instance_connected",
                    "instance_id": instance_id,
                    "service_name": instance.service_name,
                    "scope": instance.scope,
                    "timestamp": now,
                    "tools_count": tools.len()
                }),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn cache_instance_removed(&self, instance_id: InstanceId) -> Result<()> {
        if let Some(value) = self
            .cache
            .get_relation("instance_tools", &instance_id.to_string())
            .await?
        {
            let relation: InstanceToolRelation =
                serde_json::from_value(value).map_err(|error| {
                    StoreError::Other(format!(
                        "Instance tool relation deserialization failed: {error}"
                    ))
                })?;
            for tool_name in relation.tools {
                self.cache
                    .delete_entity("tools", &Self::instance_tool_key(instance_id, &tool_name))
                    .await?;
            }
        }

        self.cache
            .delete_entity("service_instances", &instance_id.to_string())
            .await?;
        self.cache
            .delete_relation("instance_tools", &instance_id.to_string())
            .await?;
        self.cache
            .delete_state("instance_status", &instance_id.to_string())
            .await?;
        self.remove_instance_from_agent_relations(instance_id)
            .await?;

        let now = chrono::Utc::now().timestamp();
        self.cache
            .put_event(
                "instance",
                &format!("{instance_id}:removed:{now}"),
                serde_json::json!({
                    "event": "instance_removed",
                    "instance_id": instance_id,
                    "timestamp": now
                }),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn cache_definition_removed(&self, service_name: &str) -> Result<()> {
        self.cache
            .delete_entity("service_definitions", service_name)
            .await?;
        Ok(())
    }

    pub(crate) async fn cached_instance_status(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<InstanceStatus>> {
        let value = self
            .cache
            .get_state("instance_status", &instance_id.to_string())
            .await?;
        match value {
            Some(value) => Ok(Some(serde_json::from_value(value).map_err(|error| {
                StoreError::Other(format!("Instance status deserialization failed: {error}"))
            })?)),
            None => Ok(None),
        }
    }

    pub(crate) async fn put_instance_status(&self, status: &InstanceStatus) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return Ok(());
        }
        self.cache
            .put_state(
                "instance_status",
                &status.instance_id.to_string(),
                serde_json::to_value(status).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    async fn cache_definition(&self, definition: &ServiceDefinition) -> Result<()> {
        let entity = ServiceDefinitionEntity::from(definition);
        self.cache
            .put_entity(
                "service_definitions",
                &definition.service_name,
                serde_json::to_value(entity).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    fn instance_entity(instance: &ServiceInstance) -> ServiceInstanceEntity {
        ServiceInstanceEntity {
            instance_id: instance.instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            transport: instance.transport.clone(),
            url: instance.url.clone(),
            command: instance.command.clone(),
            effective_config: instance.effective_config.clone(),
            config_revision: instance.config_revision,
            applied_config_revision: instance.applied_config_revision,
            added_time: instance.added_time,
        }
    }

    fn instance_status_payload(
        &self,
        instance: &ServiceInstance,
        health_status: HealthStatus,
        error: Option<String>,
        tools: Vec<ToolStatusItem>,
    ) -> InstanceStatus {
        InstanceStatus {
            instance_id: instance.instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            health_status,
            last_health_check: chrono::Utc::now().timestamp(),
            connection_attempts: 0,
            max_connection_attempts: self.runtime_config.max_connection_attempts,
            current_error: error,
            tools,
            window_error_rate: None,
            latency_p95: None,
            latency_p99: None,
            sample_size: None,
            next_retry_time: None,
            hard_deadline: None,
            lease_deadline: None,
            lifecycle_state: ServiceLifecycleState::default(),
        }
    }

    fn instance_tool_key(instance_id: InstanceId, tool_name: &str) -> String {
        format!("{instance_id}:{tool_name}")
    }

    fn tool_content_hash(instance_id: InstanceId, tool: &ToolInfo) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        instance_id.hash(&mut hasher);
        tool.name.hash(&mut hasher);
        tool.description.hash(&mut hasher);
        serde_json::to_string(&tool.input_schema)
            .unwrap_or_default()
            .hash(&mut hasher);
        serde_json::to_string(&tool.output_schema)
            .unwrap_or_default()
            .hash(&mut hasher);
        serde_json::to_string(&tool.annotations)
            .unwrap_or_default()
            .hash(&mut hasher);
        serde_json::to_string(&tool.meta)
            .unwrap_or_default()
            .hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}
