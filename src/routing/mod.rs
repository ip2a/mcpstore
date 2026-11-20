use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::store::prelude::*;

fn is_default_weight(weight: &i32) -> bool {
    *weight == 0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RouteFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Value>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Gateway visibility: `Some(false)` hides this component from the external
    /// catalog (store still keeps the real upstream). Restore projection by
    /// deleting the route entry — not by flipping this flag as "passthrough".
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Gateway same-name routing weight (default 0). Call path keeps only the
    /// highest weight; ties among that score are chosen at random.
    #[serde(default, skip_serializing_if = "is_default_weight")]
    pub weight: i32,
}

impl RouteFields {
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
    pub fn with_icons(mut self, value: Value) -> Self {
        self.icons = Some(value);
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
    pub fn with_weight(mut self, value: i32) -> Self {
        self.weight = value;
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
            Self::Tool => "tool_routes",
            Self::Prompt => "prompt_routes",
            Self::Resource => "resource_routes",
            Self::ResourceTemplate => "resource_template_routes",
        }
    }
}

pub(crate) fn apply_route_meta(
    value: &mut serde_json::Map<String, Value>,
    common: &RouteFields,
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
    if let Some(icons) = &common.icons {
        value.insert("icons".into(), icons.clone());
    }
}

pub(crate) fn is_route_visible(common: &RouteFields) -> bool {
    common.enabled.unwrap_or(true)
}

impl MCPStore {
    /// Fire `McpToolsChanged` after a route write so projection consumers
    /// (the gateway mount catalog, subscription notifications) rebuild
    /// promptly. The payload carries the routed kind (`tool` or `service`)
    /// and the instance so per-instance mounts can filter; `wait=false`
    /// keeps the write path non-blocking.
    pub(crate) async fn publish_route_changed(
        &self,
        kind: &'static str,
        instance_id: InstanceId,
        service_name: &str,
    ) {
        self.kernel
            .execution
            .event_bus
            .publish(
                crate::events::Event::new(
                    crate::events::types::EventKind::McpToolsChanged.as_str(),
                    serde_json::json!({
                        "reason": "route_changed",
                        "kind": kind,
                        "instanceId": instance_id.to_string(),
                        "service": service_name,
                    }),
                ),
                false,
            )
            .await;
    }

    /// Incrementally update an route rule's enabled field using version CAS.
    /// The existing rule is preserved; a conflicting write is retried once.
    pub async fn patch_component_visibility(
        &self,
        kind: ComponentKind,
        instance_id: InstanceId,
        original_key: &str,
        enabled: bool,
    ) -> Result<()> {
        self.validate_component_key(kind, instance_id, original_key)
            .await?;
        let key = Self::component_route_key(instance_id, original_key);
        for attempt in 0..8 {
            let Some(value) = self
                .kernel
                .persistence
                .kv
                .get_state(kind.state_type(), &key)
                .await?
            else {
                match self
                    .dispatch_set_enabled_when_no_rule(kind, instance_id, original_key, enabled)
                    .await
                {
                    Ok(()) => return Ok(()),
                    Err(error)
                        if error.code() == FailureCode::TaskStateFailed
                            && error.message().starts_with("kv write conflict") =>
                    {
                        continue
                    }
                    Err(error) => return Err(error),
                }
            };
            let mut obj = match value {
                Value::Object(map) => map,
                _ => {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("{kind:?} route state is not an object"),
                    ))
                }
            };
            let version = obj.get("version").and_then(Value::as_u64).ok_or_else(|| {
                Error::new(
                    FailureCode::Internal,
                    format!("{kind:?} route missing version"),
                )
            })?;
            obj.insert("enabled".into(), Value::Bool(enabled));
            obj.insert("version".into(), serde_json::json!(version + 1));
            obj.insert(
                "updated_at".into(),
                serde_json::json!(Self::now_timestamp()),
            );
            match self
                .kernel
                .persistence
                .kv
                .compare_and_put_state(kind.state_type(), &key, Some(version), Value::Object(obj))
                .await
            {
                Ok(()) => {
                    // Tool toggles change the gateway catalog projection.
                    if kind == ComponentKind::Tool {
                        let service_name = self
                            .kernel
                            .control
                            .registry
                            .find_instance(instance_id)
                            .await
                            .map(|instance| instance.service_name);
                        if let Some(service_name) = service_name.as_deref() {
                            self.publish_route_changed("tool", instance_id, service_name)
                                .await;
                        }
                    }
                    return Ok(());
                }
                Err(crate::kv::KvError::Conflict(_)) if attempt < 7 => continue,
                Err(crate::kv::KvError::Conflict(_)) => {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("{kind:?} route concurrent modification, retry exhausted"),
                    ))
                }
                Err(error) => return Err(Error::from(error)),
            }
        }
        unreachable!()
    }

    pub(crate) async fn validate_component_key(
        &self,
        kind: ComponentKind,
        instance_id: InstanceId,
        key: &str,
    ) -> Result<()> {
        let exists = match kind {
            ComponentKind::Tool => self
                .tools_from_kv(instance_id)
                .await?
                .iter()
                .any(|tool| tool.name == key),
            ComponentKind::Prompt => self
                .list_prompts(instance_id)
                .await?
                .iter()
                .any(|v| v.name == key),
            ComponentKind::Resource => self
                .list_resources(instance_id)
                .await?
                .iter()
                .any(|v| v.uri == key),
            ComponentKind::ResourceTemplate => self
                .list_resource_templates(instance_id)
                .await?
                .iter()
                .any(|v| v.uri_template == key),
        };
        if exists {
            Ok(())
        } else {
            Err(Error::new(
                FailureCode::Internal,
                format!("{kind:?} '{key}' not found in instance '{instance_id}'"),
            ))
        }
    }

    pub(crate) async fn resolve_component_route_key(
        &self,
        kind: ComponentKind,
        instance_id: InstanceId,
        client_key: &str,
    ) -> Result<String> {
        let raw_keys: Vec<String> = match kind {
            ComponentKind::Tool => self
                .tools_from_kv(instance_id)
                .await?
                .into_iter()
                .map(|tool| tool.name)
                .collect(),
            ComponentKind::Prompt => self
                .list_prompts(instance_id)
                .await?
                .into_iter()
                .map(|v| v.name)
                .collect(),
            ComponentKind::Resource => self
                .list_resources(instance_id)
                .await?
                .into_iter()
                .map(|v| v.uri)
                .collect(),
            ComponentKind::ResourceTemplate => self
                .list_resource_templates(instance_id)
                .await?
                .into_iter()
                .map(|v| v.uri_template)
                .collect(),
        };
        self.ensure_original_key_for_component(kind, instance_id, client_key, &raw_keys)
            .await
    }

    pub(crate) fn component_route_key(instance_id: InstanceId, original_key: &str) -> String {
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
                self.set_tool_route(
                    instance_id,
                    key,
                    ToolRoutePatch::default().enabled(enabled),
                )
                .await?;
            }
            ComponentKind::Prompt => {
                self.set_prompt_route(
                    instance_id,
                    key,
                    PromptRoutePatch::default().enabled(enabled),
                )
                .await?;
            }
            ComponentKind::Resource => {
                self.set_resource_route(
                    instance_id,
                    key,
                    ResourceRoutePatch::default().enabled(enabled),
                )
                .await?;
            }
            ComponentKind::ResourceTemplate => {
                self.set_resource_template_route(
                    instance_id,
                    key,
                    ResourceTemplateRoutePatch::default().enabled(enabled),
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
                    .load_tool_route(instance_id, original)
                    .await?
                    .map(|r| (r.common.display_name, r.common.enabled)),
                ComponentKind::Prompt => self
                    .load_prompt_route(instance_id, original)
                    .await?
                    .map(|r| (r.common.display_name, r.common.enabled)),
                ComponentKind::Resource => self
                    .load_resource_route(instance_id, original)
                    .await?
                    .map(|r| (r.common.display_name, r.common.enabled)),
                ComponentKind::ResourceTemplate => self
                    .load_resource_template_route(instance_id, original)
                    .await?
                    .map(|r| (r.common.display_name, r.common.enabled)),
            };
            if let Some((display_name, enabled)) = display_name_and_enabled {
                if enabled.unwrap_or(true) && display_name.as_deref() == Some(client_key) {
                    return Ok(original.clone());
                }
            }
        }
        Err(Error::new(
            FailureCode::Internal,
            format!("{kind:?} '{client_key}' not found in instance '{instance_id}'"),
        ))
    }
}

pub mod prompts;
pub mod resources;
pub mod services;
pub mod tools;

pub use prompts::{PromptRoutePatch, PromptRouteRule};
pub use resources::{
    ResourceRoutePatch, ResourceRouteRule, ResourceTemplateRoutePatch,
    ResourceTemplateRouteRule,
};
pub use services::{ServiceRoutePatch, ServiceRouteRule};
pub use tools::{
    ToolArgumentRoute, ToolRoutePatch, ToolRouteRule, ToolRouteSafetyPolicy,
};
