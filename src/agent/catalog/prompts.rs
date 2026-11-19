use crate::store::prelude::*;

impl MCPStore {
    /// Scope-wide prompt catalog with skip accounting: a failed instance is
    /// dropped (warned) but counted, so a caller that needs a complete page
    /// can refuse to publish a partial one.
    pub(crate) async fn collect_scope_prompts_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<ScopedCatalogPage<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut prompts = Vec::new();
        let mut skipped_instances = 0usize;
        for instance in instances {
            // Same partial tolerance as tools: one unreachable instance must
            // not blank the whole scope catalog. The addressable per-instance
            // route still reports the error loudly to its caller.
            match self.list_prompts_for_instance(instance.instance_id).await {
                Ok(instance_prompts) => prompts.extend(instance_prompts),
                Err(error) => {
                    skipped_instances += 1;
                    tracing::warn!(
                        %error,
                        service = %instance.service_name,
                        "scope prompts listing skipped a failed instance"
                    );
                }
            }
        }
        prompts.sort_by(|left, right| {
            left.get("service_name")
                .and_then(serde_json::Value::as_str)
                .cmp(
                    &right
                        .get("service_name")
                        .and_then(serde_json::Value::as_str),
                )
                .then_with(|| {
                    left.get("name")
                        .and_then(serde_json::Value::as_str)
                        .cmp(&right.get("name").and_then(serde_json::Value::as_str))
                })
        });
        Ok(ScopedCatalogPage {
            entries: prompts,
            skipped_instances,
        })
    }
}
