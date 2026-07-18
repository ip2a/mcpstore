use serde::{Deserialize, Serialize};

use crate::config::ServerConfig;
use crate::{Result, StoreError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(in crate::control) enum ControlRequestStatus {
    Queued,
    Executing { started_at: i64 },
    Applied { applied_at: i64 },
    RetryScheduled { retry_at: i64, reason: String },
    Rejected { rejected_at: i64, reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::control) struct ControlRequest {
    pub id: String,
    #[serde(rename = "type")]
    pub request_type: String,
    pub payload: serde_json::Value,
    pub source: String,
    pub created_at: i64,
    pub dedup_key: String,
    pub trace_id: String,
    #[serde(flatten)]
    pub status: ControlRequestStatus,
}

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

#[cfg(test)]
mod tests {
    use super::ControlRequestStatus;

    #[test]
    fn control_request_status_round_trips() {
        let statuses = [
            ControlRequestStatus::Queued,
            ControlRequestStatus::Executing { started_at: 10 },
            ControlRequestStatus::Applied { applied_at: 20 },
            ControlRequestStatus::RetryScheduled {
                retry_at: 30,
                reason: "store unavailable".to_string(),
            },
            ControlRequestStatus::Rejected {
                rejected_at: 40,
                reason: "invalid request".to_string(),
            },
        ];

        for status in statuses {
            let value = serde_json::to_value(&status).expect("serialize control request status");
            let decoded: ControlRequestStatus =
                serde_json::from_value(value).expect("deserialize control request status");
            assert_eq!(decoded, status);
        }
    }
}
