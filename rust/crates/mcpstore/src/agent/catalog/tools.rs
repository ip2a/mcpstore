use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_scope_tools_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut tools = Vec::new();
        for instance in instances {
            tools.extend(
                self.list_tools_for_instance_with_filter(
                    instance.instance_id,
                    crate::agent::tool_visibility::ToolVisibilityFilter::Available,
                )
                .await?,
            );
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
}
