use crate::store::prelude::*;

impl MCPStore {
    pub(crate) fn prompt_payload_value(
        prompt: DiscoveredPrompt,
        displayed_name: Option<String>,
        service_name: String,
        global_service_name: String,
    ) -> Result<serde_json::Value> {
        let original_name = prompt.name.clone();
        let mut prompt = serde_json::to_value(prompt).unwrap_or(serde_json::Value::Null);
        if let serde_json::Value::Object(object) = &mut prompt {
            if let Some(displayed_name) = displayed_name {
                object.insert("name".to_string(), serde_json::json!(displayed_name));
            }
            object.insert(
                "original_name".to_string(),
                serde_json::json!(original_name),
            );
            object.insert("service_name".to_string(), serde_json::json!(service_name));
            object.insert(
                "global_service_name".to_string(),
                serde_json::json!(global_service_name),
            );
            object.insert(
                "service_global_name".to_string(),
                serde_json::json!(global_service_name),
            );
        }
        Ok(prompt)
    }
}
