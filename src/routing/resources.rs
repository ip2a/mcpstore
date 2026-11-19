use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::routing::{
    apply_route_meta, is_route_visible, ComponentKind, RouteFields,
};
use crate::store::prelude::*;

pub(crate) const RESOURCE_ROUTES_STATE_TYPE: &str = "resource_routes";
const RESOURCE_TEMPLATE_ROUTES_STATE_TYPE: &str = "resource_template_routes";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ResourceRoutePatch {
    #[serde(flatten)]
    pub common: RouteFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceRouteRule {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub uri: String,
    #[serde(flatten)]
    pub common: RouteFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub updated_at: i64,
    pub version: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ResourceTemplateRoutePatch {
    #[serde(flatten)]
    pub common: RouteFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceTemplateRouteRule {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub uri_template: String,
    #[serde(flatten)]
    pub common: RouteFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub updated_at: i64,
    pub version: u64,
}

macro_rules! common_builders {
    ($type:ident) => {
        impl $type {
            pub fn with_display_name(mut self, v: impl Into<String>) -> Self {
                self.common.display_name = Some(v.into());
                self
            }
            pub fn with_description(mut self, v: impl Into<String>) -> Self {
                self.common.description = Some(v.into());
                self
            }
            pub fn with_mime_type(mut self, v: impl Into<String>) -> Self {
                self.mime_type = Some(v.into());
                self
            }
            pub fn with_meta(mut self, v: serde_json::Value) -> Self {
                self.common.meta = Some(v);
                self
            }
            pub fn with_tag(mut self, v: impl Into<String>) -> Self {
                self.common.tags.push(v.into());
                self
            }
            pub fn enabled(mut self, v: bool) -> Self {
                self.common.enabled = Some(v);
                self
            }
        }
    };
}
common_builders!(ResourceRoutePatch);
common_builders!(ResourceTemplateRoutePatch);

impl MCPStore {
    pub async fn set_resource_route(
        &self,
        instance_id: InstanceId,
        uri: &str,
        patch: ResourceRoutePatch,
    ) -> Result<ResourceRouteRule> {
        let instance = self.require_instance(instance_id).await?;
        let raw_uris = self
            .list_resources(instance_id)
            .await?
            .into_iter()
            .map(|resource| resource.uri)
            .collect::<Vec<_>>();
        let uri = self
            .ensure_original_key_for_component(ComponentKind::Resource, instance_id, uri, &raw_uris)
            .await?;
        let loaded = self.load_resource_route(instance_id, &uri).await?;
        let expected = loaded.as_ref().map(|r| r.version);
        let mut rule = loaded.unwrap_or(ResourceRouteRule {
            instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            uri: uri.clone(),
            common: RouteFields::default(),
            mime_type: None,
            updated_at: 0,
            version: 0,
        });
        rule.service_name = instance.service_name;
        rule.scope = instance.scope;
        rule.uri = uri.clone();
        rule.common = patch.common;
        rule.common.display_name = rule.common.display_name.filter(|v| !v.trim().is_empty());
        rule.mime_type = patch.mime_type;
        rule.updated_at = Self::now_timestamp();
        rule.version += 1;
        self.kernel
            .persistence
            .kv
            .compare_and_put_state(
                RESOURCE_ROUTES_STATE_TYPE,
                &Self::component_route_key(instance_id, &uri),
                expected,
                serde_json::to_value(&rule)
                    .map_err(|e| Error::new(FailureCode::Internal, e.to_string()))?,
            )
            .await?;
        Ok(rule)
    }
    pub async fn get_resource_route(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<Option<ResourceRouteRule>> {
        self.load_resource_route(instance_id, uri).await
    }
    pub async fn remove_resource_route(&self, instance_id: InstanceId, uri: &str) -> Result<()> {
        self.kernel
            .persistence
            .kv
            .delete_state(
                RESOURCE_ROUTES_STATE_TYPE,
                &Self::component_route_key(instance_id, uri),
            )
            .await
            .map_err(Error::from)
    }
    pub async fn list_resource_routes(&self) -> Result<Vec<ResourceRouteRule>> {
        list_rules::<ResourceRouteRule, _>(
            &self.kernel.persistence.kv,
            RESOURCE_ROUTES_STATE_TYPE,
            |a, b| (a.instance_id, a.uri.as_str()).cmp(&(b.instance_id, b.uri.as_str())),
        )
        .await
    }
    pub async fn enable_resource(&self, i: InstanceId, u: &str) -> Result<()> {
        self.patch_component_visibility(ComponentKind::Resource, i, u, true)
            .await
    }
    pub async fn disable_resource(&self, i: InstanceId, u: &str) -> Result<()> {
        self.patch_component_visibility(ComponentKind::Resource, i, u, false)
            .await
    }
    pub(crate) async fn load_resource_route(
        &self,
        i: InstanceId,
        u: &str,
    ) -> Result<Option<ResourceRouteRule>> {
        load_rule(
            &self.kernel.persistence.kv,
            RESOURCE_ROUTES_STATE_TYPE,
            i,
            u,
            "Resource",
        )
        .await
    }

    /// Batched read of one instance's enabled resource route rules — keys
    /// derive from URIs the caller already listed, so no second catalog read.
    pub(crate) async fn enabled_resource_routes_for_uris(
        &self,
        instance_id: InstanceId,
        uris: &[String],
    ) -> Result<HashMap<String, ResourceRouteRule>> {
        enabled_rules_for_keys(
            &self.kernel.persistence.kv,
            RESOURCE_ROUTES_STATE_TYPE,
            instance_id,
            uris,
            "Resource",
        )
        .await
    }

    /// Batched read of one instance's enabled resource-template route rules.
    pub(crate) async fn enabled_resource_template_routes_for_keys(
        &self,
        instance_id: InstanceId,
        uri_templates: &[String],
    ) -> Result<HashMap<String, ResourceTemplateRouteRule>> {
        enabled_rules_for_keys(
            &self.kernel.persistence.kv,
            RESOURCE_TEMPLATE_ROUTES_STATE_TYPE,
            instance_id,
            uri_templates,
            "Resource template",
        )
        .await
    }

    pub async fn set_resource_template_route(
        &self,
        i: InstanceId,
        u: &str,
        patch: ResourceTemplateRoutePatch,
    ) -> Result<ResourceTemplateRouteRule> {
        let instance = self.require_instance(i).await?;
        let raw_templates = self
            .list_resource_templates(i)
            .await?
            .into_iter()
            .map(|template| template.uri_template)
            .collect::<Vec<_>>();
        let u = self
            .ensure_original_key_for_component(
                ComponentKind::ResourceTemplate,
                i,
                u,
                &raw_templates,
            )
            .await?;
        let loaded = self.load_resource_template_route(i, &u).await?;
        let expected = loaded.as_ref().map(|r| r.version);
        let mut rule = loaded.unwrap_or(ResourceTemplateRouteRule {
            instance_id: i,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            uri_template: u.clone(),
            common: RouteFields::default(),
            mime_type: None,
            updated_at: 0,
            version: 0,
        });
        rule.service_name = instance.service_name;
        rule.scope = instance.scope;
        rule.uri_template = u.clone();
        rule.common = patch.common;
        rule.common.display_name = rule.common.display_name.filter(|v| !v.trim().is_empty());
        rule.mime_type = patch.mime_type;
        rule.updated_at = Self::now_timestamp();
        rule.version += 1;
        self.kernel
            .persistence
            .kv
            .compare_and_put_state(
                RESOURCE_TEMPLATE_ROUTES_STATE_TYPE,
                &Self::component_route_key(i, &u),
                expected,
                serde_json::to_value(&rule)
                    .map_err(|e| Error::new(FailureCode::Internal, e.to_string()))?,
            )
            .await?;
        Ok(rule)
    }
    pub async fn get_resource_template_route(
        &self,
        i: InstanceId,
        u: &str,
    ) -> Result<Option<ResourceTemplateRouteRule>> {
        self.load_resource_template_route(i, u).await
    }
    pub async fn remove_resource_template_route(&self, i: InstanceId, u: &str) -> Result<()> {
        self.kernel
            .persistence
            .kv
            .delete_state(
                RESOURCE_TEMPLATE_ROUTES_STATE_TYPE,
                &Self::component_route_key(i, u),
            )
            .await
            .map_err(Error::from)
    }
    pub async fn list_resource_template_routes(
        &self,
    ) -> Result<Vec<ResourceTemplateRouteRule>> {
        list_rules::<ResourceTemplateRouteRule, _>(
            &self.kernel.persistence.kv,
            RESOURCE_TEMPLATE_ROUTES_STATE_TYPE,
            |a, b| {
                (a.instance_id, a.uri_template.as_str())
                    .cmp(&(b.instance_id, b.uri_template.as_str()))
            },
        )
        .await
    }
    pub async fn enable_resource_template(&self, i: InstanceId, u: &str) -> Result<()> {
        self.patch_component_visibility(ComponentKind::ResourceTemplate, i, u, true)
            .await
    }
    pub async fn disable_resource_template(&self, i: InstanceId, u: &str) -> Result<()> {
        self.patch_component_visibility(ComponentKind::ResourceTemplate, i, u, false)
            .await
    }
    pub(crate) async fn load_resource_template_route(
        &self,
        i: InstanceId,
        u: &str,
    ) -> Result<Option<ResourceTemplateRouteRule>> {
        load_rule(
            &self.kernel.persistence.kv,
            RESOURCE_TEMPLATE_ROUTES_STATE_TYPE,
            i,
            u,
            "Resource template",
        )
        .await
    }
}

/// Batched read of one instance's enabled route rules keyed by component
/// key. One `get_states_many` for the whole set — catalog paths must not
/// re-read one rule per component.
async fn enabled_rules_for_keys<T: for<'de> Deserialize<'de>>(
    kv: &std::sync::Arc<crate::kv::KvLayerManager>,
    state: &str,
    instance_id: InstanceId,
    keys: &[String],
    label: &str,
) -> Result<HashMap<String, T>> {
    let state_keys: Vec<String> = keys
        .iter()
        .map(|key| MCPStore::component_route_key(instance_id, key))
        .collect();
    let values = kv.get_states_many(state, &state_keys).await?;
    let mut rules = HashMap::new();
    for (key, value) in keys.iter().zip(values) {
        let Some(value) = value else {
            continue;
        };
        let rule: T = serde_json::from_value(value).map_err(|e| {
            Error::new(
                FailureCode::Internal,
                format!("{label} route deserialization failed: {e}"),
            )
        })?;
        rules.insert(key.clone(), rule);
    }
    Ok(rules)
}

async fn load_rule<T: for<'de> Deserialize<'de>>(
    kv: &std::sync::Arc<crate::kv::KvLayerManager>,
    state: &str,
    i: InstanceId,
    key: &str,
    label: &str,
) -> Result<Option<T>> {
    kv.get_state(state, &format!("{i}:{key}"))
        .await?
        .map(|v| {
            serde_json::from_value(v).map_err(|e| {
                Error::new(
                    FailureCode::Internal,
                    format!("{label} route deserialization failed: {e}"),
                )
            })
        })
        .transpose()
}
async fn list_rules<T: for<'de> Deserialize<'de>, F: FnMut(&T, &T) -> std::cmp::Ordering>(
    kv: &std::sync::Arc<crate::kv::KvLayerManager>,
    state: &str,
    mut cmp: F,
) -> Result<Vec<T>> {
    let mut out = kv
        .get_all_states_async(state)
        .await?
        .into_iter()
        .map(|(_, v)| {
            serde_json::from_value(v).map_err(|e| Error::new(FailureCode::Internal, e.to_string()))
        })
        .collect::<Result<Vec<T>>>()?;
    out.sort_by(|a, b| cmp(a, b));
    Ok(out)
}
/// Pure projection of one resource/template row through its route rule
/// (None rule = passthrough). Catalog paths read the rules once per instance
/// and apply this in memory.
pub(crate) fn apply_resource<T: Serialize>(
    rule: Option<&impl RuleCommon>,
    item: &T,
) -> Result<Option<serde_json::Value>> {
    let mut value =
        serde_json::to_value(item).map_err(|e| Error::new(FailureCode::Internal, e.to_string()))?;
    if let Some(rule) = rule {
        if !is_route_visible(rule.common()) {
            return Ok(None);
        };
        if let serde_json::Value::Object(o) = &mut value {
            apply_route_meta(o, rule.common());
            if let Some(mt) = rule.mime_type() {
                o.insert("mimeType".into(), mt.into());
            }
        }
    }
    Ok(Some(value))
}
pub(crate) trait RuleCommon {
    fn common(&self) -> &RouteFields;
    fn mime_type(&self) -> Option<String>;
}
impl RuleCommon for ResourceRouteRule {
    fn common(&self) -> &RouteFields {
        &self.common
    }
    fn mime_type(&self) -> Option<String> {
        self.mime_type.clone()
    }
}
impl RuleCommon for ResourceTemplateRouteRule {
    fn common(&self) -> &RouteFields {
        &self.common
    }
    fn mime_type(&self) -> Option<String> {
        self.mime_type.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_builder_sets_mime_type() {
        let p = ResourceRoutePatch::default()
            .with_display_name("x")
            .with_mime_type("text/plain");
        assert_eq!(p.common.display_name.as_deref(), Some("x"));
        assert_eq!(p.mime_type.as_deref(), Some("text/plain"));
    }
}
