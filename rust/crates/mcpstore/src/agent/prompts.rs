use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_prompts_scoped(&self, scope: &ScopeRef) -> Result<Vec<serde_json::Value>> {
        self.collect_scope_prompts_scoped(scope).await
    }

    pub async fn list_prompts_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<serde_json::Value>> {
        let instance = self.require_instance(instance_id).await?;
        let mut prompts = self.list_prompts(instance_id).await?;
        prompts.sort_by(|left, right| left.name.cmp(&right.name));
        prompts
            .into_iter()
            .map(|prompt| {
                let mut value = serde_json::to_value(prompt)
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

    pub async fn get_prompt_scoped(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.require_instance(instance_id).await?;
        self.get_prompt(instance_id, prompt_name, arguments).await
    }
}
