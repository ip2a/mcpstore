use crate::store::prelude::*;

impl MCPStore {
    pub(crate) fn resource_payload_value(
        resource: DiscoveredResource,
        service_name: String,
        global_service_name: String,
    ) -> Result<serde_json::Value> {
        let original_uri = resource.uri.clone();
        let mut resource = serde_json::to_value(resource).unwrap_or(serde_json::Value::Null);
        if let serde_json::Value::Object(object) = &mut resource {
            object.insert("original_uri".to_string(), serde_json::json!(original_uri));
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
        Ok(resource)
    }

    pub(crate) fn resource_template_payload_value(
        template: DiscoveredResourceTemplate,
        service_name: String,
        global_service_name: String,
    ) -> Result<serde_json::Value> {
        let original_uri_template = template.uri_template.clone();
        let mut template = serde_json::to_value(template).unwrap_or(serde_json::Value::Null);
        if let serde_json::Value::Object(object) = &mut template {
            object.insert(
                "original_uri_template".to_string(),
                serde_json::json!(original_uri_template),
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
        Ok(template)
    }
}
