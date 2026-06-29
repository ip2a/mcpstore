use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{Result, StoreError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OpenApiComponentType {
    Tool,
    Resource,
    ResourceTemplate,
}

impl OpenApiComponentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Resource => "resource",
            Self::ResourceTemplate => "resource_template",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenApiEndpoint {
    pub path: String,
    pub method: String,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<Value>,
    pub request_body: Option<Value>,
    #[serde(default)]
    pub responses: Map<String, Value>,
    #[serde(default)]
    pub security: Vec<Value>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub security_defined: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenApiComponent {
    pub name: String,
    #[serde(rename = "type")]
    pub component_type: OpenApiComponentType,
    pub endpoint: OpenApiEndpoint,
    #[serde(default)]
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub service_name: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenApiSpecInfo {
    pub title: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenApiComponentCounts {
    pub tools: usize,
    pub resources: usize,
    pub resource_templates: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenApiImportResult {
    pub service_name: String,
    pub spec_url: String,
    pub base_url: String,
    pub spec_info: OpenApiSpecInfo,
    #[serde(default)]
    pub security_schemes: Map<String, Value>,
    #[serde(default)]
    pub security: Vec<Value>,
    pub components: Vec<OpenApiComponent>,
    pub total_endpoints: usize,
    pub component_types: OpenApiComponentCounts,
    pub runtime_executable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OpenApiImportOptions {
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub auth: Map<String, Value>,
}

pub fn analyze_openapi_spec(
    name: &str,
    spec_url: &str,
    spec: Value,
) -> Result<OpenApiImportResult> {
    let endpoints = analyze_endpoints(&spec)?;
    let mut components = Vec::with_capacity(endpoints.len());
    for endpoint in endpoints {
        let component_type = suggest_component_type(&endpoint);
        let component_name = generate_component_name(&endpoint);
        let description = endpoint
            .description
            .clone()
            .or_else(|| endpoint.summary.clone());
        let input_schema = input_schema_for_endpoint(&endpoint);
        components.push(OpenApiComponent {
            name: component_name,
            component_type,
            tags: endpoint.tags.clone(),
            description,
            service_name: name.to_string(),
            endpoint,
            input_schema,
        });
    }

    let tools = components
        .iter()
        .filter(|component| component.component_type == OpenApiComponentType::Tool)
        .count();
    let resources = components
        .iter()
        .filter(|component| component.component_type == OpenApiComponentType::Resource)
        .count();
    let resource_templates = components
        .iter()
        .filter(|component| component.component_type == OpenApiComponentType::ResourceTemplate)
        .count();

    for component in &mut components {
        component.service_name = name.to_string();
    }

    Ok(OpenApiImportResult {
        service_name: name.to_string(),
        spec_url: spec_url.to_string(),
        base_url: extract_base_url(&spec),
        spec_info: spec_info(&spec),
        security_schemes: extract_security_schemes(&spec),
        security: extract_security(&spec),
        total_endpoints: components.len(),
        component_types: OpenApiComponentCounts {
            tools,
            resources,
            resource_templates,
        },
        components,
        runtime_executable: false,
    })
}

fn analyze_endpoints(spec: &Value) -> Result<Vec<OpenApiEndpoint>> {
    let paths = spec
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| StoreError::Other("OpenAPI spec missing object field: paths".to_string()))?;
    let mut endpoints = Vec::new();
    for (path, path_item) in paths {
        let Some(operations) = path_item.as_object() else {
            continue;
        };
        for (method, operation) in operations {
            let method_upper = method.to_ascii_uppercase();
            if !matches!(
                method_upper.as_str(),
                "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS"
            ) {
                continue;
            }
            let operation = operation.as_object().ok_or_else(|| {
                StoreError::Other(format!(
                    "OpenAPI operation must be an object: {method_upper} {path}"
                ))
            })?;
            let security_defined = operation.contains_key("security");
            endpoints.push(OpenApiEndpoint {
                path: path.clone(),
                method: method_upper,
                operation_id: operation
                    .get("operationId")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                summary: operation
                    .get("summary")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                description: operation
                    .get("description")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                tags: operation
                    .get("tags")
                    .and_then(Value::as_array)
                    .map(|tags| {
                        tags.iter()
                            .filter_map(Value::as_str)
                            .map(ToString::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
                parameters: operation
                    .get("parameters")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
                request_body: operation.get("requestBody").cloned(),
                responses: operation
                    .get("responses")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default(),
                security: operation
                    .get("security")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
                security_defined,
            });
        }
    }
    Ok(endpoints)
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn suggest_component_type(endpoint: &OpenApiEndpoint) -> OpenApiComponentType {
    if endpoint.method == "GET" {
        if endpoint.path.contains('{') && endpoint.path.contains('}') {
            OpenApiComponentType::ResourceTemplate
        } else {
            OpenApiComponentType::Resource
        }
    } else {
        OpenApiComponentType::Tool
    }
}

fn generate_component_name(endpoint: &OpenApiEndpoint) -> String {
    let raw = endpoint.operation_id.clone().unwrap_or_else(|| {
        format!(
            "{}_{}",
            endpoint.method.to_ascii_lowercase(),
            endpoint.path.trim_matches('/').replace('/', "_")
        )
    });
    sanitize_component_name(&raw)
}

fn sanitize_component_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_was_underscore = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_underscore = false;
        } else if ch == '_' {
            if !last_was_underscore {
                out.push('_');
            }
            last_was_underscore = true;
        } else if !last_was_underscore {
            out.push('_');
            last_was_underscore = true;
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "operation".to_string()
    } else {
        trimmed
    }
}

fn input_schema_for_endpoint(endpoint: &OpenApiEndpoint) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for parameter in &endpoint.parameters {
        let Some(parameter) = parameter.as_object() else {
            continue;
        };
        let Some(name) = parameter.get("name").and_then(Value::as_str) else {
            continue;
        };
        let mut schema = parameter
            .get("schema")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({ "type": "string" }));
        if let Some(description) = parameter.get("description").and_then(Value::as_str) {
            if let Value::Object(map) = &mut schema {
                map.entry("description".to_string())
                    .or_insert_with(|| Value::String(description.to_string()));
            }
        }
        if let Value::Object(map) = &mut schema {
            if let Some(location) = parameter.get("in").and_then(Value::as_str) {
                map.insert(
                    "x_mcpstore_parameter_in".to_string(),
                    Value::String(location.to_string()),
                );
            }
        }
        properties.insert(name.to_string(), schema);
        if parameter
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            required.push(Value::String(name.to_string()));
        }
    }

    if let Some(request_body) = &endpoint.request_body {
        properties.insert("body".to_string(), request_body_schema(request_body));
        if request_body
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            required.push(Value::String("body".to_string()));
        }
    }

    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

fn request_body_schema(request_body: &Value) -> Value {
    request_body
        .get("content")
        .and_then(Value::as_object)
        .and_then(|content| {
            content
                .get("application/json")
                .or_else(|| content.values().next())
        })
        .and_then(|media| media.get("schema"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({ "type": "object" }))
}

fn spec_info(spec: &Value) -> OpenApiSpecInfo {
    let info = spec.get("info").and_then(Value::as_object);
    OpenApiSpecInfo {
        title: info
            .and_then(|info| info.get("title"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        version: info
            .and_then(|info| info.get("version"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        description: info
            .and_then(|info| info.get("description"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
}

fn extract_base_url(spec: &Value) -> String {
    spec.get("servers")
        .and_then(Value::as_array)
        .and_then(|servers| servers.first())
        .and_then(|server| server.get("url"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn extract_security_schemes(spec: &Value) -> Map<String, Value> {
    spec.get("components")
        .and_then(|components| components.get("securitySchemes"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn extract_security(spec: &Value) -> Vec<Value> {
    spec.get("security")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

#[allow(dead_code)]
fn _component_type_as_str(component_type: &OpenApiComponentType) -> &'static str {
    component_type.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_openapi_components_like_old_python_defaults() {
        let spec = serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "Demo", "version": "1.0" },
            "servers": [{ "url": "https://api.example.test" }],
            "paths": {
                "/pets": {
                    "get": { "operationId": "listPets", "summary": "List pets", "tags": ["pets"] },
                    "post": {
                        "operationId": "create-pet",
                        "requestBody": {
                            "required": true,
                            "content": { "application/json": { "schema": { "type": "object" } } }
                        }
                    }
                },
                "/pets/{id}": {
                    "get": {
                        "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }]
                    }
                }
            }
        });

        let result = analyze_openapi_spec("petstore", "memory://spec", spec).unwrap();

        assert_eq!(result.service_name, "petstore");
        assert_eq!(result.base_url, "https://api.example.test");
        assert_eq!(result.total_endpoints, 3);
        assert_eq!(result.component_types.tools, 1);
        assert_eq!(result.component_types.resources, 1);
        assert_eq!(result.component_types.resource_templates, 1);
        assert!(!result.runtime_executable);
        assert!(result
            .components
            .iter()
            .any(|component| component.name == "listPets"));
        assert!(result
            .components
            .iter()
            .any(|component| component.name == "create_pet"));
        let templated = result
            .components
            .iter()
            .find(|component| component.component_type == OpenApiComponentType::ResourceTemplate)
            .unwrap();
        assert_eq!(
            templated.input_schema["required"],
            serde_json::json!(["id"])
        );
    }
}
