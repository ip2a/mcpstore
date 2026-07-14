use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_tools_scoped(&self, scope: &ScopeRef) -> Result<Vec<serde_json::Value>> {
        self.collect_scope_tools_scoped(scope).await
    }

    pub async fn list_tools_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<serde_json::Value>> {
        let instance = self.require_instance(instance_id).await?;
        let mut tools = self.list_tools(instance_id).await?;
        tools.sort_by(|left, right| left.name.cmp(&right.name));

        let mut payload = Vec::with_capacity(tools.len());
        for tool in tools {
            let tool_name = tool.name.clone();
            let transformed = self
                .apply_tool_transform(
                    instance_id,
                    &tool_name,
                    tool.name,
                    tool.description,
                    tool.input_schema,
                )
                .await?;
            payload.push(serde_json::json!({
                "name": transformed.display_name,
                "tool_name": tool_name,
                "title": tool.title,
                "description": transformed.description,
                "input_schema": transformed.input_schema,
                "output_schema": tool.output_schema,
                "annotations": tool.annotations,
                "_meta": tool.meta,
                "instance_id": instance_id,
                "service_name": instance.service_name,
                "scope": instance.scope,
            }));
        }
        Ok(payload)
    }

    pub async fn list_tool_entries_scoped(&self, scope: &ScopeRef) -> Result<Vec<ScopedToolEntry>> {
        self.collect_scope_tool_entries_scoped(scope).await
    }

    pub async fn list_tool_entries_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<ScopedToolEntry>> {
        let instance = self.require_instance(instance_id).await?;
        let mut tools = self.list_tools(instance_id).await?;
        tools.sort_by(|left, right| left.name.cmp(&right.name));

        let mut entries = Vec::with_capacity(tools.len());
        for tool in tools {
            let tool_name = tool.name.clone();
            let transformed = self
                .apply_tool_transform(
                    instance_id,
                    &tool_name,
                    tool.name,
                    tool.description,
                    tool.input_schema,
                )
                .await?;
            entries.push(ScopedToolEntry {
                name: transformed.display_name,
                tool_name,
                title: tool.title,
                description: transformed.description,
                input_schema: transformed.input_schema,
                output_schema: tool.output_schema,
                annotations: tool.annotations,
                meta: tool.meta,
                instance_id,
                service_name: instance.service_name.clone(),
                scope: instance.scope.clone(),
            });
        }
        Ok(entries)
    }
}
