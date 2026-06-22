use crate::store::prelude::*;

pub(crate) fn wrap_cache_item(
    key: &str,
    type_name: &str,
    collection: &str,
    value: serde_json::Value,
) -> serde_json::Value {
    let mut object = match value {
        serde_json::Value::Object(object) => object,
        other => {
            let mut object = serde_json::Map::new();
            object.insert("value".to_string(), other);
            object
        }
    };
    object.insert("_key".to_string(), serde_json::json!(key));
    object.insert("_type".to_string(), serde_json::json!(type_name));
    object.insert("_collection".to_string(), serde_json::json!(collection));
    serde_json::Value::Object(object)
}

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

    pub(crate) fn tool_payload_value(
        displayed_name: String,
        original_name: String,
        service_name: String,
        global_service_name: String,
        description: String,
        schema: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let global_tool_name = generate_tool_global_name(&global_service_name, &original_name)?;
        let client_id = global_service_name.clone();
        Ok(serde_json::json!({
            "name": displayed_name,
            "original_name": original_name,
            "description": description,
            "schema": schema,
            "input_schema": schema,
            "service_name": service_name,
            "global_service_name": global_service_name,
            "service_global_name": global_service_name,
            "global_tool_name": global_tool_name,
            "client_id": client_id,
        }))
    }

    pub(crate) fn scoped_tool_entry(
        displayed_name: String,
        original_name: String,
        service_name: String,
        global_service_name: String,
        description: String,
        schema: serde_json::Value,
    ) -> Result<ScopedToolEntry> {
        let global_tool_name = generate_tool_global_name(&global_service_name, &original_name)?;
        let client_id = global_service_name.clone();
        Ok(ScopedToolEntry {
            name: displayed_name,
            original_name,
            description,
            schema: schema.clone(),
            input_schema: schema,
            service_name,
            service_global_name: global_service_name.clone(),
            global_service_name,
            global_tool_name,
            client_id,
        })
    }

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

    pub(crate) fn prompt_payload_value(
        mut prompt: serde_json::Value,
        displayed_name: Option<String>,
        service_name: String,
        global_service_name: String,
    ) -> Result<serde_json::Value> {
        let original_name = Self::required_value_field(&prompt, "name")?;
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
