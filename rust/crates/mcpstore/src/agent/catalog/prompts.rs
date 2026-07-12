use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_store_prompts_scoped(&self) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(None).await?;
        targets.sort();

        let mut prompts = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_prompts = self.list_prompts(&global_service_name).await?;
            service_prompts.sort_by(|left, right| left.name.cmp(&right.name));
            for prompt in service_prompts {
                let original_name = prompt.name.clone();
                let display_name = format!("{}_{}", display_service_name, original_name);
                prompts.push(Self::prompt_payload_value(
                    prompt,
                    Some(display_name),
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(prompts)
    }

    pub(crate) async fn collect_agent_prompts_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(Some(agent_id)).await?;
        targets.sort();

        let mut prompts = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_prompts = self.list_prompts(&global_service_name).await?;
            service_prompts.sort_by(|left, right| left.name.cmp(&right.name));
            for prompt in service_prompts {
                let original_name = prompt.name.clone();
                let display_name = format!("{}_{}", display_service_name, original_name);
                prompts.push(Self::prompt_payload_value(
                    prompt,
                    Some(display_name),
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(prompts)
    }
}
