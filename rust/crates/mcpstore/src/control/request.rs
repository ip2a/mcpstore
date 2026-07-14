use crate::config::ServerConfig;
use crate::{Result, StoreError};

pub(in crate::control) fn dedup_key(request_type: &str, payload: &serde_json::Value) -> String {
    let identity = payload
        .get("instance_id")
        .or_else(|| payload.get("scope"))
        .or_else(|| payload.get("service_name"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    format!("{request_type}:{identity}")
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
