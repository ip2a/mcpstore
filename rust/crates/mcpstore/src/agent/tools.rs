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
                    let original_name = tool.name.clone();
                    let transformed = self
                        .apply_tool_transform(
                            service_name,
                            &original_name,
                            tool.name,
                            tool.description,
                            tool.input_schema,
                        )
                        .await?;
                    payload.push(Self::tool_payload_value(
                        transformed.display_name,
                        original_name,
                        service_name.to_string(),
                        service_name.to_string(),
                        tool.title,
                        transformed.description,
                        transformed.input_schema,
                        tool.output_schema,
                        tool.annotations,
                        tool.meta,
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
                    let original_name = tool.name.clone();
                    let transformed = self
                        .apply_tool_transform(
                            &global_service_name,
                            &original_name,
                            tool.name,
                            tool.description,
                            tool.input_schema,
                        )
                        .await?;
                    payload.push(Self::tool_payload_value(
                        transformed.display_name,
                        original_name,
                        service.original_name.clone(),
                        global_service_name.clone(),
                        tool.title,
                        transformed.description,
                        transformed.input_schema,
                        tool.output_schema,
                        tool.annotations,
                        tool.meta,
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
                let mut entries = Vec::with_capacity(tools.len());
                for tool in tools {
                    let original_name = tool.name.clone();
                    let transformed = self
                        .apply_tool_transform(
                            service_name,
                            &original_name,
                            tool.name,
                            tool.description,
                            tool.input_schema,
                        )
                        .await?;
                    entries.push(Self::scoped_tool_entry(
                        transformed.display_name,
                        original_name,
                        service_name.to_string(),
                        service_name.to_string(),
                        tool.title,
                        transformed.description,
                        transformed.input_schema,
                        tool.output_schema,
                        tool.annotations,
                        tool.meta,
                    )?);
                }
                Ok(entries)
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
                let mut entries = Vec::with_capacity(tools.len());
                for tool in tools {
                    let original_name = tool.name.clone();
                    let transformed = self
                        .apply_tool_transform(
                            &global_service_name,
                            &original_name,
                            tool.name,
                            tool.description,
                            tool.input_schema,
                        )
                        .await?;
                    entries.push(Self::scoped_tool_entry(
                        transformed.display_name,
                        original_name,
                        service.original_name.clone(),
                        global_service_name.clone(),
                        tool.title,
                        transformed.description,
                        transformed.input_schema,
                        tool.output_schema,
                        tool.annotations,
                        tool.meta,
                    )?);
                }
                Ok(entries)
            }
            (Some(agent_id), None) => self.collect_agent_tool_descriptions_scoped(agent_id).await,
        }
    }
}
