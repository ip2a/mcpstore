use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_prompts(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredPrompt>> {
        self.ensure_instance_connected(instance_id).await?;
        self.pool
            .list_prompts(instance_id)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn get_prompt(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.ensure_instance_connected(instance_id).await?;
        self.pool
            .get_prompt(instance_id, prompt_name, arguments)
            .await
            .map_err(StoreError::Transport)
    }
}
