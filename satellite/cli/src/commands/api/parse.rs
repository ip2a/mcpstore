use mcpstore::{ScopeRef, ScopeView, ServiceListProjection};
use serde::Deserialize;
use serde_json::{json, Value};

use super::envelope::{ApiError, ApiResult};

pub(super) fn extract_tool_name(payload: &Value) -> ApiResult<String> {
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .or_else(|| payload.get("tool").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::missing_parameter("tool_name"))?;
    Ok(tool_name.to_string())
}

pub(super) fn extract_tool_args(payload: &Value) -> ApiResult<Value> {
    match payload.get("args") {
        None | Some(Value::Null) => Ok(json!({})),
        Some(Value::Object(_)) => Ok(payload.get("args").cloned().unwrap_or_else(|| json!({}))),
        Some(_) => Err(ApiError::invalid_parameter(
            "args must be a JSON object",
            Some("args"),
        )),
    }
}

pub(super) fn extract_prompt_name(payload: &Value) -> ApiResult<String> {
    payload
        .get("prompt_name")
        .and_then(Value::as_str)
        .or_else(|| payload.get("prompt").and_then(Value::as_str))
        .or_else(|| payload.get("name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::missing_parameter("prompt_name"))
}

pub(super) fn extract_prompt_args(payload: &Value) -> ApiResult<Value> {
    match payload.get("args") {
        None | Some(Value::Null) => Ok(json!({})),
        Some(Value::Object(_)) => Ok(payload.get("args").cloned().unwrap_or_else(|| json!({}))),
        Some(_) => Err(ApiError::invalid_parameter(
            "args must be a JSON object",
            Some("args"),
        )),
    }
}

/// Scope identifier in query params: `?scope=store|agent&agent_id=...`; `scope` defaults to `store`.
#[derive(Deserialize)]
pub(super) struct ScopeQuery {
    pub(super) scope: Option<String>,
    pub(super) agent_id: Option<String>,
}

impl ScopeQuery {
    pub(super) fn into_scope_ref(self) -> ApiResult<ScopeRef> {
        parse_scope_ref(self.scope.as_deref(), self.agent_id.as_deref())
    }

    /// Read-view scope: root | store | agent (root = aggregate, read/list only).
    pub(super) fn into_scope_view(self) -> ApiResult<ScopeView> {
        parse_scope_view(self.scope.as_deref(), self.agent_id.as_deref())
    }
}

/// Payload-width params for `GET /services/list`: `?fields=summary|full&include=tools,config`.
/// Absent params keep the legacy full payload; `fields=summary` strips the heavy
/// fields (`tools`, `effective_config`) and `include` adds individual ones back.
#[derive(Deserialize)]
pub(super) struct ServiceListQuery {
    fields: Option<String>,
    include: Option<String>,
}

impl ServiceListQuery {
    pub(super) fn into_projection(self) -> ApiResult<ServiceListProjection> {
        let mut projection = ServiceListProjection::FULL;
        match self
            .fields
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            None | Some("full") => {}
            Some("summary") => projection.summary = true,
            Some(other) => {
                return Err(ApiError::invalid_parameter(
                    format!("Unsupported fields value: {other} (expected 'full' or 'summary')"),
                    Some("fields"),
                ))
            }
        }
        if let Some(include) = self
            .include
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            for part in include.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                match part {
                    "tools" => projection.include_tools = true,
                    "config" => projection.include_config = true,
                    other => {
                        return Err(ApiError::invalid_parameter(
                            format!(
                                "Unsupported include value: {other} (expected 'tools' or 'config')"
                            ),
                            Some("include"),
                        ))
                    }
                }
            }
        }
        Ok(projection)
    }
}

pub(super) fn parse_scope_ref(scope: Option<&str>, agent_id: Option<&str>) -> ApiResult<ScopeRef> {
    match scope.unwrap_or("store") {
        "store" => Ok(ScopeRef::Store),
        "agent" => Ok(ScopeRef::Agent {
            agent_id: agent_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ApiError::missing_parameter("agent_id"))?
                .to_string(),
        }),
        other => Err(ApiError::invalid_parameter(
            format!("Unsupported scope: {other}"),
            Some("scope"),
        )),
    }
}

/// Read-view scope parsing: `root` aggregate / `store` / `agent` (agent requires `agent_id`).
pub(super) fn parse_scope_view(
    scope: Option<&str>,
    agent_id: Option<&str>,
) -> ApiResult<ScopeView> {
    match scope.unwrap_or("store") {
        "all" => Ok(ScopeView::All),
        "store" => Ok(ScopeView::Store),
        "agent" => Ok(ScopeView::Agent {
            agent_id: agent_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ApiError::missing_parameter("agent_id"))?
                .to_string(),
        }),
        other => Err(ApiError::invalid_parameter(
            format!("Unsupported scope: {other}"),
            Some("scope"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn extract_tool_args_requires_object() {
        let error = extract_tool_args(&json!({ "args": [] })).unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.code, "invalid_input");
    }
}
