use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_scope_tools_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        // One concurrent fetch per instance — each rides its own pooled
        // connection (per-connection lock in the pool), so one slow upstream
        // no longer serializes the whole catalog behind it. Results and
        // errors are consumed in instance order: the first failure is the
        // one that surfaces, exactly as the sequential loop did.
        let results = futures::future::join_all(instances.iter().map(|instance| async move {
            self.list_tools_for_instance_with_filter(
                instance.instance_id,
                crate::agent::tool_visibility::ToolVisibilityFilter::Available,
            )
            .await
        }))
        .await;
        let mut tools = Vec::new();
        for result in results {
            tools.extend(result?);
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
