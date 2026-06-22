use super::*;

impl MCPStore {
    pub(super) async fn cache_service_added(
        &self,
        name: &str,
        original_name: &str,
        agent_id: &str,
        config: &ServerConfig,
        now: i64,
    ) -> Result<()> {
        let entity = ServiceEntity {
            service_global_name: name.to_string(),
            service_original_name: original_name.to_string(),
            source_agent: agent_id.to_string(),
            config: serde_json::to_value(config).unwrap_or_default(),
            added_time: now,
        };
        self.cache
            .put_entity(
                "services",
                name,
                serde_json::to_value(entity).unwrap_or_default(),
            )
            .await?;

        self.upsert_agent_service_relation(agent_id, name, now)
            .await?;
        self.set_service_status(name, HealthStatus::Init, None, Vec::new())
            .await?;
        self.cache
            .put_event(
                "service",
                &format!("{name}:added:{now}"),
                serde_json::json!({
                    "event": "service_added",
                    "service": name,
                    "timestamp": now
                }),
            )
            .await?;
        Ok(())
    }

    pub(super) async fn cache_agent_scope(&self, agent_id: &str) -> Result<()> {
        let service_names = self.registry.list_agent_services(agent_id).await;
        self.cache_agent_scope_names(agent_id, service_names).await
    }

    pub(super) async fn cache_agent_scope_names(
        &self,
        agent_id: &str,
        service_names: Vec<String>,
    ) -> Result<()> {
        let mut services = Vec::with_capacity(service_names.len());
        let now = chrono::Utc::now().timestamp();
        for service_name in service_names {
            let parsed = parse_agent_scoped(&service_name)?;
            services.push(ServiceRelationItem {
                service_original_name: parsed.local_name,
                service_global_name: service_name.clone(),
                client_id: service_name,
                established_time: now,
                last_access: Some(now),
            });
        }
        self.cache
            .put_relation(
                "agent_services",
                agent_id,
                serde_json::to_value(AgentServiceRelation { services }).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    async fn upsert_agent_service_relation(
        &self,
        agent_id: &str,
        service_name: &str,
        now: i64,
    ) -> Result<()> {
        let mut relation = match self.cache.get_relation("agent_services", agent_id).await? {
            Some(value) => serde_json::from_value(value).map_err(|e| {
                StoreError::Other(format!("Agent relation deserialization failed: {e}"))
            })?,
            None => AgentServiceRelation {
                services: Vec::new(),
            },
        };

        if !relation
            .services
            .iter()
            .any(|item| item.service_global_name == service_name)
        {
            let parsed = parse_agent_scoped(service_name)?;
            relation.services.push(ServiceRelationItem {
                service_original_name: parsed.local_name,
                service_global_name: service_name.to_string(),
                client_id: service_name.to_string(),
                established_time: now,
                last_access: Some(now),
            });
        }

        self.cache
            .put_relation(
                "agent_services",
                agent_id,
                serde_json::to_value(relation).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    pub(super) async fn cached_agent_scope(&self, agent_id: &str) -> Result<Vec<String>> {
        let value = self.cache.get_relation("agent_services", agent_id).await?;
        match value {
            Some(value) => {
                let relation: AgentServiceRelation =
                    serde_json::from_value(value).map_err(|e| {
                        StoreError::Other(format!("Agent scope deserialization failed: {e}"))
                    })?;
                Ok(relation
                    .services
                    .into_iter()
                    .map(|item| item.service_global_name)
                    .collect())
            }
            None => Ok(Vec::new()),
        }
    }

    pub(super) async fn cache_service_connected(
        &self,
        name: &str,
        tools: &[crate::registry::ToolInfo],
    ) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return Ok(());
        }

        let now = chrono::Utc::now().timestamp();
        let parsed = parse_agent_scoped(name)?;
        let source_agent = parsed
            .agent_id
            .unwrap_or_else(|| GLOBAL_AGENT_STORE.to_string());
        let service_original_name = parsed.local_name;
        let mut relation_tools = Vec::with_capacity(tools.len());
        let mut status_tools = Vec::with_capacity(tools.len());
        for tool in tools {
            let global_name = generate_tool_global_name(name, &tool.name)?;
            let entity = ToolEntity {
                tool_global_name: global_name.clone(),
                tool_original_name: tool.name.clone(),
                service_global_name: name.to_string(),
                service_original_name: service_original_name.clone(),
                source_agent: source_agent.clone(),
                description: tool.description.clone(),
                input_schema: tool.schema.clone(),
                created_time: now,
                tool_hash: format!("{}:{}:{}", name, tool.name, now),
            };
            self.cache
                .put_entity(
                    "tools",
                    &global_name,
                    serde_json::to_value(entity).unwrap_or_default(),
                )
                .await?;
            relation_tools.push(ToolRelationItem {
                tool_global_name: global_name.clone(),
                tool_original_name: tool.name.clone(),
            });
            status_tools.push(ToolStatusItem {
                tool_global_name: global_name,
                tool_original_name: tool.name.clone(),
                status: ToolAvailability::Available,
            });
        }

        let relation = ServiceToolRelation {
            service_global_name: name.to_string(),
            service_original_name,
            source_agent,
            tools: relation_tools,
        };
        self.cache
            .put_relation(
                "service_tools",
                name,
                serde_json::to_value(relation).unwrap_or_default(),
            )
            .await?;
        self.set_service_status(name, HealthStatus::Healthy, None, status_tools)
            .await?;
        self.cache
            .put_event(
                "service",
                &format!("{name}:connected:{now}"),
                serde_json::json!({
                    "event": "service_connected",
                    "service": name,
                    "timestamp": now,
                    "tools_count": tools.len()
                }),
            )
            .await?;
        Ok(())
    }

    pub(super) async fn cache_service_removed(&self, name: &str) -> Result<()> {
        self.cache.delete_entity("services", name).await?;
        self.cache.delete_relation("service_tools", name).await?;
        self.cache.delete_state("service_status", name).await?;
        self.remove_service_from_agent_relations(name).await?;
        let now = chrono::Utc::now().timestamp();
        self.cache
            .put_event(
                "service",
                &format!("{name}:removed:{now}"),
                serde_json::json!({
                    "event": "service_removed",
                    "service": name,
                    "timestamp": now
                }),
            )
            .await?;
        Ok(())
    }

    async fn remove_service_from_agent_relations(&self, name: &str) -> Result<()> {
        let relations = self.cache.get_all_relations_async("agent_services").await?;
        for (agent_id, value) in relations {
            let mut relation: AgentServiceRelation =
                serde_json::from_value(value).map_err(|e| {
                    StoreError::Other(format!("Agent relation deserialization failed: {e}"))
                })?;
            let original_len = relation.services.len();
            relation.services.retain(|item| {
                item.service_global_name != name && item.service_original_name != name
            });

            if relation.services.is_empty() {
                self.cache
                    .delete_relation("agent_services", &agent_id)
                    .await?;
            } else if relation.services.len() != original_len {
                self.cache
                    .put_relation(
                        "agent_services",
                        &agent_id,
                        serde_json::to_value(relation).unwrap_or_default(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}
