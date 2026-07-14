use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_scope_tools_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut tools = Vec::new();
        for instance in instances {
            tools.extend(self.list_tools_for_instance(instance.instance_id).await?);
        }
        tools.sort_by(|left, right| {
            left.get("service_name")
                .and_then(serde_json::Value::as_str)
                .cmp(
                    &right
                        .get("service_name")
                        .and_then(serde_json::Value::as_str),
                )
                .then_with(|| {
                    left.get("tool_name")
                        .and_then(serde_json::Value::as_str)
                        .cmp(&right.get("tool_name").and_then(serde_json::Value::as_str))
                })
        });
        Ok(tools)
    }

    pub(crate) async fn collect_scope_tool_entries_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<ScopedToolEntry>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut tools = Vec::new();
        for instance in instances {
            tools.extend(
                self.list_tool_entries_for_instance(instance.instance_id)
                    .await?,
            );
        }
        tools.sort_by(|left, right| {
            left.service_name
                .cmp(&right.service_name)
                .then_with(|| left.tool_name.cmp(&right.tool_name))
                .then_with(|| left.instance_id.cmp(&right.instance_id))
        });
        Ok(tools)
    }
}
