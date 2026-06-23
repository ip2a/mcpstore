use crate::store::prelude::*;

impl MCPStore {
    pub(crate) fn value_field<'a>(value: &'a serde_json::Value, field: &str) -> &'a str {
        value
            .get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    }

    pub(crate) fn required_value_field(value: &serde_json::Value, field: &str) -> Result<String> {
        value
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| StoreError::Other(format!("响应缺少字符串字段: {field}")))
    }
}
