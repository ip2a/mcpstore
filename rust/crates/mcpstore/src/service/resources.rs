use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_resources(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredResource>> {
        self.ensure_instance_connected(instance_id).await?;
        if self.is_openapi_virtual_instance(instance_id).await? {
            let instance = self
                .registry
                .find_instance(instance_id)
                .await
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
            let import = self
                .get_openapi_import(&instance.service_name)
                .await?
                .ok_or_else(|| {
                    StoreError::Other(format!(
                        "OpenAPI import not found for instance {instance_id}"
                    ))
                })?;
            return crate::openapi_runtime::list_openapi_resources(&import)
                .into_iter()
                .map(|resource| {
                    serde_json::from_value(resource).map_err(|error| {
                        StoreError::Other(format!("OpenAPI resource model failed: {error}"))
                    })
                })
                .collect();
        }
        self.pool
            .list_resources(instance_id)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn list_resource_templates(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<DiscoveredResourceTemplate>> {
        self.ensure_instance_connected(instance_id).await?;
        if self.is_openapi_virtual_instance(instance_id).await? {
            let instance = self
                .registry
                .find_instance(instance_id)
                .await
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
            let import = self
                .get_openapi_import(&instance.service_name)
                .await?
                .ok_or_else(|| {
                    StoreError::Other(format!(
                        "OpenAPI import not found for instance {instance_id}"
                    ))
                })?;
            return crate::openapi_runtime::list_openapi_resource_templates(&import)
                .into_iter()
                .map(|template| {
                    serde_json::from_value(template).map_err(|error| {
                        StoreError::Other(format!(
                            "OpenAPI resource template model failed: {error}"
                        ))
                    })
                })
                .collect();
        }
        self.pool
            .list_resource_templates(instance_id)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn read_resource(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<serde_json::Value> {
        self.ensure_instance_connected(instance_id).await?;
        if self.is_openapi_virtual_instance(instance_id).await? {
            let instance = self
                .registry
                .find_instance(instance_id)
                .await
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
            let import = self
                .get_openapi_import(&instance.service_name)
                .await?
                .ok_or_else(|| {
                    StoreError::Other(format!(
                        "OpenAPI import not found for instance {instance_id}"
                    ))
                })?;
            let options = self
                .openapi_runtime_options_for_instance(instance_id)
                .await?;
            return crate::openapi_runtime::read_openapi_resource(&import, uri, &options).await;
        }
        self.pool
            .read_resource(instance_id, uri)
            .await
            .map_err(StoreError::Transport)
    }
}
