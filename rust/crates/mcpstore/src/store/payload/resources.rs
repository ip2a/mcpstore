use crate::store::prelude::*;

impl MCPStore {
    pub(crate) fn resource_payload_value(
        mut resource: serde_json::Value,
        service_name: String,
        global_service_name: String,
    ) -> Result<serde_json::Value> {
        let original_uri = Self::required_value_field(&resource, "uri")?;
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
        mut template: serde_json::Value,
        service_name: String,
        global_service_name: String,
    ) -> Result<serde_json::Value> {
        let original_uri_template = Self::required_value_field(&template, "uriTemplate")?;
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
