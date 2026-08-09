use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_resources_scoped(&self, scope: &ScopeRef) -> Result<Vec<serde_json::Value>> {
        self.collect_scope_resources_scoped(scope).await
    }

    pub async fn list_resources_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<serde_json::Value>> {
        let instance = self.require_instance(instance_id).await?;
        let mut resources = self.list_resources(instance_id).await?;
        resources.sort_by(|left, right| left.uri.cmp(&right.uri));
        let mut out = Vec::new();
        for resource in resources {
            let Some(mut value) = self.apply_resource_override(instance_id, &resource).await?
            else {
                continue;
            };
            if let serde_json::Value::Object(object) = &mut value {
                object.insert("instance_id".to_string(), serde_json::json!(instance_id));
                object.insert(
                    "service_name".to_string(),
                    serde_json::json!(instance.service_name.clone()),
                );
                object.insert(
                    "scope".to_string(),
                    serde_json::json!(instance.scope.clone()),
                );
            }
            out.push(value);
        }
        Ok(out)
    }

    pub async fn list_resource_templates_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        self.collect_scope_resource_templates_scoped(scope).await
    }

    pub async fn list_resource_templates_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<serde_json::Value>> {
        let instance = self.require_instance(instance_id).await?;
        let mut templates = self.list_resource_templates(instance_id).await?;
        templates.sort_by(|left, right| left.uri_template.cmp(&right.uri_template));
        let mut out = Vec::new();
        for template in templates {
            let Some(mut value) = self
                .apply_resource_template_override(instance_id, &template)
                .await?
            else {
                continue;
            };
            if let serde_json::Value::Object(object) = &mut value {
                object.insert("instance_id".to_string(), serde_json::json!(instance_id));
                object.insert(
                    "service_name".to_string(),
                    serde_json::json!(instance.service_name.clone()),
                );
                object.insert(
                    "scope".to_string(),
                    serde_json::json!(instance.scope.clone()),
                );
            }
            out.push(value);
        }
        Ok(out)
    }

    pub async fn read_resource_scoped(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<serde_json::Value> {
        self.require_instance(instance_id).await?;
        let raw_uris: Vec<String> = self
            .list_resources(instance_id)
            .await?
            .into_iter()
            .map(|resource| resource.uri)
            .collect();
        let original = self
            .ensure_original_key_for_component(
                crate::overrides::ComponentKind::Resource,
                instance_id,
                uri,
                &raw_uris,
            )
            .await?;
        if self
            .load_resource_override(instance_id, &original)
            .await?
            .is_some_and(|rule| rule.common.enabled == Some(false))
        {
            return Err(crate::StoreError::Other(format!(
                "Resource '{uri}' is disabled in instance '{instance_id}'"
            )));
        }
        self.read_resource(instance_id, &original).await
    }
}
