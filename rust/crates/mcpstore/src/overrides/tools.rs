use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::overrides::{ComponentKind, ComponentOverrideCommon};
pub type ToolArgumentOverride = crate::cache::models::ToolArgumentOverride;
pub type ToolOverrideSafetyPolicy = crate::cache::models::ToolOverrideSafetyPolicy;
use crate::openapi_runtime::validate_json_schema_value;
use crate::store::prelude::*;

const TOOL_OVERRIDES_STATE_TYPE: &str = "tool_overrides";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToolOverridePatch {
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
    #[serde(default)]
    pub arguments: Vec<ToolArgumentOverride>,
    #[serde(default)]
    pub safety_policy: Option<ToolOverrideSafetyPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolOverrideRule {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub tool_name: String,
    #[serde(flatten)]
    pub common: ComponentOverrideCommon,
    #[serde(default)]
    pub arguments: Vec<ToolArgumentOverride>,
    #[serde(default)]
    pub safety_policy: Option<ToolOverrideSafetyPolicy>,
    pub updated_at: i64,
    pub version: u64,
}

impl ToolOverridePatch {
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.common.display_name = Some(display_name.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.common.description = Some(description.into());
        self
    }

    pub fn with_argument(mut self, argument: ToolArgumentOverride) -> Self {
        self.arguments.push(argument);
        self
    }

    pub fn rename_argument(
        self,
        original_name: impl Into<String>,
        new_name: impl Into<String>,
    ) -> Self {
        self.with_argument(ToolArgumentOverride {
            original_name: original_name.into(),
            new_name: Some(new_name.into()),
            hidden: false,
            default_value: None,
            description: None,
            validation_schema: None,
        })
    }

    pub fn hide_argument(
        self,
        original_name: impl Into<String>,
        default_value: impl Into<serde_json::Value>,
    ) -> Self {
        self.with_argument(ToolArgumentOverride {
            original_name: original_name.into(),
            new_name: None,
            hidden: true,
            default_value: Some(default_value.into()),
            description: None,
            validation_schema: None,
        })
    }

    pub fn validate_argument(
        self,
        original_name: impl Into<String>,
        validation_schema: serde_json::Value,
    ) -> Self {
        self.with_argument(ToolArgumentOverride {
            original_name: original_name.into(),
            new_name: None,
            hidden: false,
            default_value: None,
            description: None,
            validation_schema: Some(validation_schema),
        })
    }

    pub fn with_default_safety_policy(mut self) -> Self {
        self.safety_policy = Some(ToolOverrideSafetyPolicy::default());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.common.tags.push(tag.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.common.enabled = Some(enabled);
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AppliedToolOverride {
    pub display_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl MCPStore {
    pub async fn create_llm_friendly_tool_override(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        friendly_name: Option<&str>,
        description: Option<&str>,
        hide_technical_params: bool,
        add_safety_policy: bool,
    ) -> Result<ToolOverrideRule> {
        let mut patch = ToolOverridePatch::default()
            .with_display_name(
                friendly_name
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{tool_name}_simple")),
            )
            .with_tag("llm-friendly")
            .with_tag("simplified");
        if let Some(description) = description {
            patch = patch.with_description(description);
        }
        if hide_technical_params {
            for param in ["timeout", "retry_count", "debug", "verbose", "raw_output"] {
                if let Some(default_value) = Self::default_override_value_for_param(param) {
                    patch = patch.hide_argument(param, default_value);
                }
            }
        }
        if add_safety_policy {
            patch = patch.with_default_safety_policy().with_tag("safe");
        }
        self.set_tool_override(instance_id, tool_name, patch).await
    }

    pub async fn create_parameter_renamed_tool_override(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        new_tool_name: Option<&str>,
        parameter_mapping: &[(&str, &str)],
    ) -> Result<ToolOverrideRule> {
        let mut patch = ToolOverridePatch::default()
            .with_display_name(
                new_tool_name
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{tool_name}_renamed")),
            )
            .with_tag("parameter-renamed");
        for (original_param, new_param) in parameter_mapping {
            patch = patch.rename_argument(*original_param, *new_param);
        }
        self.set_tool_override(instance_id, tool_name, patch).await
    }

    pub async fn create_validated_tool_override(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        new_tool_name: Option<&str>,
        validation_rules: &[(&str, serde_json::Value)],
    ) -> Result<ToolOverrideRule> {
        let mut patch = ToolOverridePatch::default()
            .with_display_name(
                new_tool_name
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{tool_name}_validated")),
            )
            .with_tag("validated")
            .with_tag("safe");
        for (param_name, validation_schema) in validation_rules {
            patch = patch.validate_argument(*param_name, validation_schema.clone());
        }
        self.set_tool_override(instance_id, tool_name, patch).await
    }

    pub async fn set_tool_override(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        patch: ToolOverridePatch,
    ) -> Result<ToolOverrideRule> {
        self.refresh_from_db_if_needed().await?;
        let (instance, original_tool_name) = self
            .resolve_tool_override_target(instance_id, tool_name)
            .await?;
        Self::validate_tool_override_patch(&patch)?;
        let loaded = self
            .load_tool_override(instance_id, &original_tool_name)
            .await?;
        let expected_version = loaded.as_ref().map(|rule| rule.version);
        let now = Self::now_timestamp();
        let mut rule = loaded.unwrap_or_else(|| ToolOverrideRule {
            instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            tool_name: original_tool_name.clone(),
            common: ComponentOverrideCommon::default(),
            arguments: Vec::new(),
            safety_policy: None,
            updated_at: now,
            version: 0,
        });

        rule.service_name = instance.service_name;
        rule.scope = instance.scope;
        rule.tool_name = original_tool_name;
        rule.common = patch.common;
        rule.common.display_name = rule
            .common
            .display_name
            .filter(|value| !value.trim().is_empty());
        rule.arguments = patch.arguments;
        rule.safety_policy = patch.safety_policy;
        rule.updated_at = now;
        rule.version += 1;
        self.store_tool_override(&rule, expected_version).await?;
        Ok(rule)
    }

    pub async fn get_tool_override(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        let (_, original_tool_name) = self
            .resolve_tool_override_target(instance_id, tool_name)
            .await?;
        self.load_tool_override(instance_id, &original_tool_name)
            .await
    }

    pub async fn delete_tool_override(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        let (_, original_tool_name) = self
            .resolve_tool_override_target(instance_id, tool_name)
            .await?;
        self.cache
            .delete_state(
                TOOL_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(instance_id, &original_tool_name),
            )
            .await?;
        Ok(())
    }

    pub async fn list_tool_overrides(&self) -> Result<Vec<ToolOverrideRule>> {
        self.refresh_from_db_if_needed().await?;
        let mut rules = Vec::new();
        for (key, value) in self
            .cache
            .get_all_states_async(TOOL_OVERRIDES_STATE_TYPE)
            .await?
        {
            let rule: ToolOverrideRule = serde_json::from_value(value).map_err(|err| {
                StoreError::Other(format!(
                    "Tool override deserialization failed for {key}: {err}"
                ))
            })?;
            rules.push(rule);
        }
        rules.sort_by(|left, right| {
            (left.instance_id, left.tool_name.as_str())
                .cmp(&(right.instance_id, right.tool_name.as_str()))
        });
        Ok(rules)
    }

    pub(crate) async fn apply_tool_override(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        fallback_display_name: String,
        description: String,
        input_schema: serde_json::Value,
    ) -> Result<AppliedToolOverride> {
        let Some(rule) = self
            .load_enabled_tool_override(instance_id, tool_name)
            .await?
        else {
            return Ok(AppliedToolOverride {
                display_name: fallback_display_name,
                description,
                input_schema,
            });
        };
        Ok(AppliedToolOverride {
            display_name: rule.common.display_name.unwrap_or(fallback_display_name),
            description: rule.common.description.unwrap_or(description),
            input_schema: Self::override_input_schema(input_schema, &rule.arguments),
        })
    }

    pub(crate) async fn resolve_override_tool_call(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<(InstanceId, String, serde_json::Value)> {
        let original_tool_name = self
            .resolve_original_tool_name_for_instance(instance_id, tool_name)
            .await?;
        let args = match self
            .load_enabled_tool_override(instance_id, &original_tool_name)
            .await?
        {
            Some(rule) => {
                let args = Self::override_call_arguments(args, &rule.arguments);
                Self::apply_override_safety_policy(&args, rule.safety_policy.as_ref())?;
                Self::validate_override_call_arguments(&args, &rule.arguments)?;
                args
            }
            None => args,
        };
        Ok((instance_id, original_tool_name, args))
    }

    async fn resolve_tool_override_target(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<(ServiceInstance, String)> {
        let instance = self.require_instance(instance_id).await?;
        let original_tool_name = self
            .resolve_original_tool_name_for_instance(instance_id, tool_name)
            .await?;
        Ok((instance, original_tool_name))
    }

    async fn resolve_original_tool_name_for_instance(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<String> {
        let instance = self.require_instance(instance_id).await?;
        if instance.tools.iter().any(|tool| tool.name == tool_name) {
            return Ok(tool_name.to_string());
        }
        for tool in &instance.tools {
            if let Some(rule) = self
                .load_enabled_tool_override(instance_id, &tool.name)
                .await?
            {
                if rule.common.display_name.as_deref() == Some(tool_name) {
                    return Ok(tool.name.clone());
                }
            }
        }
        Err(StoreError::Other(format!(
            "Tool '{tool_name}' not found in instance '{instance_id}'"
        )))
    }

    async fn load_enabled_tool_override(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolOverrideRule>> {
        Ok(self
            .load_tool_override(instance_id, tool_name)
            .await?
            .filter(|rule| rule.common.enabled.unwrap_or(true)))
    }

    pub(crate) async fn load_tool_override(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolOverrideRule>> {
        self.cache
            .get_state(
                TOOL_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(instance_id, tool_name),
            )
            .await?
            .map(|value| {
                serde_json::from_value(value).map_err(|err| {
                    StoreError::Other(format!("Tool override deserialization failed: {err}"))
                })
            })
            .transpose()
    }

    async fn store_tool_override(
        &self,
        rule: &ToolOverrideRule,
        expected_version: Option<u64>,
    ) -> Result<()> {
        self.cache
            .compare_and_put_state(
                TOOL_OVERRIDES_STATE_TYPE,
                &Self::component_override_key(rule.instance_id, &rule.tool_name),
                expected_version,
                serde_json::to_value(rule).map_err(|err| StoreError::Other(err.to_string()))?,
            )
            .await?;
        Ok(())
    }

    pub async fn enable_tool(&self, instance_id: InstanceId, tool_name: &str) -> Result<()> {
        self.patch_component_enabled(ComponentKind::Tool, instance_id, tool_name, true)
            .await
    }

    pub async fn disable_tool(&self, instance_id: InstanceId, tool_name: &str) -> Result<()> {
        self.patch_component_enabled(ComponentKind::Tool, instance_id, tool_name, false)
            .await
    }

    fn validate_tool_override_patch(patch: &ToolOverridePatch) -> Result<()> {
        let mut original_names = HashSet::new();
        let mut exposed_names = HashSet::new();
        for arg in &patch.arguments {
            if arg.original_name.trim().is_empty() {
                return Err(StoreError::Other(
                    "Tool override argument original_name cannot be empty".to_string(),
                ));
            }
            if !original_names.insert(arg.original_name.clone()) {
                return Err(StoreError::Other(format!(
                    "Duplicate tool argument_override argument: {}",
                    arg.original_name
                )));
            }
            if let Some(new_name) = arg.new_name.as_deref() {
                if new_name.trim().is_empty() {
                    return Err(StoreError::Other(
                        "Tool override argument new_name cannot be empty".to_string(),
                    ));
                }
                if !arg.hidden && !exposed_names.insert(new_name.to_string()) {
                    return Err(StoreError::Other(format!(
                        "Duplicate exposed tool argument: {new_name}"
                    )));
                }
            }
            if matches!(arg.validation_schema.as_ref(), Some(schema) if !schema.is_object()) {
                return Err(StoreError::Other(format!(
                    "Tool override validation_schema for {} must be a JSON object",
                    arg.original_name
                )));
            }
        }
        if let Some(policy) = patch.safety_policy.as_ref() {
            Self::validate_override_safety_policy(policy)?;
        }
        Ok(())
    }

    fn validate_override_safety_policy(policy: &ToolOverrideSafetyPolicy) -> Result<()> {
        if policy.reject_dangerous_argument_names
            && policy
                .dangerous_argument_name_patterns
                .iter()
                .any(|pattern| pattern.trim().is_empty())
        {
            return Err(StoreError::Other(
                "Tool override safety policy patterns cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn default_override_value_for_param(param_name: &str) -> Option<serde_json::Value> {
        match param_name {
            "timeout" => Some(serde_json::json!(30.0)),
            "retry_count" => Some(serde_json::json!(3)),
            "debug" | "verbose" | "raw_output" => Some(serde_json::json!(false)),
            _ => None,
        }
    }

    fn override_input_schema(
        mut schema: serde_json::Value,
        arguments: &[ToolArgumentOverride],
    ) -> serde_json::Value {
        let overrides = arguments
            .iter()
            .map(|arg| (arg.original_name.as_str(), arg))
            .collect::<HashMap<_, _>>();
        if let Some(properties) = schema
            .get_mut("properties")
            .and_then(serde_json::Value::as_object_mut)
        {
            let original = std::mem::take(properties);
            for (name, mut property) in original {
                let Some(argument_override) = overrides.get(name.as_str()) else {
                    properties.insert(name, property);
                    continue;
                };
                if argument_override.hidden {
                    continue;
                }
                if let Some(description) = argument_override.description.as_ref() {
                    if let serde_json::Value::Object(property_object) = &mut property {
                        property_object.insert(
                            "description".to_string(),
                            serde_json::Value::String(description.clone()),
                        );
                    }
                }
                if let Some(validation_schema) = argument_override.validation_schema.as_ref() {
                    if let (
                        serde_json::Value::Object(property_object),
                        serde_json::Value::Object(validation_object),
                    ) = (&mut property, validation_schema)
                    {
                        for (key, value) in validation_object {
                            property_object.insert(key.clone(), value.clone());
                        }
                    }
                }
                properties.insert(argument_override.new_name.clone().unwrap_or(name), property);
            }
        }
        if let Some(required) = schema
            .get_mut("required")
            .and_then(serde_json::Value::as_array_mut)
        {
            let mut rewritten = Vec::new();
            for value in std::mem::take(required) {
                let Some(name) = value.as_str() else {
                    rewritten.push(value);
                    continue;
                };
                match overrides.get(name) {
                    Some(argument_override) if argument_override.hidden => {}
                    Some(argument_override) => rewritten.push(serde_json::Value::String(
                        argument_override
                            .new_name
                            .clone()
                            .unwrap_or_else(|| name.to_string()),
                    )),
                    None => rewritten.push(value),
                }
            }
            *required = rewritten;
        }
        schema
    }

    fn override_call_arguments(
        args: serde_json::Value,
        arguments: &[ToolArgumentOverride],
    ) -> serde_json::Value {
        let serde_json::Value::Object(mut input) = args else {
            return args;
        };
        for argument_override in arguments {
            if argument_override.hidden {
                input.remove(&argument_override.original_name);
                if let Some(new_name) = argument_override.new_name.as_deref() {
                    input.remove(new_name);
                }
                if let Some(default_value) = argument_override.default_value.clone() {
                    input.insert(argument_override.original_name.clone(), default_value);
                }
                continue;
            }
            if let Some(new_name) = argument_override.new_name.as_deref() {
                if let Some(value) = input.remove(new_name) {
                    input.insert(argument_override.original_name.clone(), value);
                }
            }
        }
        serde_json::Value::Object(input)
    }

    fn validate_override_call_arguments(
        args: &serde_json::Value,
        arguments: &[ToolArgumentOverride],
    ) -> Result<()> {
        let serde_json::Value::Object(input) = args else {
            return Ok(());
        };
        let mut errors = Vec::new();
        for argument_override in arguments {
            let Some(schema) = argument_override.validation_schema.as_ref() else {
                continue;
            };
            let Some(value) = input.get(&argument_override.original_name) else {
                continue;
            };
            validate_json_schema_value(
                schema,
                value,
                &format!("arguments.{}", argument_override.original_name),
                &mut errors,
            );
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(StoreError::Other(format!(
                "Tool override validation failed: {}",
                errors.join("; ")
            )))
        }
    }

    fn apply_override_safety_policy(
        args: &serde_json::Value,
        policy: Option<&ToolOverrideSafetyPolicy>,
    ) -> Result<()> {
        let Some(policy) = policy else {
            return Ok(());
        };
        if !policy.reject_dangerous_argument_names {
            return Ok(());
        }
        let serde_json::Value::Object(input) = args else {
            return Err(StoreError::Other(
                "Tool override safety policy requires object arguments".to_string(),
            ));
        };
        for argument_name in input.keys() {
            let normalized = argument_name.to_lowercase();
            if let Some(pattern) = policy
                .dangerous_argument_name_patterns
                .iter()
                .find(|pattern| normalized.contains(&pattern.to_lowercase()))
            {
                return Err(StoreError::Other(format!(
                    "Tool override safety policy rejected argument '{argument_name}' matching '{pattern}'"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_call_arguments_renames_and_injects_hidden_defaults() {
        let args = serde_json::json!({"message": "hi", "debug": true, "extra": 1});
        let overrideed = MCPStore::override_call_arguments(
            args,
            &[
                ToolArgumentOverride {
                    original_name: "text".to_string(),
                    new_name: Some("message".to_string()),
                    hidden: false,
                    default_value: None,
                    description: None,
                    validation_schema: None,
                },
                ToolArgumentOverride {
                    original_name: "debug".to_string(),
                    new_name: None,
                    hidden: true,
                    default_value: Some(serde_json::json!(false)),
                    description: None,
                    validation_schema: None,
                },
            ],
        );

        assert_eq!(
            overrideed,
            serde_json::json!({"text": "hi", "debug": false, "extra": 1})
        );
    }

    #[test]
    fn overridden_arguments_validate_declarative_schema() {
        let args = MCPStore::override_call_arguments(
            serde_json::json!({"message": "hi"}),
            &[ToolArgumentOverride {
                original_name: "text".to_string(),
                new_name: Some("message".to_string()),
                hidden: false,
                default_value: None,
                description: None,
                validation_schema: Some(serde_json::json!({"type": "string", "minLength": 3})),
            }],
        );

        let err = MCPStore::validate_override_call_arguments(
            &args,
            &[ToolArgumentOverride {
                original_name: "text".to_string(),
                new_name: Some("message".to_string()),
                hidden: false,
                default_value: None,
                description: None,
                validation_schema: Some(serde_json::json!({"type": "string", "minLength": 3})),
            }],
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("Tool override validation failed"));
        assert!(err.contains("arguments.text length must be at least 3"));
    }

    #[test]
    fn override_safety_policy_rejects_dangerous_argument_names() {
        MCPStore::apply_override_safety_policy(
            &serde_json::json!({"city": "Paris"}),
            Some(&ToolOverrideSafetyPolicy::default()),
        )
        .unwrap();

        let err = MCPStore::apply_override_safety_policy(
            &serde_json::json!({"__import__": "os"}),
            Some(&ToolOverrideSafetyPolicy::default()),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("Tool override safety policy rejected argument '__import__'"));
    }
}
