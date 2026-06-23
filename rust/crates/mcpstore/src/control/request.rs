use crate::config::ServerConfig;
use crate::{Result, StoreError};

pub(in crate::control) fn dedup_key(request_type: &str, payload: &serde_json::Value) -> String {
    let service_name = payload
        .get("service_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let agent_id = payload
        .get("agent_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    format!("{request_type}:{agent_id}:{service_name}")
}

pub(in crate::control) fn required_string(
    payload: &serde_json::Value,
    field: &str,
) -> Result<String> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| StoreError::Other(format!("Control request missing {field}")))
}

pub(in crate::control) fn optional_string(
    payload: &serde_json::Value,
    field: &str,
) -> Option<String> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

pub(in crate::control) fn required_config(payload: &serde_json::Value) -> Result<ServerConfig> {
    let config = payload
        .get("config")
        .cloned()
        .ok_or_else(|| StoreError::Other("Control request missing config".to_string()))?;
    serde_json::from_value(config).map_err(|error| {
        StoreError::Other(format!(
            "Control request config deserialization failed: {error}"
        ))
    })
}
