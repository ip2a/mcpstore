use crate::store::prelude::*;

impl MCPStore {
    pub(crate) fn scoped_service_entry(
        mut service: ServiceEntry,
        localize_for_agent: bool,
    ) -> ScopedServiceEntry {
        service
            .tools
            .sort_by(|left, right| left.name.cmp(&right.name));
        let tool_count = service.tools.len();
        let client_id = service.name.clone();
        let global_name = if localize_for_agent {
            let global_name = service.name.clone();
            service.name = service.original_name.clone();
            Some(global_name)
        } else {
            None
        };
        ScopedServiceEntry {
            service,
            tool_count,
            global_name,
            client_id,
        }
    }

    pub(crate) fn service_payload_value(
        mut service: ServiceEntry,
        localize_for_agent: bool,
    ) -> serde_json::Value {
        service
            .tools
            .sort_by(|left, right| left.name.cmp(&right.name));
        let tool_count = service.tools.len();
        let global_name = service.name.clone();
        let localized_name = service.original_name.clone();
        let mut value = serde_json::to_value(service)
            .unwrap_or_else(|_| serde_json::Value::Object(Default::default()));
        if let serde_json::Value::Object(object) = &mut value {
            object.insert("client_id".to_string(), serde_json::json!(global_name));
            object.insert("tool_count".to_string(), serde_json::json!(tool_count));
            if localize_for_agent {
                object.insert("global_name".to_string(), serde_json::json!(global_name));
                object.insert("name".to_string(), serde_json::json!(localized_name));
            }
        }
        value
    }
}
