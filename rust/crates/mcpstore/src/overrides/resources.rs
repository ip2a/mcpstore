use crate::overrides::{
    apply_meta_override, is_override_enabled, ComponentKind, ComponentOverrideCommon,
};
use crate::store::prelude::*;
use serde::{Deserialize, Serialize};

const RESOURCE_OVERRIDES_STATE_TYPE: &str = "resource_overrides";
const RESOURCE_TEMPLATE_OVERRIDES_STATE_TYPE: &str = "resource_template_overrides";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ResourceOverridePatch {
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceOverrideRule {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub uri: String,
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub updated_at: i64,
    pub version: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ResourceTemplateOverridePatch {
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceTemplateOverrideRule {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub uri_template: String,
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
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
common_builders!(ResourceOverridePatch);
common_builders!(ResourceTemplateOverridePatch);

impl MCPStore {
    pub async fn set_resource_override(
        &self,
        instance_id: InstanceId,
        uri: &str,
        patch: ResourceOverridePatch,
    ) -> Result<ResourceOverrideRule> {
        self.refresh_from_db_if_needed().await?;
        let instance = self.require_instance(instance_id).await?;
        let loaded = self.load_resource_override(instance_id, uri).await?;
        let expected = loaded.as_ref().map(|r| r.version);
        let mut rule = loaded.unwrap_or(ResourceOverrideRule {
            instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            uri: uri.into(),
            common: ComponentOverrideCommon::default(),
            mime_type: None,
            updated_at: 0,
            version: 0,
        });
        rule.service_name = instance.service_name;
        rule.scope = instance.scope;
        rule.uri = uri.into();
        rule.common = patch.common;
        rule.common.display_name = rule.common.display_name.filter(|v| !v.trim().is_empty());
        rule.mime_type = patch.mime_type;
        rule.updated_at = Self::now_timestamp();
        rule.version += 1;
        self.cache
            .compare_and_put_state(
                RESOURCE_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(instance_id, uri),
                expected,
                serde_json::to_value(&rule).map_err(|e| StoreError::Other(e.to_string()))?,
            )
            .await?;
        Ok(rule)
    }
    pub async fn get_resource_override(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<Option<ResourceOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        self.load_resource_override(instance_id, uri).await
    }
    pub async fn delete_resource_override(&self, instance_id: InstanceId, uri: &str) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        self.cache
            .delete_state(
                RESOURCE_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(instance_id, uri),
            )
            .await
            .map_err(StoreError::from)
    }
    pub async fn list_resource_overrides(&self) -> Result<Vec<ResourceOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        list_rules::<ResourceOverrideRule, _>(&self.cache, RESOURCE_OVERRIDES_STATE_TYPE, |a, b| {
            (a.instance_id, a.uri.as_str()).cmp(&(b.instance_id, b.uri.as_str()))
        })
        .await
    }
    pub async fn enable_resource(&self, i: InstanceId, u: &str) -> Result<()> {
        self.patch_component_enabled(ComponentKind::Resource, i, u, true)
            .await
    }
    pub async fn disable_resource(&self, i: InstanceId, u: &str) -> Result<()> {
        self.patch_component_enabled(ComponentKind::Resource, i, u, false)
            .await
    }
    pub(crate) async fn load_resource_override(
        &self,
        i: InstanceId,
        u: &str,
    ) -> Result<Option<ResourceOverrideRule>> {
        load_rule(&self.cache, RESOURCE_OVERRIDES_STATE_TYPE, i, u, "Resource").await
    }
    pub(crate) async fn apply_resource_override(
        &self,
        i: InstanceId,
        r: &DiscoveredResource,
    ) -> Result<Option<serde_json::Value>> {
        apply_resource(&self.load_resource_override(i, &r.uri).await?, r)
    }

    pub async fn set_resource_template_override(
        &self,
        i: InstanceId,
        u: &str,
        patch: ResourceTemplateOverridePatch,
    ) -> Result<ResourceTemplateOverrideRule> {
        self.refresh_from_db_if_needed().await?;
        let instance = self.require_instance(i).await?;
        let loaded = self.load_resource_template_override(i, u).await?;
        let expected = loaded.as_ref().map(|r| r.version);
        let mut rule = loaded.unwrap_or(ResourceTemplateOverrideRule {
            instance_id: i,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            uri_template: u.into(),
            common: ComponentOverrideCommon::default(),
            mime_type: None,
            updated_at: 0,
            version: 0,
        });
        rule.service_name = instance.service_name;
        rule.scope = instance.scope;
        rule.uri_template = u.into();
        rule.common = patch.common;
        rule.common.display_name = rule.common.display_name.filter(|v| !v.trim().is_empty());
        rule.mime_type = patch.mime_type;
        rule.updated_at = Self::now_timestamp();
        rule.version += 1;
        self.cache
            .compare_and_put_state(
                RESOURCE_TEMPLATE_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(i, u),
                expected,
                serde_json::to_value(&rule).map_err(|e| StoreError::Other(e.to_string()))?,
            )
            .await?;
        Ok(rule)
    }
    pub async fn get_resource_template_override(
        &self,
        i: InstanceId,
        u: &str,
    ) -> Result<Option<ResourceTemplateOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        self.load_resource_template_override(i, u).await
    }
    pub async fn delete_resource_template_override(&self, i: InstanceId, u: &str) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        self.cache
            .delete_state(
                RESOURCE_TEMPLATE_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(i, u),
            )
            .await
            .map_err(StoreError::from)
    }
    pub async fn list_resource_template_overrides(
        &self,
    ) -> Result<Vec<ResourceTemplateOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        list_rules::<ResourceTemplateOverrideRule, _>(
            &self.cache,
            RESOURCE_TEMPLATE_OVERRIDES_STATE_TYPE,
            |a, b| {
                (a.instance_id, a.uri_template.as_str())
                    .cmp(&(b.instance_id, b.uri_template.as_str()))
            },
        )
        .await
    }
    pub async fn enable_resource_template(&self, i: InstanceId, u: &str) -> Result<()> {
        self.patch_component_enabled(ComponentKind::ResourceTemplate, i, u, true)
            .await
    }
    pub async fn disable_resource_template(&self, i: InstanceId, u: &str) -> Result<()> {
        self.patch_component_enabled(ComponentKind::ResourceTemplate, i, u, false)
            .await
    }
    pub(crate) async fn load_resource_template_override(
        &self,
        i: InstanceId,
        u: &str,
    ) -> Result<Option<ResourceTemplateOverrideRule>> {
        load_rule(
            &self.cache,
            RESOURCE_TEMPLATE_OVERRIDES_STATE_TYPE,
            i,
            u,
            "Resource template",
        )
        .await
    }
    pub(crate) async fn apply_resource_template_override(
        &self,
        i: InstanceId,
        r: &DiscoveredResourceTemplate,
    ) -> Result<Option<serde_json::Value>> {
        apply_resource(
            &self
                .load_resource_template_override(i, &r.uri_template)
                .await?,
            r,
        )
    }
}

async fn load_rule<T: for<'de> Deserialize<'de>>(
    cache: &std::sync::Arc<crate::cache::CacheLayerManager>,
    state: &str,
    i: InstanceId,
    key: &str,
    label: &str,
) -> Result<Option<T>> {
    cache
        .get_state(state, &format!("{i}:{key}"))
        .await?
        .map(|v| {
            serde_json::from_value(v).map_err(|e| {
                StoreError::Other(format!("{label} override deserialization failed: {e}"))
            })
        })
        .transpose()
}
async fn list_rules<T: for<'de> Deserialize<'de>, F: FnMut(&T, &T) -> std::cmp::Ordering>(
    cache: &std::sync::Arc<crate::cache::CacheLayerManager>,
    state: &str,
    mut cmp: F,
) -> Result<Vec<T>> {
    let mut out = cache
        .get_all_states_async(state)
        .await?
        .into_iter()
        .map(|(_, v)| serde_json::from_value(v).map_err(|e| StoreError::Other(e.to_string())))
        .collect::<Result<Vec<T>>>()?;
    out.sort_by(|a, b| cmp(a, b));
    Ok(out)
}
fn apply_resource<T: Serialize>(
    rule: &Option<impl RuleCommon>,
    item: &T,
) -> Result<Option<serde_json::Value>> {
    let mut value = serde_json::to_value(item).map_err(|e| StoreError::Other(e.to_string()))?;
    if let Some(rule) = rule {
        if !is_override_enabled(rule.common()) {
            return Ok(None);
        };
        if let serde_json::Value::Object(o) = &mut value {
            apply_meta_override(o, rule.common());
            if let Some(mt) = rule.mime_type() {
                o.insert("mimeType".into(), mt.into());
            }
        }
    }
    Ok(Some(value))
}
trait RuleCommon {
    fn common(&self) -> &ComponentOverrideCommon;
    fn mime_type(&self) -> Option<String>;
}
impl RuleCommon for ResourceOverrideRule {
    fn common(&self) -> &ComponentOverrideCommon {
        &self.common
    }
    fn mime_type(&self) -> Option<String> {
        self.mime_type.clone()
    }
}
impl RuleCommon for ResourceTemplateOverrideRule {
    fn common(&self) -> &ComponentOverrideCommon {
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
        let p = ResourceOverridePatch::default()
            .with_display_name("x")
            .with_mime_type("text/plain");
        assert_eq!(p.common.display_name.as_deref(), Some("x"));
        assert_eq!(p.mime_type.as_deref(), Some("text/plain"));
    }
}
