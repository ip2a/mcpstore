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
        resources
            .into_iter()
            .map(|resource| {
                let mut value = serde_json::to_value(resource)
                    .map_err(|error| StoreError::Other(error.to_string()))?;
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
                Ok(value)
            })
            .collect()
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
        templates
            .into_iter()
            .map(|template| {
                let mut value = serde_json::to_value(template)
                    .map_err(|error| StoreError::Other(error.to_string()))?;
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
                Ok(value)
            })
            .collect()
    }

    pub async fn read_resource_scoped(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<serde_json::Value> {
        self.require_instance(instance_id).await?;
        self.read_resource(instance_id, uri).await
    }
}
