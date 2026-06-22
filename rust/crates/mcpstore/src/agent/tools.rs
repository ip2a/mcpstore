use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_tools_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        self.refresh_from_db_if_needed().await?;
        match (agent_id, service_name) {
            (None, Some(service_name)) => {
                let tools = self.list_tools(service_name).await?;
                let mut payload = Vec::with_capacity(tools.len());
                for tool in tools {
                    payload.push(Self::tool_payload_value(
                        tool.name.clone(),
                        tool.name,
                        service_name.to_string(),
                        service_name.to_string(),
                        tool.description,
                        tool.input_schema,
                    )?);
                }
                Ok(payload)
            }
            (None, None) => self.collect_store_tools_scoped().await,
            (Some(agent_id), Some(service_name)) => {
                let global_service_name = self
                    .resolve_service_name_for_agent(agent_id, service_name)
                    .await?;
                let service = self
                    .find_service(&global_service_name)
                    .await
                    .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
                let tools = self.list_tools(&global_service_name).await?;
                let mut payload = Vec::with_capacity(tools.len());
                for tool in tools {
                    payload.push(Self::tool_payload_value(
                        tool.name.clone(),
                        tool.name,
                        service.original_name.clone(),
                        global_service_name.clone(),
                        tool.description,
                        tool.input_schema,
                    )?);
                }
                Ok(payload)
            }
            (Some(agent_id), None) => self.collect_agent_tools_scoped(agent_id).await,
        }
    }

    pub async fn list_tool_entries_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<ScopedToolEntry>> {
        self.refresh_from_db_if_needed().await?;
        match (agent_id, service_name) {
            (None, Some(service_name)) => {
                let mut tools = self.list_tools(service_name).await?;
                tools.sort_by(|left, right| left.name.cmp(&right.name));
                tools
                    .into_iter()
                    .map(|tool| {
                        Self::scoped_tool_entry(
                            tool.name.clone(),
                            tool.name,
                            service_name.to_string(),
                            service_name.to_string(),
                            tool.description,
                            tool.input_schema,
                        )
                    })
                    .collect()
            }
            (None, None) => self.collect_store_tool_descriptions_scoped().await,
            (Some(agent_id), Some(service_name)) => {
                let global_service_name = self
                    .resolve_service_name_for_agent(agent_id, service_name)
                    .await?;
                let service = self
                    .find_service(&global_service_name)
                    .await
                    .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
                let mut tools = self.list_tools(&global_service_name).await?;
                tools.sort_by(|left, right| left.name.cmp(&right.name));
                tools
                    .into_iter()
                    .map(|tool| {
                        Self::scoped_tool_entry(
                            tool.name.clone(),
                            tool.name,
                            service.original_name.clone(),
                            global_service_name.clone(),
                            tool.description,
                            tool.input_schema,
                        )
                    })
                    .collect()
            }
            (Some(agent_id), None) => self.collect_agent_tool_descriptions_scoped(agent_id).await,
        }
    }
}
