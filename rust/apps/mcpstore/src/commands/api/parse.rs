use std::collections::HashMap;

use mcpstore::CacheStorage;
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

pub(super) fn cache_storage_label(cache_storage: CacheStorage) -> &'static str {
    cache_storage.as_str()
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

pub(super) fn extract_resource_uri(params: &HashMap<String, String>) -> ApiResult<String> {
    params
        .get("uri")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::missing_parameter("uri"))
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

pub(super) fn parse_positive_u64(value: &str) -> ApiResult<u64> {
    value
        .parse::<u64>()
        .map_err(|_| ApiError::invalid_parameter(format!("无效的正整数: {value}"), Some("timeout")))
}

pub(super) fn parse_positive_usize(value: &str) -> ApiResult<usize> {
    value
        .parse::<usize>()
        .map_err(|_| ApiError::invalid_parameter(format!("无效的正整数: {value}"), Some("count")))
}

pub(super) fn parse_cache_storage(value: &str) -> ApiResult<CacheStorage> {
    match value {
        "memory" => Ok(CacheStorage::Memory),
        "redis" => Ok(CacheStorage::Redis),
        "openkeyv_memory" => Ok(CacheStorage::OpenKeyvMemory),
        "openkeyv_redis" => Ok(CacheStorage::OpenKeyvRedis),
        other => Err(ApiError::invalid_parameter(
            format!("不支持的 backend: {other}"),
            Some("backend"),
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
        assert_eq!(error.code, "INVALID_PARAMETER");
    }

    #[test]
    fn parse_cache_storage_supports_known_values() {
        assert!(matches!(
            parse_cache_storage("memory").unwrap(),
            CacheStorage::Memory
        ));
        assert!(matches!(
            parse_cache_storage("openkeyv_redis").unwrap(),
            CacheStorage::OpenKeyvRedis
        ));
        assert!(parse_cache_storage("unknown").is_err());
    }

    #[test]
    fn parse_positive_numbers_require_valid_integers() {
        assert_eq!(parse_positive_u64("10").unwrap(), 10);
        assert_eq!(parse_positive_usize("7").unwrap(), 7);
        assert!(parse_positive_u64("oops").is_err());
        assert!(parse_positive_usize("oops").is_err());
    }
}
