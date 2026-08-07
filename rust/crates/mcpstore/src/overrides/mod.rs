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
    pub(crate) async fn ensure_original_key_for_component(
        &self,
        kind: ComponentKind,
        instance_id: InstanceId,
        client_key: &str,
        raw_keys: &[String],
    ) -> Result<String> {
        if kind != ComponentKind::Tool {
            unimplemented!("non-tool overrides are not implemented in M1");
        }
        if raw_keys.iter().any(|key| key == client_key) {
            return Ok(client_key.to_string());
        }
        for original in raw_keys {
            if let Some(rule) = self.load_tool_override(instance_id, original).await? {
                if rule.common.enabled.unwrap_or(true)
                    && rule.common.display_name.as_deref() == Some(client_key)
                {
                    return Ok(original.clone());
                }
            }
        }
        Err(StoreError::Other(format!(
            "{kind:?} '{client_key}' not found in instance '{instance_id}'"
        )))
    }
}

pub mod tools;

pub use tools::{
    ToolArgumentOverride, ToolOverridePatch, ToolOverrideRule, ToolOverrideSafetyPolicy,
};
