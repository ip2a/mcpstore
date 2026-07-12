use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_prompts(&self, service_name: &str) -> Result<Vec<DiscoveredPrompt>> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .list_prompts(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn get_prompt(
        &self,
        service_name: &str,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .get_prompt(service_name, prompt_name, arguments)
            .await
            .map_err(StoreError::Transport)
    }
}
