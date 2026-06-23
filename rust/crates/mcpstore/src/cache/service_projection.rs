use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn cache_service_added(
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

    pub(crate) async fn cache_service_connected(
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

    pub(crate) async fn cache_service_removed(&self, name: &str) -> Result<()> {
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
}
