use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::cache::models::{ToolArgumentTransform, ToolTransformRule, ToolTransformSafetyPolicy};
use crate::openapi_runtime::validate_json_schema_value;
use crate::store::prelude::*;

const TOOL_TRANSFORMS_STATE_TYPE: &str = "tool_transforms";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToolTransformPatch {
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub arguments: Vec<ToolArgumentTransform>,
    #[serde(default)]
    pub safety_policy: Option<ToolTransformSafetyPolicy>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

impl ToolTransformPatch {
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_argument(mut self, argument: ToolArgumentTransform) -> Self {
        self.arguments.push(argument);
        self
    }

    pub fn rename_argument(
        self,
        original_name: impl Into<String>,
        new_name: impl Into<String>,
    ) -> Self {
        self.with_argument(ToolArgumentTransform {
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
        self.with_argument(ToolArgumentTransform {
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
        self.with_argument(ToolArgumentTransform {
            original_name: original_name.into(),
            new_name: None,
            hidden: false,
            default_value: None,
            description: None,
            validation_schema: Some(validation_schema),
        })
    }

    pub fn with_default_safety_policy(mut self) -> Self {
        self.safety_policy = Some(ToolTransformSafetyPolicy::default());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AppliedToolTransform {
    pub display_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl MCPStore {
    pub async fn create_llm_friendly_tool_transform(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        friendly_name: Option<&str>,
        description: Option<&str>,
        hide_technical_params: bool,
        add_safety_policy: bool,
    ) -> Result<ToolTransformRule> {
        let mut patch = ToolTransformPatch::default()
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
                if let Some(default_value) = Self::default_transform_value_for_param(param) {
                    patch = patch.hide_argument(param, default_value);
                }
            }
        }
        if add_safety_policy {
            patch = patch.with_default_safety_policy().with_tag("safe");
        }
        self.set_tool_transform(instance_id, tool_name, patch).await
    }

    pub async fn create_parameter_renamed_tool_transform(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        new_tool_name: Option<&str>,
        parameter_mapping: &[(&str, &str)],
    ) -> Result<ToolTransformRule> {
        let mut patch = ToolTransformPatch::default()
            .with_display_name(
                new_tool_name
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{tool_name}_renamed")),
            )
            .with_tag("parameter-renamed");
        for (original_param, new_param) in parameter_mapping {
            patch = patch.rename_argument(*original_param, *new_param);
        }
        self.set_tool_transform(instance_id, tool_name, patch).await
    }

    pub async fn create_validated_tool_transform(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        new_tool_name: Option<&str>,
        validation_rules: &[(&str, serde_json::Value)],
    ) -> Result<ToolTransformRule> {
        let mut patch = ToolTransformPatch::default()
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
        self.set_tool_transform(instance_id, tool_name, patch).await
    }

    pub async fn set_tool_transform(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        patch: ToolTransformPatch,
    ) -> Result<ToolTransformRule> {
        self.refresh_from_db_if_needed().await?;
        let (instance, original_tool_name) = self
            .resolve_tool_transform_target(instance_id, tool_name)
            .await?;
        Self::validate_tool_transform_patch(&patch)?;
        let loaded = self
            .load_tool_transform(instance_id, &original_tool_name)
            .await?;
        let expected_version = loaded.as_ref().map(|rule| rule.version);
        let now = Self::now_timestamp();
        let mut rule = loaded.unwrap_or_else(|| ToolTransformRule {
            instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            tool_name: original_tool_name.clone(),
            display_name: None,
            description: None,
            arguments: Vec::new(),
            safety_policy: None,
            tags: Vec::new(),
            enabled: true,
            updated_at: now,
            version: 0,
        });

        rule.service_name = instance.service_name;
        rule.scope = instance.scope;
        rule.tool_name = original_tool_name;
        rule.display_name = patch.display_name.filter(|value| !value.trim().is_empty());
        rule.description = patch.description;
        rule.arguments = patch.arguments;
        rule.safety_policy = patch.safety_policy;
        rule.tags = patch.tags;
        rule.enabled = patch.enabled.unwrap_or(true);
        rule.updated_at = now;
        rule.version += 1;
        self.store_tool_transform(&rule, expected_version).await?;
        Ok(rule)
    }

    pub async fn get_tool_transform(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolTransformRule>> {
        self.refresh_from_db_if_needed().await?;
        let (_, original_tool_name) = self
            .resolve_tool_transform_target(instance_id, tool_name)
            .await?;
        self.load_tool_transform(instance_id, &original_tool_name)
            .await
    }

    pub async fn delete_tool_transform(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        let (_, original_tool_name) = self
            .resolve_tool_transform_target(instance_id, tool_name)
            .await?;
        self.cache
            .delete_state(
                TOOL_TRANSFORMS_STATE_TYPE,
                &Self::tool_transform_key(instance_id, &original_tool_name),
            )
            .await?;
        Ok(())
    }

    pub async fn list_tool_transforms(&self) -> Result<Vec<ToolTransformRule>> {
        self.refresh_from_db_if_needed().await?;
        let mut rules = Vec::new();
        for (key, value) in self
            .cache
            .get_all_states_async(TOOL_TRANSFORMS_STATE_TYPE)
            .await?
        {
            let rule: ToolTransformRule = serde_json::from_value(value).map_err(|err| {
                StoreError::Other(format!(
                    "Tool transform deserialization failed for {key}: {err}"
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

    pub(crate) async fn apply_tool_transform(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        fallback_display_name: String,
        description: String,
        input_schema: serde_json::Value,
    ) -> Result<AppliedToolTransform> {
        let Some(rule) = self
            .load_enabled_tool_transform(instance_id, tool_name)
            .await?
        else {
            return Ok(AppliedToolTransform {
                display_name: fallback_display_name,
                description,
                input_schema,
            });
        };
        Ok(AppliedToolTransform {
            display_name: rule.display_name.unwrap_or(fallback_display_name),
            description: rule.description.unwrap_or(description),
            input_schema: Self::transform_input_schema(input_schema, &rule.arguments),
        })
    }

    pub(crate) async fn resolve_transformed_tool_call(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<(InstanceId, String, serde_json::Value)> {
        let original_tool_name = self
            .resolve_original_tool_name_for_instance(instance_id, tool_name)
            .await?;
        let args = match self
            .load_enabled_tool_transform(instance_id, &original_tool_name)
            .await?
        {
            Some(rule) => {
                let args = Self::transform_call_arguments(args, &rule.arguments);
                Self::apply_transform_safety_policy(&args, rule.safety_policy.as_ref())?;
                Self::validate_transformed_call_arguments(&args, &rule.arguments)?;
                args
            }
            None => args,
        };
        Ok((instance_id, original_tool_name, args))
    }

    async fn resolve_tool_transform_target(
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
                .load_enabled_tool_transform(instance_id, &tool.name)
                .await?
            {
                if rule.display_name.as_deref() == Some(tool_name) {
                    return Ok(tool.name.clone());
                }
            }
        }
        Err(StoreError::Other(format!(
            "Tool '{tool_name}' not found in instance '{instance_id}'"
        )))
    }

    async fn load_enabled_tool_transform(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolTransformRule>> {
        Ok(self
            .load_tool_transform(instance_id, tool_name)
            .await?
            .filter(|rule| rule.enabled))
    }

    async fn load_tool_transform(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolTransformRule>> {
        self.cache
            .get_state(
                TOOL_TRANSFORMS_STATE_TYPE,
                &Self::tool_transform_key(instance_id, tool_name),
            )
            .await?
            .map(|value| {
                serde_json::from_value(value).map_err(|err| {
                    StoreError::Other(format!("Tool transform deserialization failed: {err}"))
                })
            })
            .transpose()
    }

    async fn store_tool_transform(
        &self,
        rule: &ToolTransformRule,
        expected_version: Option<u64>,
    ) -> Result<()> {
        self.cache
            .compare_and_put_state(
                TOOL_TRANSFORMS_STATE_TYPE,
                &Self::tool_transform_key(rule.instance_id, &rule.tool_name),
                expected_version,
                serde_json::to_value(rule).map_err(|err| StoreError::Other(err.to_string()))?,
            )
            .await?;
        Ok(())
    }

    fn tool_transform_key(instance_id: InstanceId, tool_name: &str) -> String {
        format!("{instance_id}:{tool_name}")
    }

    fn validate_tool_transform_patch(patch: &ToolTransformPatch) -> Result<()> {
        let mut original_names = HashSet::new();
        let mut exposed_names = HashSet::new();
        for arg in &patch.arguments {
            if arg.original_name.trim().is_empty() {
                return Err(StoreError::Other(
                    "Tool transform argument original_name cannot be empty".to_string(),
                ));
            }
            if !original_names.insert(arg.original_name.clone()) {
                return Err(StoreError::Other(format!(
                    "Duplicate tool transform argument: {}",
                    arg.original_name
                )));
            }
            if let Some(new_name) = arg.new_name.as_deref() {
                if new_name.trim().is_empty() {
                    return Err(StoreError::Other(
                        "Tool transform argument new_name cannot be empty".to_string(),
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
                    "Tool transform validation_schema for {} must be a JSON object",
                    arg.original_name
                )));
            }
        }
        if let Some(policy) = patch.safety_policy.as_ref() {
            Self::validate_transform_safety_policy(policy)?;
        }
        Ok(())
    }

    fn validate_transform_safety_policy(policy: &ToolTransformSafetyPolicy) -> Result<()> {
        if policy.reject_dangerous_argument_names
            && policy
                .dangerous_argument_name_patterns
                .iter()
                .any(|pattern| pattern.trim().is_empty())
        {
            return Err(StoreError::Other(
                "Tool transform safety policy patterns cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn default_transform_value_for_param(param_name: &str) -> Option<serde_json::Value> {
        match param_name {
            "timeout" => Some(serde_json::json!(30.0)),
            "retry_count" => Some(serde_json::json!(3)),
            "debug" | "verbose" | "raw_output" => Some(serde_json::json!(false)),
            _ => None,
        }
    }

    fn transform_input_schema(
        mut schema: serde_json::Value,
        arguments: &[ToolArgumentTransform],
    ) -> serde_json::Value {
        let transforms = arguments
            .iter()
            .map(|arg| (arg.original_name.as_str(), arg))
            .collect::<HashMap<_, _>>();
        if let Some(properties) = schema
            .get_mut("properties")
            .and_then(serde_json::Value::as_object_mut)
        {
            let original = std::mem::take(properties);
            for (name, mut property) in original {
                let Some(transform) = transforms.get(name.as_str()) else {
                    properties.insert(name, property);
                    continue;
                };
                if transform.hidden {
                    continue;
                }
                if let Some(description) = transform.description.as_ref() {
                    if let serde_json::Value::Object(property_object) = &mut property {
                        property_object.insert(
                            "description".to_string(),
                            serde_json::Value::String(description.clone()),
                        );
                    }
                }
                if let Some(validation_schema) = transform.validation_schema.as_ref() {
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
                properties.insert(transform.new_name.clone().unwrap_or(name), property);
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
                match transforms.get(name) {
                    Some(transform) if transform.hidden => {}
                    Some(transform) => rewritten.push(serde_json::Value::String(
                        transform
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

    fn transform_call_arguments(
        args: serde_json::Value,
        arguments: &[ToolArgumentTransform],
    ) -> serde_json::Value {
        let serde_json::Value::Object(mut input) = args else {
            return args;
        };
        for transform in arguments {
            if transform.hidden {
                input.remove(&transform.original_name);
                if let Some(new_name) = transform.new_name.as_deref() {
                    input.remove(new_name);
                }
                if let Some(default_value) = transform.default_value.clone() {
                    input.insert(transform.original_name.clone(), default_value);
                }
                continue;
            }
            if let Some(new_name) = transform.new_name.as_deref() {
                if let Some(value) = input.remove(new_name) {
                    input.insert(transform.original_name.clone(), value);
                }
            }
        }
        serde_json::Value::Object(input)
    }

    fn validate_transformed_call_arguments(
        args: &serde_json::Value,
        arguments: &[ToolArgumentTransform],
    ) -> Result<()> {
        let serde_json::Value::Object(input) = args else {
            return Ok(());
        };
        let mut errors = Vec::new();
        for transform in arguments {
            let Some(schema) = transform.validation_schema.as_ref() else {
                continue;
            };
            let Some(value) = input.get(&transform.original_name) else {
                continue;
            };
            validate_json_schema_value(
                schema,
                value,
                &format!("arguments.{}", transform.original_name),
                &mut errors,
            );
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(StoreError::Other(format!(
                "Tool transform validation failed: {}",
                errors.join("; ")
            )))
        }
    }

    fn apply_transform_safety_policy(
        args: &serde_json::Value,
        policy: Option<&ToolTransformSafetyPolicy>,
    ) -> Result<()> {
        let Some(policy) = policy else {
            return Ok(());
        };
        if !policy.reject_dangerous_argument_names {
            return Ok(());
        }
        let serde_json::Value::Object(input) = args else {
            return Err(StoreError::Other(
                "Tool transform safety policy requires object arguments".to_string(),
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
                    "Tool transform safety policy rejected argument '{argument_name}' matching '{pattern}'"
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
    fn transform_call_arguments_renames_and_injects_hidden_defaults() {
        let args = serde_json::json!({"message": "hi", "debug": true, "extra": 1});
        let transformed = MCPStore::transform_call_arguments(
            args,
            &[
                ToolArgumentTransform {
                    original_name: "text".to_string(),
                    new_name: Some("message".to_string()),
                    hidden: false,
                    default_value: None,
                    description: None,
                    validation_schema: None,
                },
                ToolArgumentTransform {
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
            transformed,
            serde_json::json!({"text": "hi", "debug": false, "extra": 1})
        );
    }

    #[test]
    fn transformed_arguments_validate_declarative_schema() {
        let args = MCPStore::transform_call_arguments(
            serde_json::json!({"message": "hi"}),
            &[ToolArgumentTransform {
                original_name: "text".to_string(),
                new_name: Some("message".to_string()),
                hidden: false,
                default_value: None,
                description: None,
                validation_schema: Some(serde_json::json!({"type": "string", "minLength": 3})),
            }],
        );

        let err = MCPStore::validate_transformed_call_arguments(
            &args,
            &[ToolArgumentTransform {
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

        assert!(err.contains("Tool transform validation failed"));
        assert!(err.contains("arguments.text length must be at least 3"));
    }

    #[test]
    fn transform_safety_policy_rejects_dangerous_argument_names() {
        MCPStore::apply_transform_safety_policy(
            &serde_json::json!({"city": "Paris"}),
            Some(&ToolTransformSafetyPolicy::default()),
        )
        .unwrap();

        let err = MCPStore::apply_transform_safety_policy(
            &serde_json::json!({"__import__": "os"}),
            Some(&ToolTransformSafetyPolicy::default()),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("Tool transform safety policy rejected argument '__import__'"));
    }
}
