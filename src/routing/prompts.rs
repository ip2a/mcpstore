use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::routing::{
    apply_route_meta, is_route_visible, ComponentKind, RouteFields,
};
use crate::store::prelude::*;

pub(crate) const PROMPT_ROUTES_STATE_TYPE: &str = "prompt_routes";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PromptRoutePatch {
    #[serde(flatten)]
    pub common: RouteFields,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptRouteRule {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub prompt_name: String,
    #[serde(flatten)]
    pub common: RouteFields,
    pub updated_at: i64,
    pub version: u64,
}

impl PromptRoutePatch {
    pub fn with_display_name(mut self, value: impl Into<String>) -> Self {
        self.common.display_name = Some(value.into());
        self
    }
    pub fn with_description(mut self, value: impl Into<String>) -> Self {
        self.common.description = Some(value.into());
        self
    }
    pub fn with_meta(mut self, value: serde_json::Value) -> Self {
        self.common.meta = Some(value);
        self
    }
    pub fn with_tag(mut self, value: impl Into<String>) -> Self {
        self.common.tags.push(value.into());
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.common.enabled = Some(value);
        self
    }
}

impl MCPStore {
    pub async fn set_prompt_route(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        patch: PromptRoutePatch,
    ) -> Result<PromptRouteRule> {
        let instance = self.require_instance(instance_id).await?;
        let raw_names = self
            .list_prompts(instance_id)
            .await?
            .into_iter()
            .map(|prompt| prompt.name)
            .collect::<Vec<_>>();
        let prompt_name = self
            .ensure_original_key_for_component(
                ComponentKind::Prompt,
                instance_id,
                prompt_name,
                &raw_names,
            )
            .await?;
        let loaded = self.load_prompt_route(instance_id, &prompt_name).await?;
        let expected_version = loaded.as_ref().map(|rule| rule.version);
        let mut rule = loaded.unwrap_or_else(|| PromptRouteRule {
            instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            prompt_name: prompt_name.clone(),
            common: RouteFields::default(),
            updated_at: 0,
            version: 0,
        });
        rule.service_name = instance.service_name;
        rule.scope = instance.scope;
        rule.prompt_name = prompt_name.clone();
        rule.common = patch.common;
        rule.common.display_name = rule.common.display_name.filter(|v| !v.trim().is_empty());
        rule.updated_at = Self::now_timestamp();
        rule.version += 1;
        self.kernel
            .persistence
            .kv
            .compare_and_put_state(
                PROMPT_ROUTES_STATE_TYPE,
                &Self::component_route_key(instance_id, &prompt_name),
                expected_version,
                serde_json::to_value(&rule)
                    .map_err(|e| Error::new(FailureCode::Internal, e.to_string()))?,
            )
            .await?;
        Ok(rule)
    }

    pub async fn get_prompt_route(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
    ) -> Result<Option<PromptRouteRule>> {
        self.load_prompt_route(instance_id, prompt_name).await
    }
    pub async fn remove_prompt_route(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
    ) -> Result<()> {
        self.kernel
            .persistence
            .kv
            .delete_state(
                PROMPT_ROUTES_STATE_TYPE,
                &Self::component_route_key(instance_id, prompt_name),
            )
            .await
            .map_err(Error::from)
    }
    pub async fn list_prompt_routes(&self) -> Result<Vec<PromptRouteRule>> {
        let mut rules = self
            .kernel
            .persistence
            .kv
            .get_all_states_async(PROMPT_ROUTES_STATE_TYPE)
            .await?
            .into_iter()
            .map(|(key, value)| {
                serde_json::from_value::<PromptRouteRule>(value).map_err(|e| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Prompt route deserialization failed for {key}: {e}"),
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;
        rules.sort_by(|a, b| {
            (a.instance_id, a.prompt_name.as_str()).cmp(&(b.instance_id, b.prompt_name.as_str()))
        });
        Ok(rules)
    }
    pub async fn enable_prompt(&self, instance_id: InstanceId, prompt_name: &str) -> Result<()> {
        self.patch_component_visibility(ComponentKind::Prompt, instance_id, prompt_name, true)
            .await
    }
    pub async fn disable_prompt(&self, instance_id: InstanceId, prompt_name: &str) -> Result<()> {
        self.patch_component_visibility(ComponentKind::Prompt, instance_id, prompt_name, false)
            .await
    }

    pub(crate) async fn load_prompt_route(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
    ) -> Result<Option<PromptRouteRule>> {
        self.kernel
            .persistence
            .kv
            .get_state(
                PROMPT_ROUTES_STATE_TYPE,
                &Self::component_route_key(instance_id, prompt_name),
            )
            .await?
            .map(|value| {
                serde_json::from_value(value).map_err(|e| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Prompt route deserialization failed: {e}"),
                    )
                })
            })
            .transpose()
    }
    /// Batched read of one instance's prompt route rules (enabled or not —
    /// a disabled rule must stay in the map so the projection drops the
    /// prompt). Keys derive from prompt names the caller already listed, so no
    /// second catalog read.
    pub(crate) async fn prompt_routes_for_names(
        &self,
        instance_id: InstanceId,
        prompt_names: &[String],
    ) -> Result<HashMap<String, PromptRouteRule>> {
        let keys: Vec<String> = prompt_names
            .iter()
            .map(|name| Self::component_route_key(instance_id, name))
            .collect();
        let values = self
            .kernel
            .persistence
            .kv
            .get_states_many(PROMPT_ROUTES_STATE_TYPE, &keys)
            .await?;
        let mut rules = HashMap::new();
        for (name, value) in prompt_names.iter().zip(values) {
            let Some(value) = value else {
                continue;
            };
            let rule: PromptRouteRule = serde_json::from_value(value).map_err(|e| {
                Error::new(
                    FailureCode::Internal,
                    format!("Prompt route deserialization failed: {e}"),
                )
            })?;
            rules.insert(name.clone(), rule);
        }
        Ok(rules)
    }

    /// Pure projection of one prompt row through its route rule (no rule =
    /// passthrough). Catalog paths read the rules once per instance and apply
    /// this in memory.
    pub(crate) fn project_prompt_value(
        prompt: &DiscoveredPrompt,
        rule: Option<&PromptRouteRule>,
    ) -> Result<Option<serde_json::Value>> {
        let Some(rule) = rule else {
            return Ok(Some(
                serde_json::to_value(prompt)
                    .map_err(|e| Error::new(FailureCode::Internal, e.to_string()))?,
            ));
        };
        if !is_route_visible(&rule.common) {
            return Ok(None);
        }
        let mut value = serde_json::to_value(prompt)
            .map_err(|e| Error::new(FailureCode::Internal, e.to_string()))?;
        if let serde_json::Value::Object(object) = &mut value {
            apply_route_meta(object, &rule.common);
        }
        Ok(Some(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patch_builder_sets_common_fields() {
        let patch = PromptRoutePatch::default()
            .with_display_name("x")
            .enabled(false);
        assert_eq!(patch.common.display_name.as_deref(), Some("x"));
        assert_eq!(patch.common.enabled, Some(false));
    }
    #[test]
    fn meta_override_inserts_name() {
        let mut map = serde_json::Map::new();
        apply_route_meta(
            &mut map,
            &RouteFields::default().with_display_name("x"),
        );
        assert_eq!(map.get("name"), Some(&serde_json::json!("x")));
    }
}
