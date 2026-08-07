use crate::overrides::ComponentKind;
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
        let mut out = Vec::new();
        for prompt in prompts {
            let Some(mut value) = self.apply_prompt_override(instance_id, &prompt).await? else {
                continue;
            };
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
            out.push(value);
        }
        Ok(out)
    }

    pub async fn get_prompt_scoped(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let raw_names: Vec<String> = self
            .list_prompts(instance_id)
            .await?
            .into_iter()
            .map(|p| p.name)
            .collect();
        let original = self
            .ensure_original_key_for_component(
                ComponentKind::Prompt,
                instance_id,
                prompt_name,
                &raw_names,
            )
            .await?;
        self.get_prompt(instance_id, &original, arguments).await
    }
}
