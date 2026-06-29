use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_resources(&self, service_name: &str) -> Result<Vec<serde_json::Value>> {
        self.ensure_service_connected(service_name).await?;
        if self.is_openapi_virtual_service(service_name).await? {
            let import = self
                .get_openapi_import(service_name)
                .await?
                .ok_or_else(|| {
                    StoreError::Other(format!("OpenAPI import not found: {service_name}"))
                })?;
            return Ok(crate::openapi_runtime::list_openapi_resources(&import));
        }
        self.pool
            .list_resources(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn list_resource_templates(
        &self,
        service_name: &str,
    ) -> Result<Vec<serde_json::Value>> {
        self.ensure_service_connected(service_name).await?;
        if self.is_openapi_virtual_service(service_name).await? {
            let import = self
                .get_openapi_import(service_name)
                .await?
                .ok_or_else(|| {
                    StoreError::Other(format!("OpenAPI import not found: {service_name}"))
                })?;
            return Ok(crate::openapi_runtime::list_openapi_resource_templates(
                &import,
            ));
        }
        self.pool
            .list_resource_templates(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn read_resource(&self, service_name: &str, uri: &str) -> Result<serde_json::Value> {
        self.ensure_service_connected(service_name).await?;
        if self.is_openapi_virtual_service(service_name).await? {
            let import = self
                .get_openapi_import(service_name)
                .await?
                .ok_or_else(|| {
                    StoreError::Other(format!("OpenAPI import not found: {service_name}"))
                })?;
            let options = self.openapi_runtime_options(service_name).await?;
            return crate::openapi_runtime::read_openapi_resource(&import, uri, &options).await;
        }
        self.pool
            .read_resource(service_name, uri)
            .await
            .map_err(StoreError::Transport)
    }
}
