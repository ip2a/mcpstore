use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_scope_resources_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut resources = Vec::new();
        for instance in instances {
            // Partial tolerance: one unreachable instance must not blank the
            // whole scope catalog (matches tools; per-instance routes stay loud).
            match self.list_resources_for_instance(instance.instance_id).await {
                Ok(instance_resources) => resources.extend(instance_resources),
                Err(error) => tracing::warn!(
                    %error,
                    service = %instance.service_name,
                    "scope resources listing skipped a failed instance"
                ),
            }
        }
        resources.sort_by(|left, right| {
            left.get("service_name")
                .and_then(serde_json::Value::as_str)
                .cmp(
                    &right
                        .get("service_name")
                        .and_then(serde_json::Value::as_str),
                )
                .then_with(|| {
                    left.get("uri")
                        .and_then(serde_json::Value::as_str)
                        .cmp(&right.get("uri").and_then(serde_json::Value::as_str))
                })
        });
        Ok(resources)
    }

    pub(crate) async fn collect_scope_resource_templates_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut templates = Vec::new();
        for instance in instances {
            match self
                .list_resource_templates_for_instance(instance.instance_id)
                .await
            {
                Ok(instance_templates) => templates.extend(instance_templates),
                Err(error) => tracing::warn!(
                    %error,
                    service = %instance.service_name,
                    "scope resource templates listing skipped a failed instance"
                ),
            }
        }
        templates.sort_by(|left, right| {
            left.get("service_name")
                .and_then(serde_json::Value::as_str)
                .cmp(
                    &right
                        .get("service_name")
                        .and_then(serde_json::Value::as_str),
                )
                .then_with(|| {
                    left.get("uri_template")
                        .and_then(serde_json::Value::as_str)
                        .cmp(
                            &right
                                .get("uri_template")
                                .and_then(serde_json::Value::as_str),
                        )
                })
        });
        Ok(templates)
    }
}
