use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_scope_prompts_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut prompts = Vec::new();
        for instance in instances {
            prompts.extend(self.list_prompts_for_instance(instance.instance_id).await?);
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
        Ok(prompts)
    }
}
