use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_prompts(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredPrompt>> {
        self.require_instance_connected_for_read(instance_id)
            .await?;
        self.kernel.execution.pool.list_prompts(instance_id).await
    }

    pub async fn get_prompt(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.get_prompt_traced(instance_id, prompt_name, arguments, None)
            .await
    }

    pub(crate) async fn get_prompt_traced(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        arguments: serde_json::Value,
        trace: Option<std::sync::Arc<crate::telemetry::CallTrace>>,
    ) -> Result<serde_json::Value> {
        self.ensure_instance_connected(instance_id).await?;
        self.kernel
            .execution
            .pool
            .get_prompt_traced(instance_id, prompt_name, arguments, trace)
            .await
    }
}
