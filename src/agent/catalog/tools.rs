use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_scope_tools_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        // One shared-store read for every instance's override rules, then one
        // concurrent fetch per instance (each rides its own pooled connection
        // — per-connection lock in the pool — so one slow upstream no longer
        // serializes the whole catalog behind it). Results and errors are
        // consumed in instance order: the first failure is the one that
        // surfaces, exactly as the sequential loop did.
        let overrides_by_instance = self.enabled_tool_overrides_by_instance().await?;
        let empty_overrides = std::collections::HashMap::new();
        let (overrides_by_instance, empty_overrides) = (&overrides_by_instance, &empty_overrides);
        let results = futures::future::join_all(instances.iter().map(|instance| async move {
            let overrides = overrides_by_instance
                .get(&instance.instance_id)
                .unwrap_or(&empty_overrides);
            let policy = crate::agent::tool_visibility::EffectiveToolPolicy::resolve_with_overrides(
                self,
                instance.instance_id,
                overrides,
            )
            .await?;
            policy
                .available
                .into_iter()
                .map(|entry| {
                    serde_json::to_value(entry)
                        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))
                })
                .collect::<Result<Vec<serde_json::Value>>>()
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
