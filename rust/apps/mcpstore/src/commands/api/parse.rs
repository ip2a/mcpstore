use mcpstore::{ScopeRef, ScopeView};
use serde::Deserialize;
use serde_json::{json, Value};

use super::envelope::{ApiError, ApiResult};

pub(super) fn normalize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return String::new();
    }

    let mut normalized = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    while normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

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
            "args 必须是 JSON 对象",
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
            "args 必须是 JSON 对象",
            Some("args"),
        )),
    }
}

/// Query 参数里的作用域标识：`?scope=store|agent&agent_id=...`，`scope` 缺省为 `store`。
#[derive(Deserialize)]
pub(super) struct ScopeQuery {
    pub(super) scope: Option<String>,
    pub(super) agent_id: Option<String>,
}

impl ScopeQuery {
    pub(super) fn into_scope_ref(self) -> ApiResult<ScopeRef> {
        parse_scope_ref(self.scope.as_deref(), self.agent_id.as_deref())
    }

    /// 读视图作用域：root | store | agent（root = 聚合，仅用于读 / 列表）。
    pub(super) fn into_scope_view(self) -> ApiResult<ScopeView> {
        parse_scope_view(self.scope.as_deref(), self.agent_id.as_deref())
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
            format!("不支持的 scope: {other}"),
            Some("scope"),
        )),
    }
}

/// 读视图作用域解析：`root` 聚合 / `store` / `agent`（agent 需带 `agent_id`）。
pub(super) fn parse_scope_view(
    scope: Option<&str>,
    agent_id: Option<&str>,
) -> ApiResult<ScopeView> {
    match scope.unwrap_or("store") {
        "root" => Ok(ScopeView::Root),
        "store" => Ok(ScopeView::Store),
        "agent" => Ok(ScopeView::Agent {
            agent_id: agent_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ApiError::missing_parameter("agent_id"))?
                .to_string(),
        }),
        other => Err(ApiError::invalid_parameter(
            format!("不支持的 scope: {other}"),
            Some("scope"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn normalize_prefix_trims_empty_and_trailing_slash() {
        assert_eq!(normalize_prefix(""), "");
        assert_eq!(normalize_prefix("/"), "");
        assert_eq!(normalize_prefix("mcp"), "/mcp");
        assert_eq!(normalize_prefix("/mcp/"), "/mcp");
    }

    #[test]
    fn extract_tool_args_requires_object() {
        let error = extract_tool_args(&json!({ "args": [] })).unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.code, "invalid_input");
    }
}
