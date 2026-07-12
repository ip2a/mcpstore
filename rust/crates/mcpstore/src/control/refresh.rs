use std::collections::HashSet;

use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn load_from_db(&self) -> Result<()> {
        let services = self.cache.get_all_entities_async("services").await?;
        let cached_service_names = services.keys().cloned().collect::<HashSet<_>>();
        for service in self.registry.list_services().await {
            if !cached_service_names.contains(&service.name) {
                self.pool.remove(&service.name).await.ok();
                self.registry.unregister(&service.name).await;
            }
        }

        for (name, value) in services {
            let entity: ServiceEntity = serde_json::from_value(value).map_err(|e| {
                StoreError::Other(format!("Service entity deserialization failed: {e}"))
            })?;
            let config_value = entity.config.clone();
            let config: ServerConfig =
                serde_json::from_value(config_value.clone()).map_err(|e| {
                    StoreError::Other(format!("Service config deserialization failed: {e}"))
                })?;
            let existing = self.registry.find_service(&name).await;
            let config_changed = existing
                .as_ref()
                .map(|entry| entry.config != config_value)
                .unwrap_or(true);
            let status = existing
                .as_ref()
                .map(|entry| entry.status)
                .unwrap_or(ConnectionStatus::Disconnected);

            self.registry
                .register(ServiceEntry {
                    name: name.clone(),
                    original_name: entity.service_original_name,
                    agent_id: entity.source_agent,
                    transport: config.infer_transport().to_string(),
                    url: config.url.clone(),
                    command: config.command.clone(),
                    status,
                    tools: Vec::new(),
                    config: config_value,
                    added_time: entity.added_time,
                })
                .await;

            if config_changed {
                self.pool.remove(&name).await.ok();
                self.pool.add(name.clone(), config).await;
            }
        }

        let tool_entities = self.cache.get_all_entities_async("tools").await?;
        let service_tool_relations = self.cache.get_all_relations_async("service_tools").await?;
        for (service_name, value) in service_tool_relations {
            let relation: ServiceToolRelation = serde_json::from_value(value).map_err(|e| {
                StoreError::Other(format!("Service tool relation deserialization failed: {e}"))
            })?;
            let Some(mut entry) = self.registry.find_service(&service_name).await else {
                continue;
            };
            let mut tools = Vec::with_capacity(relation.tools.len());
            for item in relation.tools {
                let entity = tool_entities.get(&item.tool_global_name);
                let (title, description, input_schema, output_schema, annotations, meta) =
                    match entity {
                        Some(value) => {
                            let entity: ToolEntity = serde_json::from_value(value.clone())
                                .map_err(|e| {
                                    StoreError::Other(format!(
                                        "Tool entity deserialization failed: {e}"
                                    ))
                                })?;
                            (
                                entity.title,
                                entity.description,
                                entity.input_schema,
                                entity.output_schema,
                                entity.annotations,
                                entity.meta,
                            )
                        }
                        None => (
                            None,
                            String::new(),
                            serde_json::Value::Object(Default::default()),
                            None,
                            None,
                            None,
                        ),
                    };
                tools.push(crate::registry::ToolInfo {
                    name: item.tool_original_name,
                    title,
                    description,
                    input_schema,
                    output_schema,
                    annotations,
                    meta,
                });
            }
            entry.tools = tools;
            self.registry.register(entry).await;
        }

        for agent_id in self.registry.list_agent_ids().await {
            self.registry.clear_agent_scope(&agent_id).await;
        }
        let relations = self.cache.get_all_relations_async("agent_services").await?;
        for (agent_id, value) in relations {
            let relation: AgentServiceRelation = serde_json::from_value(value).map_err(|e| {
                StoreError::Other(format!("Agent relation deserialization failed: {e}"))
            })?;
            for item in relation.services {
                if self
                    .registry
                    .find_service(&item.service_global_name)
                    .await
                    .is_some()
                {
                    self.registry
                        .add_to_agent_scope(&agent_id, &item.service_global_name)
                        .await;
                }
            }
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
