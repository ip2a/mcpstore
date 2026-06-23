/// Generic event wrapper.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub event_type: String,
    pub event_id: String,
    pub timestamp: i64,
    pub priority: i32,
    pub payload: serde_json::Value,
}

impl Event {
    pub fn new(event_type: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            event_type: event_type.into(),
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
            priority: 0,
            payload,
        }
    }
}
