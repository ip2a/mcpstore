use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_resources(&self, service_name: &str) -> Result<Vec<serde_json::Value>> {
        self.ensure_service_connected(service_name).await?;
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
        self.pool
            .list_resource_templates(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn read_resource(&self, service_name: &str, uri: &str) -> Result<serde_json::Value> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .read_resource(service_name, uri)
            .await
            .map_err(StoreError::Transport)
    }
}
