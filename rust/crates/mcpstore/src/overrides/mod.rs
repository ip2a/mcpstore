use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::store::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ComponentOverrideCommon {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

impl ComponentOverrideCommon {
    pub fn with_display_name(mut self, value: impl Into<String>) -> Self {
        self.display_name = Some(value.into());
        self
    }
    pub fn with_description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }
    pub fn with_meta(mut self, value: Value) -> Self {
        self.meta = Some(value);
        self
    }
    pub fn with_annotations(mut self, value: Value) -> Self {
        self.annotations = Some(value);
        self
    }
    pub fn with_tag(mut self, value: impl Into<String>) -> Self {
        self.tags.push(value.into());
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = Some(value);
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ComponentKind {
    Tool,
    Prompt,
    Resource,
    ResourceTemplate,
}

impl ComponentKind {
    pub fn state_type(self) -> &'static str {
        match self {
            Self::Tool => "tool_overrides",
            Self::Prompt => "prompt_overrides",
            Self::Resource => "resource_overrides",
            Self::ResourceTemplate => "resource_template_overrides",
        }
    }
}

pub(crate) fn apply_meta_override(
    value: &mut serde_json::Map<String, Value>,
    common: &ComponentOverrideCommon,
) {
    if let Some(name) = &common.display_name {
        value.insert("name".into(), name.clone().into());
    }
    if let Some(description) = &common.description {
        value.insert("description".into(), description.clone().into());
    }
    if let Some(meta) = &common.meta {
        value.insert("_meta".into(), meta.clone());
    }
    if let Some(annotations) = &common.annotations {
        value.insert("annotations".into(), annotations.clone());
    }
}

pub(crate) fn is_override_enabled(common: &ComponentOverrideCommon) -> bool {
    common.enabled.unwrap_or(true)
}

impl MCPStore {
    /// Incrementally update an override rule's enabled field using version CAS.
    /// The existing rule is preserved; a conflicting write is retried once.
    pub async fn patch_component_enabled(
        &self,
        kind: ComponentKind,
        instance_id: InstanceId,
        original_key: &str,
        enabled: bool,
    ) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        let key = Self::component_override_key(instance_id, original_key);
        for attempt in 0..2 {
            let Some(value) = self.cache.get_state(kind.state_type(), &key).await? else {
                self.dispatch_set_enabled_when_no_rule(kind, instance_id, original_key, enabled)
                    .await?;
                return Ok(());
            };
            let mut obj = match value {
                Value::Object(map) => map,
                _ => {
                    return Err(StoreError::Other(format!(
                        "{kind:?} override state is not an object"
                    )))
                }
            };
            let version = obj
                .get("version")
                .and_then(Value::as_u64)
                .ok_or_else(|| StoreError::Other(format!("{kind:?} override missing version")))?;
            obj.insert("enabled".into(), Value::Bool(enabled));
            obj.insert("version".into(), serde_json::json!(version + 1));
            obj.insert(
                "updated_at".into(),
                serde_json::json!(Self::now_timestamp()),
            );
            match self
                .cache
                .compare_and_put_state(kind.state_type(), &key, Some(version), Value::Object(obj))
                .await
            {
                Ok(()) => return Ok(()),
                Err(crate::cache::CacheError::Conflict(_)) if attempt == 0 => continue,
                Err(crate::cache::CacheError::Conflict(_)) => {
                    return Err(StoreError::Other(format!(
                        "{kind:?} override concurrent modification, retry exhausted"
                    )))
                }
                Err(error) => return Err(StoreError::Cache(error)),
            }
        }
        unreachable!()
    }

    pub(crate) fn component_override_key(instance_id: InstanceId, original_key: &str) -> String {
        format!("{instance_id}:{original_key}")
    }

    async fn dispatch_set_enabled_when_no_rule(
        &self,
        kind: ComponentKind,
        instance_id: InstanceId,
        key: &str,
        enabled: bool,
    ) -> Result<()> {
        match kind {
            ComponentKind::Tool => {
                self.set_tool_override(
                    instance_id,
                    key,
                    ToolOverridePatch::default().enabled(enabled),
                )
                .await?;
            }
            ComponentKind::Prompt => {
                self.set_prompt_override(
                    instance_id,
                    key,
                    PromptOverridePatch::default().enabled(enabled),
                )
                .await?;
            }
            ComponentKind::Resource => {
                self.set_resource_override(
                    instance_id,
                    key,
                    ResourceOverridePatch::default().enabled(enabled),
                )
                .await?;
            }
            ComponentKind::ResourceTemplate => {
                self.set_resource_template_override(
                    instance_id,
                    key,
                    ResourceTemplateOverridePatch::default().enabled(enabled),
                )
                .await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn ensure_original_key_for_component(
        &self,
        kind: ComponentKind,
        instance_id: InstanceId,
        client_key: &str,
        raw_keys: &[String],
    ) -> Result<String> {
        if raw_keys.iter().any(|key| key == client_key) {
            return Ok(client_key.to_string());
        }
        for original in raw_keys {
            let display_name_and_enabled = match kind {
                ComponentKind::Tool => self
                    .load_tool_override(instance_id, original)
                    .await?
                    .map(|r| (r.common.display_name, r.common.enabled)),
                ComponentKind::Prompt => self
                    .load_prompt_override(instance_id, original)
                    .await?
                    .map(|r| (r.common.display_name, r.common.enabled)),
                ComponentKind::Resource => self
                    .load_resource_override(instance_id, original)
                    .await?
                    .map(|r| (r.common.display_name, r.common.enabled)),
                ComponentKind::ResourceTemplate => self
                    .load_resource_template_override(instance_id, original)
                    .await?
                    .map(|r| (r.common.display_name, r.common.enabled)),
            };
            if let Some((display_name, enabled)) = display_name_and_enabled {
                if enabled.unwrap_or(true) && display_name.as_deref() == Some(client_key) {
                    return Ok(original.clone());
                }
            }
        }
        Err(StoreError::Other(format!(
            "{kind:?} '{client_key}' not found in instance '{instance_id}'"
        )))
    }
}

pub mod prompts;
pub mod resources;
pub mod tools;

pub use prompts::{PromptOverridePatch, PromptOverrideRule};
pub use resources::{
    ResourceOverridePatch, ResourceOverrideRule, ResourceTemplateOverridePatch,
    ResourceTemplateOverrideRule,
};
pub use tools::{
    ToolArgumentOverride, ToolOverridePatch, ToolOverrideRule, ToolOverrideSafetyPolicy,
};
