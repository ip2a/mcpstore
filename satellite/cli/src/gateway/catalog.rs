use super::tools::{read_required_instance_id, read_required_string};
use super::*;

pub(super) fn project_prompt_names(mut payloads: Vec<Value>) -> Result<Vec<Value>, ErrorData> {
    let names = catalog_name_counts(&payloads, "name")
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    for payload in &mut payloads {
        let original = read_required_string(payload, "name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        if names.get(&original).copied().unwrap_or_default() < 2 {
            continue;
        }
        let service_name = read_required_string(payload, "service_name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let instance_id = read_required_instance_id(payload, "instance_id")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        payload
            .as_object_mut()
            .ok_or_else(|| ErrorData::internal_error("prompt must be an object", None))?
            .insert(
                "name".to_string(),
                Value::String(format!(
                    "{}__{}",
                    stable_namespace(&service_name, instance_id),
                    original
                )),
            );
    }
    Ok(payloads)
}

pub(super) fn resolve_projected_prompt(
    payloads: &[Value],
    projected_name: &str,
) -> Result<(InstanceId, String, String), ErrorData> {
    let names = catalog_name_counts(payloads, "name")
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    for payload in payloads {
        let original = read_required_string(payload, "name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let instance_id = read_required_instance_id(payload, "instance_id")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let service_name = read_required_string(payload, "service_name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let candidate = if names.get(&original).copied().unwrap_or_default() > 1 {
            format!(
                "{}__{}",
                stable_namespace(&service_name, instance_id),
                original
            )
        } else {
            original.clone()
        };
        if candidate == projected_name {
            return Ok((instance_id, original, service_name.to_string()));
        }
    }
    Err(ErrorData::invalid_params(
        format!("Unknown gateway prompt: {projected_name}"),
        None,
    ))
}

pub(super) fn catalog_name_counts(
    payloads: &[Value],
    field: &str,
) -> Result<HashMap<String, usize>, BoxErr> {
    let mut names = HashMap::new();
    for payload in payloads {
        *names
            .entry(read_required_string(payload, field)?)
            .or_default() += 1;
    }
    Ok(names)
}

pub(super) fn project_catalog_uris(
    mut payloads: Vec<Value>,
    field: &str,
    template: bool,
) -> Result<Vec<Value>, ErrorData> {
    for payload in &mut payloads {
        let projected = projected_catalog_uri(payload, field, template)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let object = payload
            .as_object_mut()
            .ok_or_else(|| ErrorData::internal_error("catalog item must be an object", None))?;
        object.insert(field.to_string(), Value::String(projected));
    }
    Ok(payloads)
}

pub(super) fn resolve_projected_catalog_uri(
    payloads: &[Value],
    field: &str,
    template: bool,
    projected_uri: &str,
) -> Result<(InstanceId, String, String), ErrorData> {
    for payload in payloads {
        let candidate = projected_catalog_uri(payload, field, template)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        if candidate == projected_uri {
            return Ok((
                read_required_instance_id(payload, "instance_id")
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
                read_required_string(payload, field)
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
                read_required_string(payload, "service_name")
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
            ));
        }
    }
    Err(ErrorData::invalid_params(
        format!("Unknown gateway resource URI: {projected_uri}"),
        None,
    ))
}

pub(super) fn projected_catalog_uri(
    payload: &Value,
    field: &str,
    template: bool,
) -> Result<String, BoxErr> {
    let original = read_required_string(payload, field)?;
    let service_name = read_required_string(payload, "service_name")?;
    let instance_id = read_required_instance_id(payload, "instance_id")?;
    let namespace = stable_namespace(&service_name, instance_id);
    let mut uri = url::Url::parse("mcpstore://gateway/")?;
    {
        let mut segments = uri
            .path_segments_mut()
            .map_err(|_| "gateway URI cannot contain path segments")?;
        segments.push(&namespace);
        if template {
            segments.push("template");
        }
        segments.push(&original);
    }
    Ok(uri.into())
}

/// Effective service overrides indexed by instance, shared by every catalog
/// projection/call path so names round-trip. Corrupt stored rules are warned
/// and skipped — one bad row must not take down the whole catalog.
pub(super) async fn service_override_map(
    store: &MCPStore,
) -> Result<HashMap<InstanceId, mcpstore::ServiceOverrideRule>, mcpstore::Error> {
    Ok(store
        .list_service_overrides_lenient()
        .await?        .into_iter()
        .map(|rule| (rule.instance_id, rule))
        .collect())
}

/// Project the raw catalog payloads through per-instance service overrides:
/// disabled services drop out entirely, overridden services expose their
/// display name in place of the real one (tool prefixes, namespaces, and
/// reverse resolution all see the projected name). Call identically on the
/// projection and resolution paths so names round-trip.
///
/// The real service name is preserved under `real_service_name` for
/// observability surfaces (call log) that must stay joinable with the
/// services page.
pub(super) fn apply_service_overrides(
    payloads: Vec<Value>,
    overrides: &HashMap<InstanceId, mcpstore::ServiceOverrideRule>,
) -> Result<Vec<Value>, ErrorData> {
    payloads
        .into_iter()
        .map(|mut payload| {
            let instance_id = read_required_instance_id(&payload, "instance_id")
                .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
            match overrides.get(&instance_id) {
                Some(rule) if !rule.common.enabled.unwrap_or(true) => Ok(None),
                Some(rule) if rule.common.display_name.is_some() => {
                    if let Some(object) = payload.as_object_mut() {
                        object.insert(
                            "service_name".to_string(),
                            Value::String(
                                rule.common.display_name.clone().expect("checked above"),
                            ),
                        );
                        object.insert(
                            "real_service_name".to_string(),
                            Value::String(rule.service_name.clone()),
                        );
                    }
                    Ok(Some(payload))
                }
                _ => Ok(Some(payload)),
            }
        })
        .collect::<Result<Vec<Option<Value>>, ErrorData>>()
        .map(|payloads| payloads.into_iter().flatten().collect())
}

/// Effective tool overrides indexed by (instance, raw tool name). Lenient on
/// corrupt rows for the same reason as the service map.
pub(super) async fn tool_override_map(
    store: &MCPStore,
) -> Result<HashMap<(InstanceId, String), mcpstore::ToolOverrideRule>, mcpstore::Error> {
    Ok(store
        .list_tool_overrides_lenient()
        .await?
        .into_iter()
        .map(|rule| ((rule.instance_id.clone(), rule.tool_name.clone()), rule))
        .collect())
}

/// Project tool payloads through enabled tool overrides: display_name replaces
/// the exposed name (the canonical raw name stays in `tool_name` for upstream
/// calls), description/icons ride along. A disabled override is inactive —
/// pass-through, matching the daemon listing semantics. Call on the same
/// payload slice as the service projection so names round-trip.
pub(super) fn apply_tool_overrides(
    payloads: &mut [Value],
    overrides: &HashMap<(InstanceId, String), mcpstore::ToolOverrideRule>,
) {
    for payload in payloads.iter_mut() {
        let Some(instance_id) = read_required_instance_id(payload, "instance_id").ok() else {
            continue;
        };
        let Some(name) = payload.get("name").and_then(Value::as_str).map(str::to_string)
        else {
            continue;
        };
        let Some(rule) = overrides.get(&(instance_id, name)) else {
            continue;
        };
        if !rule.common.enabled.unwrap_or(true) {
            continue;
        }
        let Some(object) = payload.as_object_mut() else {
            continue;
        };
        if let Some(display) = rule
            .common
            .display_name
            .as_ref()
            .filter(|name| !name.trim().is_empty())
        {
            object.insert("name".to_string(), Value::String(display.clone()));
        }
        if let Some(description) = rule.common.description.as_ref() {
            object.insert(
                "description".to_string(),
                Value::String(description.clone()),
            );
        }
        if let Some(icons) = rule.common.icons.as_ref() {
            object.insert("icons".to_string(), icons.clone());
        }
    }
}

pub(super) fn service_namespace(service_name: &str) -> String {
    service_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect()
}

/// Truncate a tool name to at most `max_len` bytes without splitting a UTF-8
/// character, so multi-byte names never come out mangled.
pub(super) fn truncate_tool_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        return name.to_string();
    }
    let mut end = max_len;
    while end > 0 && !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_string()
}

pub(super) fn stable_namespace(service_name: &str, instance_id: InstanceId) -> String {
    format!("{}__{instance_id}", service_namespace(service_name))
}

/// Cap tool names at this many bytes, matching magpie's hub projection.
pub(super) const MAX_TOOL_NAME: usize = 64;

/// Resolve the exposed tool name: `<service>__<tool>`, 64-byte rune-safe
/// truncated, with `_2`/`_3`, … appended on collision (budget recomputed so
/// the suffixed name still fits in 64 bytes) instead of hard-failing the
/// gateway build.
pub(super) fn exposed_tool_name(
    service_name: &str,
    original_name: &str,
    taken: &mut HashMap<String, usize>,
) -> String {
    let base = format!("{}__{original_name}", service_namespace(service_name));
    let base_truncated = truncate_tool_name(&base, MAX_TOOL_NAME);
    let count = taken.entry(base_truncated.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base_truncated
    } else {
        let suffix = format!("_{count}");
        let budget = MAX_TOOL_NAME.saturating_sub(suffix.len());
        format!("{}{suffix}", truncate_tool_name(&base, budget))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpstore::ComponentOverrideCommon;

    fn payload() -> Value {
        serde_json::json!({
            "name": "write_file",
            "tool_name": "write_file",
            "instance_id": "11111111-1111-1111-1111-111111111111",
            "service_name": "filesystem",
            "input_schema": {"type": "object"},
        })
    }

    fn rule(instance: &str, tool: &str, common: ComponentOverrideCommon) -> mcpstore::ToolOverrideRule {
        mcpstore::ToolOverrideRule {
            instance_id: instance.parse().unwrap(),
            service_name: "filesystem".into(),
            scope: ScopeRef::Store,
            tool_name: tool.into(),
            common,
            arguments: Vec::new(),
            safety_policy: None,
            updated_at: 0,
            version: 1,
        }
    }

    #[test]
    fn enabled_display_name_replaces_exposed_name() {
        let mut payloads = vec![payload()];
        let mut map = HashMap::new();
        map.insert(
            ("11111111-1111-1111-1111-111111111111".parse().unwrap(), "write_file".to_string()),
            rule("11111111-1111-1111-1111-111111111111", "write_file", {
                let mut common = ComponentOverrideCommon::default();
                common.display_name = Some("safe_write".into());
                common.description = Some("renamed".into());
                common
            }),
        );
        apply_tool_overrides(&mut payloads, &map);
        assert_eq!(payloads[0]["name"], "safe_write");
        assert_eq!(payloads[0]["tool_name"], "write_file");
        assert_eq!(payloads[0]["description"], "renamed");
    }

    #[test]
    fn disabled_override_is_pass_through() {
        let mut payloads = vec![payload()];
        let mut map = HashMap::new();
        map.insert(
            ("11111111-1111-1111-1111-111111111111".parse().unwrap(), "write_file".to_string()),
            rule("11111111-1111-1111-1111-111111111111", "write_file", {
                let mut common = ComponentOverrideCommon::default();
                common.display_name = Some("safe_write".into());
                common.enabled = Some(false);
                common
            }),
        );
        apply_tool_overrides(&mut payloads, &map);
        assert_eq!(payloads[0]["name"], "write_file");
    }
}
