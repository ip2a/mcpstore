use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_prompts_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        match (agent_id, service_name) {
            (_, Some(service_name)) => {
                let (display_service_name, global_service_name) = self
                    .resolve_scoped_service_binding(agent_id, service_name)
                    .await?;
                let mut prompts = self.list_prompts(&global_service_name).await?;
                prompts.sort_by(|left, right| {
                    Self::value_field(left, "name").cmp(Self::value_field(right, "name"))
                });
                prompts
                    .into_iter()
                    .map(|prompt| {
                        Self::prompt_payload_value(
                            prompt,
                            None,
                            display_service_name.clone(),
                            global_service_name.clone(),
                        )
                    })
                    .collect()
            }
            (None, None) => self.collect_store_prompts_scoped().await,
            (Some(agent_id), None) => self.collect_agent_prompts_scoped(agent_id).await,
        }
    }

    pub async fn get_prompt_scoped(
        &self,
        agent_id: Option<&str>,
        prompt_name: &str,
        arguments: serde_json::Value,
        service_name: Option<&str>,
    ) -> Result<serde_json::Value> {
        let (_, global_service_name, original_prompt_name) = self
            .resolve_prompt_binding(agent_id, prompt_name, service_name)
            .await?;
        self.get_prompt(&global_service_name, &original_prompt_name, arguments)
            .await
    }
}
