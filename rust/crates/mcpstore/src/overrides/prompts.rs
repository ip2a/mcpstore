use serde::{Deserialize, Serialize};

use crate::overrides::{
    apply_meta_override, is_override_enabled, ComponentKind, ComponentOverrideCommon,
};
use crate::store::prelude::*;

const PROMPT_OVERRIDES_STATE_TYPE: &str = "prompt_overrides";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PromptOverridePatch {
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptOverrideRule {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub prompt_name: String,
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
    pub updated_at: i64,
    pub version: u64,
}

impl PromptOverridePatch {
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
    pub async fn set_prompt_override(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        patch: PromptOverridePatch,
    ) -> Result<PromptOverrideRule> {
        self.refresh_from_db_if_needed().await?;
        let instance = self.require_instance(instance_id).await?;
        let loaded = self.load_prompt_override(instance_id, prompt_name).await?;
        let expected_version = loaded.as_ref().map(|rule| rule.version);
        let mut rule = loaded.unwrap_or_else(|| PromptOverrideRule {
            instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            prompt_name: prompt_name.into(),
            common: ComponentOverrideCommon::default(),
            updated_at: 0,
            version: 0,
        });
        rule.service_name = instance.service_name;
        rule.scope = instance.scope;
        rule.prompt_name = prompt_name.into();
        rule.common = patch.common;
        rule.common.display_name = rule.common.display_name.filter(|v| !v.trim().is_empty());
        rule.updated_at = Self::now_timestamp();
        rule.version += 1;
        self.cache
            .compare_and_put_state(
                PROMPT_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(instance_id, prompt_name),
                expected_version,
                serde_json::to_value(&rule).map_err(|e| StoreError::Other(e.to_string()))?,
            )
            .await?;
        Ok(rule)
    }

    pub async fn get_prompt_override(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
    ) -> Result<Option<PromptOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        self.load_prompt_override(instance_id, prompt_name).await
    }
    pub async fn delete_prompt_override(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
    ) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        self.cache
            .delete_state(
                PROMPT_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(instance_id, prompt_name),
            )
            .await
            .map_err(StoreError::from)
    }
    pub async fn list_prompt_overrides(&self) -> Result<Vec<PromptOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        let mut rules = self
            .cache
            .get_all_states_async(PROMPT_OVERRIDES_STATE_TYPE)
            .await?
            .into_iter()
            .map(|(key, value)| {
                serde_json::from_value::<PromptOverrideRule>(value).map_err(|e| {
                    StoreError::Other(format!(
                        "Prompt override deserialization failed for {key}: {e}"
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        rules.sort_by(|a, b| {
            (a.instance_id, a.prompt_name.as_str()).cmp(&(b.instance_id, b.prompt_name.as_str()))
        });
        Ok(rules)
    }
    pub async fn enable_prompt(&self, instance_id: InstanceId, prompt_name: &str) -> Result<()> {
        self.patch_component_enabled(ComponentKind::Prompt, instance_id, prompt_name, true)
            .await
    }
    pub async fn disable_prompt(&self, instance_id: InstanceId, prompt_name: &str) -> Result<()> {
        self.patch_component_enabled(ComponentKind::Prompt, instance_id, prompt_name, false)
            .await
    }

    pub(crate) async fn load_prompt_override(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
    ) -> Result<Option<PromptOverrideRule>> {
        self.cache
            .get_state(
                PROMPT_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(instance_id, prompt_name),
            )
            .await?
            .map(|value| {
                serde_json::from_value(value).map_err(|e| {
                    StoreError::Other(format!("Prompt override deserialization failed: {e}"))
                })
            })
            .transpose()
    }
    pub(crate) async fn apply_prompt_override(
        &self,
        instance_id: InstanceId,
        prompt: &DiscoveredPrompt,
    ) -> Result<Option<serde_json::Value>> {
        let Some(rule) = self.load_prompt_override(instance_id, &prompt.name).await? else {
            return Ok(Some(
                serde_json::to_value(prompt).map_err(|e| StoreError::Other(e.to_string()))?,
            ));
        };
        if !is_override_enabled(&rule.common) {
            return Ok(None);
        }
        let mut value =
            serde_json::to_value(prompt).map_err(|e| StoreError::Other(e.to_string()))?;
        if let serde_json::Value::Object(object) = &mut value {
            apply_meta_override(object, &rule.common);
        }
        Ok(Some(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patch_builder_sets_common_fields() {
        let patch = PromptOverridePatch::default()
            .with_display_name("x")
            .enabled(false);
        assert_eq!(patch.common.display_name.as_deref(), Some("x"));
        assert_eq!(patch.common.enabled, Some(false));
    }
    #[test]
    fn meta_override_inserts_name() {
        let mut map = serde_json::Map::new();
        apply_meta_override(
            &mut map,
            &ComponentOverrideCommon::default().with_display_name("x"),
        );
        assert_eq!(map.get("name"), Some(&serde_json::json!("x")));
    }
}
