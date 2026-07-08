use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn ensure_service_connected(&self, service_name: &str) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_service(service_name).await.is_none() {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        }
        if self.is_openapi_virtual_service(service_name).await? {
            self.ensure_service_auto_start_allowed(service_name).await?;
            self.connect_service_internal(service_name, true).await?;
            return Ok(());
        }
        if !self.pool.is_connected(service_name).await {
            self.ensure_service_auto_start_allowed(service_name).await?;
            self.connect_service_internal(service_name, true).await?;
        }
        Ok(())
    }

    pub(crate) async fn is_openapi_virtual_service(&self, service_name: &str) -> Result<bool> {
        let Some(service) = self.registry.find_service(service_name).await else {
            return Ok(false);
        };
        Ok(service.transport == "openapi")
    }

    pub async fn list_services(&self) -> Vec<ServiceEntry> {
        self.refresh_from_db_if_needed().await.ok();
        let mut services = self.registry.list_services().await;
        if self.source_mode == SourceMode::Db {
            for service in &mut services {
                self.hydrate_service_status_from_cache(service).await.ok();
            }
        }
        services
    }

    pub async fn find_service(&self, name: &str) -> Option<ServiceEntry> {
        self.refresh_from_db_if_needed().await.ok();
        let mut service = self.registry.find_service(name).await?;
        if self.source_mode == SourceMode::Db {
            self.hydrate_service_status_from_cache(&mut service)
                .await
                .ok();
        }
        Some(service)
    }

    pub async fn list_tools(&self, service_name: &str) -> Result<Vec<ToolDescription>> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_service(service_name).await.is_none() {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        }
        Ok(self
            .registry
            .list_service_tools(service_name)
            .await
            .into_iter()
            .map(|tool| ToolDescription {
                name: tool.name,
                description: tool.description,
                input_schema: tool.schema,
            })
            .collect())
    }

    pub async fn list_all_tools(&self) -> Vec<crate::registry::ToolInfo> {
        self.refresh_from_db_if_needed().await.ok();
        self.registry.list_all_tools().await
    }

    pub async fn list_agents(&self) -> Result<Vec<serde_json::Value>> {
        self.refresh_from_db_if_needed().await?;
        let mut agent_ids = self.registry.list_agent_ids().await;
        let cached = self.cache.get_all_relations_async("agent_services").await?;
        for agent_id in cached.keys() {
            if !agent_ids.contains(agent_id) {
                agent_ids.push(agent_id.clone());
            }
        }
        agent_ids.sort();
        agent_ids.dedup();

        let mut agents = Vec::with_capacity(agent_ids.len());
        for agent_id in agent_ids {
            agents.push(serde_json::json!({
                "agent_id": agent_id,
                "services": self.list_agent_service_names(&agent_id).await?,
            }));
        }
        Ok(agents)
    }

    pub async fn resolve_tool_for_agent(
        &self,
        agent_id: &str,
        user_input: &str,
    ) -> Result<ToolResolution> {
        self.refresh_from_db_if_needed().await?;
        let service_names = if agent_id == GLOBAL_AGENT_STORE {
            self.registry
                .list_services()
                .await
                .into_iter()
                .map(|service| service.name)
                .collect::<Vec<_>>()
        } else {
            self.list_agent_service_names(agent_id).await?
        };
        let mut available = Vec::new();
        for global_service_name in service_names {
            let service = match self.registry.find_service(&global_service_name).await {
                Some(service) => service,
                None => continue,
            };
            let local_service_name = if agent_id == GLOBAL_AGENT_STORE {
                service.name.clone()
            } else {
                service.original_name.clone()
            };
            for tool in service.tools {
                let global_tool_name = generate_tool_global_name(&service.name, &tool.name)?;
                let fallback_name = format!("{}_{}", local_service_name, tool.name);
                let display_name = self
                    .transformed_available_tool(&service.name, &tool.name, fallback_name)
                    .await?;
                available.push(AvailableTool {
                    name: display_name,
                    original_name: Some(tool.name),
                    service_name: Some(local_service_name.clone()),
                    global_service_name: Some(service.name.clone()),
                    global_tool_name: Some(global_tool_name),
                });
            }
        }
        resolve_tool(agent_id, user_input, &available, "canonical", true)
    }
}
