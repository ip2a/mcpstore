use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_scope_resources_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut resources = Vec::new();
        for instance in instances {
            resources.extend(
                self.list_resources_for_instance(instance.instance_id)
                    .await?,
            );
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
            templates.extend(
                self.list_resource_templates_for_instance(instance.instance_id)
                    .await?,
            );
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
