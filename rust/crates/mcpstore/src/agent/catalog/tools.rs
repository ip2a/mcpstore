use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_store_tools_scoped(&self) -> Result<Vec<serde_json::Value>> {
        let mut services = self.list_services().await;
        services.sort_by(|left, right| left.name.cmp(&right.name));

        let mut tools = Vec::new();
        for service in services {
            let mut service_tools = service.tools.clone();
            service_tools.sort_by(|left, right| left.name.cmp(&right.name));
            for tool in service_tools {
                let original_name = tool.name.clone();
                let displayed_name = generate_tool_global_name(&service.name, &original_name)?;
                tools.push(Self::tool_payload_value(
                    displayed_name,
                    original_name,
                    service.name.clone(),
                    service.name.clone(),
                    tool.description,
                    tool.schema,
                )?);
            }
        }
        Ok(tools)
    }

    pub(crate) async fn collect_store_tool_descriptions_scoped(
        &self,
    ) -> Result<Vec<ScopedToolEntry>> {
        let mut services = self.list_services().await;
        services.sort_by(|left, right| left.name.cmp(&right.name));

        let mut tools = Vec::new();
        for service in services {
            let mut service_tools = service.tools.clone();
            service_tools.sort_by(|left, right| left.name.cmp(&right.name));
            for tool in service_tools {
                let original_name = tool.name.clone();
                let displayed_name = generate_tool_global_name(&service.name, &original_name)?;
                tools.push(Self::scoped_tool_entry(
                    displayed_name,
                    original_name,
                    service.name.clone(),
                    service.name.clone(),
                    tool.description,
                    tool.schema,
                )?);
            }
        }
        Ok(tools)
    }

    pub(crate) async fn collect_agent_tools_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut service_names = self.list_agent_service_names(agent_id).await?;
        service_names.sort();

        let mut tools = Vec::new();
        for global_service_name in service_names {
            let service = self
                .find_service(&global_service_name)
                .await
                .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
            let local_service_name = service.original_name.clone();
            let mut service_tools = self.list_tools(&global_service_name).await?;
            service_tools.sort_by(|left, right| left.name.cmp(&right.name));
            for tool in service_tools {
                let original_name = tool.name.clone();
                let displayed_name = format!("{}_{}", local_service_name, original_name);
                tools.push(Self::tool_payload_value(
                    displayed_name,
                    original_name,
                    local_service_name.clone(),
                    global_service_name.clone(),
                    tool.description,
                    tool.input_schema,
                )?);
            }
        }
        Ok(tools)
    }

    pub(crate) async fn collect_agent_tool_descriptions_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<ScopedToolEntry>> {
        let mut service_names = self.list_agent_service_names(agent_id).await?;
        service_names.sort();

        let mut tools = Vec::new();
        for global_service_name in service_names {
            let service = self
                .find_service(&global_service_name)
                .await
                .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
            let mut service_tools = self.list_tools(&global_service_name).await?;
            service_tools.sort_by(|left, right| left.name.cmp(&right.name));
            for tool in service_tools {
                let original_name = tool.name.clone();
                let displayed_name = format!("{}_{}", service.original_name, original_name);
                tools.push(Self::scoped_tool_entry(
                    displayed_name,
                    original_name,
                    service.original_name.clone(),
                    global_service_name.clone(),
                    tool.description,
                    tool.input_schema,
                )?);
            }
        }
        Ok(tools)
    }
}
