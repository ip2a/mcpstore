use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::routing::{ComponentKind, RouteFields};
pub type ToolArgumentRoute = crate::kv::models::ToolArgumentRoute;
pub type ToolRouteSafetyPolicy = crate::kv::models::ToolRouteSafetyPolicy;
use crate::openapi_runtime::validate_json_schema_value;
use crate::store::prelude::*;

pub(crate) const TOOL_ROUTES_STATE_TYPE: &str = "tool_routes";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToolRoutePatch {
    #[serde(flatten)]
    pub common: RouteFields,
    #[serde(default)]
    pub arguments: Vec<ToolArgumentRoute>,
    #[serde(default)]
    pub safety_policy: Option<ToolRouteSafetyPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolRouteRule {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub tool_name: String,
    #[serde(flatten)]
    pub common: RouteFields,
    #[serde(default)]
    pub arguments: Vec<ToolArgumentRoute>,
    #[serde(default)]
    pub safety_policy: Option<ToolRouteSafetyPolicy>,
    pub updated_at: i64,
    pub version: u64,
}

impl ToolRoutePatch {
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.common.display_name = Some(display_name.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.common.description = Some(description.into());
        self
    }

    pub fn with_argument(mut self, argument: ToolArgumentRoute) -> Self {
        self.arguments.push(argument);
        self
    }

    pub fn rename_argument(
        self,
        original_name: impl Into<String>,
        new_name: impl Into<String>,
    ) -> Self {
        self.with_argument(ToolArgumentRoute {
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
        self.with_argument(ToolArgumentRoute {
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
        self.with_argument(ToolArgumentRoute {
            original_name: original_name.into(),
            new_name: None,
            hidden: false,
            default_value: None,
            description: None,
            validation_schema: Some(validation_schema),
        })
    }

    pub fn with_default_safety_policy(mut self) -> Self {
        self.safety_policy = Some(ToolRouteSafetyPolicy::default());
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

    pub fn with_weight(mut self, weight: i32) -> Self {
        self.common.weight = weight;
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AppliedToolRoute {
    pub display_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub icons: Option<serde_json::Value>,
}

/// Pure route application shared by every listing path: `None` rule passes
/// the tool through untouched. Callers must skip hidden tools
/// (`enabled == Some(false)`) before calling this — hide is visibility, not
/// passthrough.
pub(crate) fn applied_route(
    fallback_display_name: String,
    description: String,
    input_schema: serde_json::Value,
    fallback_icons: Option<serde_json::Value>,
    rule: Option<&ToolRouteRule>,
) -> AppliedToolRoute {
    let Some(rule) = rule else {
        return AppliedToolRoute {
            display_name: fallback_display_name,
            description,
            input_schema,
            icons: fallback_icons,
        };
    };
    AppliedToolRoute {
        display_name: rule
            .common
            .display_name
            .clone()
            .unwrap_or(fallback_display_name),
        description: rule.common.description.clone().unwrap_or(description),
        input_schema: MCPStore::route_input_schema(input_schema, &rule.arguments),
        icons: rule.common.icons.clone().or(fallback_icons),
    }
}

impl MCPStore {
    pub async fn create_llm_friendly_tool_route(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        friendly_name: Option<&str>,
        description: Option<&str>,
        hide_technical_params: bool,
        add_safety_policy: bool,
    ) -> Result<ToolRouteRule> {
        let mut patch = ToolRoutePatch::default()
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
                if let Some(default_value) = Self::default_route_value_for_param(param) {
                    patch = patch.hide_argument(param, default_value);
                }
            }
        }
        if add_safety_policy {
            patch = patch.with_default_safety_policy().with_tag("safe");
        }
        self.set_tool_route(instance_id, tool_name, patch).await
    }

    pub async fn create_parameter_renamed_tool_route(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        new_tool_name: Option<&str>,
        parameter_mapping: &[(&str, &str)],
    ) -> Result<ToolRouteRule> {
        let mut patch = ToolRoutePatch::default()
            .with_display_name(
                new_tool_name
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{tool_name}_renamed")),
            )
            .with_tag("parameter-renamed");
        for (original_param, new_param) in parameter_mapping {
            patch = patch.rename_argument(*original_param, *new_param);
        }
        self.set_tool_route(instance_id, tool_name, patch).await
    }

    pub async fn create_validated_tool_route(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        new_tool_name: Option<&str>,
        validation_rules: &[(&str, serde_json::Value)],
    ) -> Result<ToolRouteRule> {
        let mut patch = ToolRoutePatch::default()
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
        self.set_tool_route(instance_id, tool_name, patch).await
    }

    pub async fn set_tool_route(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        patch: ToolRoutePatch,
    ) -> Result<ToolRouteRule> {
        let (instance, original_tool_name) = self
            .resolve_tool_route_target(instance_id, tool_name)
            .await?;
        Self::validate_tool_route_patch(&patch)?;
        let loaded = self
            .load_tool_route(instance_id, &original_tool_name)
            .await?;
        let expected_version = loaded.as_ref().map(|rule| rule.version);
        let now = Self::now_timestamp();
        let mut rule = loaded.unwrap_or_else(|| ToolRouteRule {
            instance_id,
            service_name: instance.service_name.clone(),
            scope: instance.scope.clone(),
            tool_name: original_tool_name.clone(),
            common: RouteFields::default(),
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
        self.store_tool_route(&rule, expected_version).await?;

        // Best-effort writeback to route.toml; openkeyv above is the source of truth.
        if let Err(error) = self
            .writeback_tool_route(
                &rule.service_name,
                &rule.tool_name,
                &rule.scope,
                &rule.arguments,
                &rule.common,
            )
            .await
        {
            tracing::warn!("Failed to write back tool route to route.toml: {error}");
        }
        // Publish after the writeback so projection consumers refresh promptly.
        self.publish_route_changed(instance_id, &rule.service_name)
            .await;

        Ok(rule)
    }

    pub async fn get_tool_route(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolRouteRule>> {
        let (_, original_tool_name) = self
            .resolve_tool_route_target(instance_id, tool_name)
            .await?;
        self.load_tool_route(instance_id, &original_tool_name)
            .await
    }

    pub async fn remove_tool_route(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<()> {
        let (_, original_tool_name) = self
            .resolve_tool_route_target(instance_id, tool_name)
            .await?;
        self.kernel
            .persistence
            .kv
            .delete_state(
                TOOL_ROUTES_STATE_TYPE,
                &Self::component_route_key(instance_id, &original_tool_name),
            )
            .await?;
        // Best-effort route.toml cleanup so the next load cannot resurrect
        // the deleted rule from its writeback (same contract as the service
        // override delete).
        let mut service_name = None;
        if let Some(instance) = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
        {
            service_name = Some(instance.service_name.clone());
            if let Err(error) = self
                .writeback_tool_route_removal(
                    &instance.service_name,
                    &original_tool_name,
                    &instance.scope,
                )
                .await
            {
                tracing::warn!("Failed to clean up tool route in route.toml: {error}");
            }
        }
        if let Some(service_name) = service_name.as_deref() {
            self.publish_route_changed(instance_id, service_name)
                .await;
        }
        Ok(())
    }

    pub async fn list_tool_routes(&self) -> Result<Vec<ToolRouteRule>> {
        let mut rules = Vec::new();
        for (key, value) in self
            .kernel
            .persistence
            .kv
            .get_all_states_async(TOOL_ROUTES_STATE_TYPE)
            .await?
        {
            let rule: ToolRouteRule = serde_json::from_value(value).map_err(|err| {
                Error::new(
                    FailureCode::Internal,
                    format!("Tool route deserialization failed for {key}: {err}"),
                )
            })?;
            rules.push(rule);
        }
        rules.sort_by(|left, right| {
            (left.instance_id, left.tool_name.as_str())
                .cmp(&(right.instance_id, right.tool_name.as_str()))
        });
        Ok(rules)
    }

    /// Catalog-path variant: unreadable rules are logged and skipped so one
    /// corrupt row cannot blank the whole gateway listing. Query path: reads
    /// the shared store directly, no registry hydration.
    pub async fn list_tool_routes_lenient(&self) -> Result<Vec<ToolRouteRule>> {
        let mut rules = Vec::new();
        for (key, value) in self
            .kernel
            .persistence
            .kv
            .get_all_states_async(TOOL_ROUTES_STATE_TYPE)
            .await?
        {
            match serde_json::from_value::<ToolRouteRule>(value) {
                Ok(rule) => rules.push(rule),
                Err(err) => {
                    tracing::warn!(key = %key, %err, "skipping corrupt tool route rule");
                }
            }
        }
        rules.sort_by(|left, right| {
            (left.instance_id, left.tool_name.as_str())
                .cmp(&(right.instance_id, right.tool_name.as_str()))
        });
        Ok(rules)
    }

    /// Route rules of one instance (includes hidden `enabled=false`).
    /// Tool-name domain comes from the synced projection; rules are batched.
    /// Catalog projection drops hidden tools; callers must not treat this map
    /// as "active rename rules only".
    pub(crate) async fn tool_routes_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<HashMap<String, ToolRouteRule>> {
        let tools = self.tools_from_kv(instance_id).await?;
        let keys: Vec<String> = tools
            .iter()
            .map(|tool| Self::component_route_key(instance_id, &tool.name))
            .collect();
        let values = self
            .kernel
            .persistence
            .kv
            .get_states_many(TOOL_ROUTES_STATE_TYPE, &keys)
            .await?;
        let mut rules = HashMap::new();
        for (tool, value) in tools.iter().zip(values) {
            let Some(value) = value else {
                continue;
            };
            let rule: ToolRouteRule = serde_json::from_value(value).map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!("Tool route deserialization failed: {error}"),
                )
            })?;
            // Include disabled (hidden) rules so catalog projection can drop
            // those tools from the gateway view.
            rules.insert(tool.name.clone(), rule);
        }
        Ok(rules)
    }

    /// Route rules for every instance in ONE shared-store read, keyed by
    /// instance then raw tool name (includes hidden `enabled=false` rules so
    /// catalogs can drop them). Scope-wide catalogs fan out over many
    /// instances; a per-instance read would rescan the collection each time.
    pub(crate) async fn tool_routes_by_instance(
        &self,
    ) -> Result<HashMap<InstanceId, HashMap<String, ToolRouteRule>>> {
        let mut by_instance: HashMap<InstanceId, HashMap<String, ToolRouteRule>> =
            HashMap::new();
        for (key, value) in self
            .kernel
            .persistence
            .kv
            .get_all_states_async(TOOL_ROUTES_STATE_TYPE)
            .await?
        {
            match serde_json::from_value::<ToolRouteRule>(value) {
                Ok(rule) => {
                    by_instance
                        .entry(rule.instance_id)
                        .or_default()
                        .insert(rule.tool_name.clone(), rule);
                }
                Err(error) => {
                    tracing::warn!(key = %key, %error, "skipping corrupt tool route rule");
                }
            }
        }
        Ok(by_instance)
    }

    pub(crate) async fn resolve_routed_tool_call(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<(InstanceId, String, serde_json::Value)> {
        let original_tool_name = self
            .resolve_original_tool_name_for_instance(instance_id, tool_name)
            .await?;
        let args = match self
            .load_enabled_tool_route(instance_id, &original_tool_name)
            .await?
        {
            Some(rule) => {
                let args = Self::route_call_arguments(args, &rule.arguments);
                Self::apply_route_safety_policy(&args, rule.safety_policy.as_ref())?;
                Self::validate_route_call_arguments(&args, &rule.arguments)?;
                args
            }
            None => args,
        };
        Ok((instance_id, original_tool_name, args))
    }

    async fn resolve_tool_route_target(
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
        let tools = self.tools_from_kv(instance_id).await?;
        if tools.iter().any(|tool| tool.name == tool_name) {
            return Ok(tool_name.to_string());
        }
        for tool in &tools {
            if let Some(rule) = self
                .load_enabled_tool_route(instance_id, &tool.name)
                .await?
            {
                if rule.common.display_name.as_deref() == Some(tool_name) {
                    return Ok(tool.name.clone());
                }
            }
        }
        Err(Error::new(
            FailureCode::Internal,
            format!("Tool '{tool_name}' not found in instance '{instance_id}'"),
        ))
    }

    async fn load_enabled_tool_route(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolRouteRule>> {
        Ok(self
            .load_tool_route(instance_id, tool_name)
            .await?
            .filter(|rule| rule.common.enabled.unwrap_or(true)))
    }

    pub(crate) async fn load_tool_route(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Result<Option<ToolRouteRule>> {
        self.kernel
            .persistence
            .kv
            .get_state(
                TOOL_ROUTES_STATE_TYPE,
                &Self::component_route_key(instance_id, tool_name),
            )
            .await?
            .map(|value| {
                serde_json::from_value(value).map_err(|err| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Tool route deserialization failed: {err}"),
                    )
                })
            })
            .transpose()
    }

    async fn store_tool_route(
        &self,
        rule: &ToolRouteRule,
        expected_version: Option<u64>,
    ) -> Result<()> {
        self.kernel
            .persistence
            .kv
            .compare_and_put_state(
                TOOL_ROUTES_STATE_TYPE,
                &Self::component_route_key(rule.instance_id, &rule.tool_name),
                expected_version,
                serde_json::to_value(rule)
                    .map_err(|err| Error::new(FailureCode::Internal, err.to_string()))?,
            )
            .await?;
        Ok(())
    }

    pub async fn enable_tool(&self, instance_id: InstanceId, tool_name: &str) -> Result<()> {
        self.patch_component_visibility(ComponentKind::Tool, instance_id, tool_name, true)
            .await
    }

    pub async fn disable_tool(&self, instance_id: InstanceId, tool_name: &str) -> Result<()> {
        self.patch_component_visibility(ComponentKind::Tool, instance_id, tool_name, false)
            .await
    }

    fn validate_tool_route_patch(patch: &ToolRoutePatch) -> Result<()> {
        let mut original_names = HashSet::new();
        let mut exposed_names = HashSet::new();
        for arg in &patch.arguments {
            if arg.original_name.trim().is_empty() {
                return Err(Error::new(
                    FailureCode::Internal,
                    "Tool route argument original_name cannot be empty".to_string(),
                ));
            }
            if !original_names.insert(arg.original_name.clone()) {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!(
                        "Duplicate tool argument_route argument: {}",
                        arg.original_name
                    ),
                ));
            }
            if let Some(new_name) = arg.new_name.as_deref() {
                if new_name.trim().is_empty() {
                    return Err(Error::new(
                        FailureCode::Internal,
                        "Tool route argument new_name cannot be empty".to_string(),
                    ));
                }
                if !arg.hidden && !exposed_names.insert(new_name.to_string()) {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("Duplicate exposed tool argument: {new_name}"),
                    ));
                }
            }
            if matches!(arg.validation_schema.as_ref(), Some(schema) if !schema.is_object()) {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!(
                        "Tool route validation_schema for {} must be a JSON object",
                        arg.original_name
                    ),
                ));
            }
        }
        if let Some(policy) = patch.safety_policy.as_ref() {
            Self::validate_route_safety_policy(policy)?;
        }
        Ok(())
    }

    fn validate_route_safety_policy(policy: &ToolRouteSafetyPolicy) -> Result<()> {
        if policy.reject_dangerous_argument_names
            && policy
                .dangerous_argument_name_patterns
                .iter()
                .any(|pattern| pattern.trim().is_empty())
        {
            return Err(Error::new(
                FailureCode::Internal,
                "Tool route safety policy patterns cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn default_route_value_for_param(param_name: &str) -> Option<serde_json::Value> {
        match param_name {
            "timeout" => Some(serde_json::json!(30.0)),
            "retry_count" => Some(serde_json::json!(3)),
            "debug" | "verbose" | "raw_output" => Some(serde_json::json!(false)),
            _ => None,
        }
    }

    fn route_input_schema(
        mut schema: serde_json::Value,
        arguments: &[ToolArgumentRoute],
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
                let Some(argument_route) = overrides.get(name.as_str()) else {
                    properties.insert(name, property);
                    continue;
                };
                if argument_route.hidden {
                    continue;
                }
                if let Some(description) = argument_route.description.as_ref() {
                    if let serde_json::Value::Object(property_object) = &mut property {
                        property_object.insert(
                            "description".to_string(),
                            serde_json::Value::String(description.clone()),
                        );
                    }
                }
                if let Some(validation_schema) = argument_route.validation_schema.as_ref() {
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
                properties.insert(argument_route.new_name.clone().unwrap_or(name), property);
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
                    Some(argument_route) if argument_route.hidden => {}
                    Some(argument_route) => rewritten.push(serde_json::Value::String(
                        argument_route
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

    fn route_call_arguments(
        args: serde_json::Value,
        arguments: &[ToolArgumentRoute],
    ) -> serde_json::Value {
        let serde_json::Value::Object(mut input) = args else {
            return args;
        };
        for argument_route in arguments {
            if argument_route.hidden {
                input.remove(&argument_route.original_name);
                if let Some(new_name) = argument_route.new_name.as_deref() {
                    input.remove(new_name);
                }
                if let Some(default_value) = argument_route.default_value.clone() {
                    input.insert(argument_route.original_name.clone(), default_value);
                }
                continue;
            }
            if let Some(new_name) = argument_route.new_name.as_deref() {
                if let Some(value) = input.remove(new_name) {
                    input.insert(argument_route.original_name.clone(), value);
                }
            }
        }
        serde_json::Value::Object(input)
    }

    fn validate_route_call_arguments(
        args: &serde_json::Value,
        arguments: &[ToolArgumentRoute],
    ) -> Result<()> {
        let serde_json::Value::Object(input) = args else {
            return Ok(());
        };
        let mut errors = Vec::new();
        for argument_route in arguments {
            let Some(schema) = argument_route.validation_schema.as_ref() else {
                continue;
            };
            let Some(value) = input.get(&argument_route.original_name) else {
                continue;
            };
            validate_json_schema_value(
                schema,
                value,
                &format!("arguments.{}", argument_route.original_name),
                &mut errors,
            );
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::new(
                FailureCode::Internal,
                format!("Tool route validation failed: {}", errors.join("; ")),
            ))
        }
    }

    fn apply_route_safety_policy(
        args: &serde_json::Value,
        policy: Option<&ToolRouteSafetyPolicy>,
    ) -> Result<()> {
        let Some(policy) = policy else {
            return Ok(());
        };
        if !policy.reject_dangerous_argument_names {
            return Ok(());
        }
        let serde_json::Value::Object(input) = args else {
            return Err(Error::new(
                FailureCode::Internal,
                "Tool route safety policy requires object arguments".to_string(),
            ));
        };
        for argument_name in input.keys() {
            let normalized = argument_name.to_lowercase();
            if let Some(pattern) = policy
                .dangerous_argument_name_patterns
                .iter()
                .find(|pattern| normalized.contains(&pattern.to_lowercase()))
            {
                return Err(Error::new(FailureCode::Internal, format!(
                    "Tool route safety policy rejected argument '{argument_name}' matching '{pattern}'"
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
    fn route_call_arguments_renames_and_injects_hidden_defaults() {
        let args = serde_json::json!({"message": "hi", "debug": true, "extra": 1});
        let routed = MCPStore::route_call_arguments(
            args,
            &[
                ToolArgumentRoute {
                    original_name: "text".to_string(),
                    new_name: Some("message".to_string()),
                    hidden: false,
                    default_value: None,
                    description: None,
                    validation_schema: None,
                },
                ToolArgumentRoute {
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
            routed,
            serde_json::json!({"text": "hi", "debug": false, "extra": 1})
        );
    }

    #[test]
    fn overridden_arguments_validate_declarative_schema() {
        let args = MCPStore::route_call_arguments(
            serde_json::json!({"message": "hi"}),
            &[ToolArgumentRoute {
                original_name: "text".to_string(),
                new_name: Some("message".to_string()),
                hidden: false,
                default_value: None,
                description: None,
                validation_schema: Some(serde_json::json!({"type": "string", "minLength": 3})),
            }],
        );

        let err = MCPStore::validate_route_call_arguments(
            &args,
            &[ToolArgumentRoute {
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

        assert!(err.contains("Tool route validation failed"));
        assert!(err.contains("arguments.text length must be at least 3"));
    }

    #[test]
    fn override_safety_policy_rejects_dangerous_argument_names() {
        MCPStore::apply_route_safety_policy(
            &serde_json::json!({"city": "Paris"}),
            Some(&ToolRouteSafetyPolicy::default()),
        )
        .unwrap();

        let err = MCPStore::apply_route_safety_policy(
            &serde_json::json!({"__import__": "os"}),
            Some(&ToolRouteSafetyPolicy::default()),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("Tool route safety policy rejected argument '__import__'"));
    }
}
