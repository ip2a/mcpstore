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
        let names: Vec<String> = prompts.iter().map(|prompt| prompt.name.clone()).collect();
        // One batched read for the whole set — never one KV read per prompt.
        let overrides = self.prompt_overrides_for_names(instance_id, &names).await?;
        let mut out = Vec::with_capacity(prompts.len());
        for prompt in prompts {
            let Some(mut value) = Self::project_prompt_value(&prompt, overrides.get(&prompt.name))?
            else {
                continue;
            };
            crate::agent::models::insert_instance_identity(
                &mut value,
                instance_id,
                &instance.service_name,
                &instance.scope,
            );
            out.push(value);
        }
        Ok(out)
    }

    /// Scope-view prompt catalog in one page. A failed instance (disconnected,
    /// auth pending) is skipped — counted in `skipped_instances`, never blanks
    /// the page; the per-instance route stays loud.
    pub async fn list_prompts_viewed(
        &self,
        view: &ScopeView,
    ) -> Result<ScopedCatalogPage<serde_json::Value>> {
        let instances = self.list_view_instances(view).await?;
        let mut entries = Vec::new();
        let mut skipped_instances = 0usize;
        for instance in &instances {
            match self.list_prompts_for_instance(instance.instance_id).await {
                Ok(instance_prompts) => entries.extend(instance_prompts),
                Err(error) => {
                    skipped_instances += 1;
                    tracing::warn!(
                        %error,
                        service = %instance.service_name,
                        "scope prompt catalog skipped a failed instance"
                    );
                }
            }
        }
        entries.sort_by(|left, right| {
            left.get("service_name")
                .and_then(serde_json::Value::as_str)
                .cmp(
                    &right
                        .get("service_name")
                        .and_then(serde_json::Value::as_str),
                )
                .then_with(|| {
                    left.get("name")
                        .and_then(serde_json::Value::as_str)
                        .cmp(&right.get("name").and_then(serde_json::Value::as_str))
                })
        });
        Ok(ScopedCatalogPage {
            entries,
            skipped_instances,
        })
    }

    pub async fn get_prompt_scoped(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Execution surface: connect on demand BEFORE resolving the original
        // key — list_prompts is side-effect free now and answers NotConnected
        // for a disconnected instance.
        self.ensure_instance_connected(instance_id).await?;
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
        if self
            .load_prompt_override(instance_id, &original)
            .await?
            .is_some_and(|rule| rule.common.enabled == Some(false))
        {
            return Err(crate::Error::new(
                FailureCode::Internal,
                format!("Prompt '{prompt_name}' is disabled in instance '{instance_id}'"),
            ));
        }
        self.get_prompt(instance_id, &original, arguments).await
    }
}
