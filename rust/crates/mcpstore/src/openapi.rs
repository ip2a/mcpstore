use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{Result, StoreError};

const MAX_LOCAL_REF_DEPTH: usize = 32;
pub const DEFAULT_OPENAPI_REF_CACHE_TTL_SECONDS: u32 = 300;

fn default_openapi_ref_cache_enabled() -> bool {
    true
}

fn default_openapi_ref_cache_ttl_seconds() -> u32 {
    DEFAULT_OPENAPI_REF_CACHE_TTL_SECONDS
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenApiBundleArtifact {
    pub spec_url: String,
    pub bundle: Value,
    #[serde(default)]
    pub documents: Vec<OpenApiBundleDocument>,
    #[serde(default)]
    pub dependencies: Vec<OpenApiBundleDependency>,
    #[serde(default)]
    pub diagnostics: Vec<OpenApiBundleDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiBundleDocument {
    pub url: String,
    pub role: String,
    pub content_hash: String,
    pub content_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiBundleDependency {
    pub source_document: String,
    pub source_ref: String,
    pub target_document: String,
    pub pointer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiBundleDiagnostic {
    pub level: String,
    pub message: String,
    pub document: Option<String>,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OpenApiBundleOptions {
    #[serde(default)]
    pub ref_cache: OpenApiRefCachePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiRefCachePolicy {
    #[serde(default = "default_openapi_ref_cache_enabled")]
    pub enabled: bool,
    #[serde(default = "default_openapi_ref_cache_ttl_seconds")]
    pub ttl_seconds: u32,
}

impl Default for OpenApiRefCachePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_seconds: DEFAULT_OPENAPI_REF_CACHE_TTL_SECONDS,
        }
    }
}

impl OpenApiRefCachePolicy {
    pub fn is_enabled(&self) -> bool {
        self.enabled && self.ttl_seconds > 0
    }

    pub fn ttl_seconds_i64(&self) -> i64 {
        i64::from(self.ttl_seconds)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenApiImportOptions {
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub auth: Map<String, Value>,
    #[serde(default)]
    pub ref_cache: OpenApiRefCachePolicy,
    #[serde(default = "default_openapi_runtime_timeout_millis")]
    pub timeout_millis: u64,
}

const DEFAULT_OPENAPI_RUNTIME_TIMEOUT_MILLIS: u64 = 30_000;

impl Default for OpenApiImportOptions {
    fn default() -> Self {
        Self {
            headers: std::collections::HashMap::new(),
            auth: Map::new(),
            ref_cache: OpenApiRefCachePolicy::default(),
            timeout_millis: DEFAULT_OPENAPI_RUNTIME_TIMEOUT_MILLIS,
        }
    }
}

impl OpenApiImportOptions {
    pub fn default_timeout_millis() -> u64 {
        DEFAULT_OPENAPI_RUNTIME_TIMEOUT_MILLIS
    }
}

fn default_openapi_runtime_timeout_millis() -> u64 {
    DEFAULT_OPENAPI_RUNTIME_TIMEOUT_MILLIS
}

pub fn parse_openapi_spec_text(spec_text: &str) -> Result<Value> {
    serde_json::from_str::<Value>(spec_text)
        .or_else(|json_err| {
            serde_yaml_ng::from_str::<Value>(spec_text).map_err(|yaml_err| {
                StoreError::Other(format!(
                    "OpenAPI spec must be valid JSON or YAML: JSON error: {json_err}; YAML error: {yaml_err}"
                ))
            })
        })
        .and_then(|value| {
            if value.is_object() {
                Ok(value)
            } else {
                Err(StoreError::Other(
                    "OpenAPI spec must parse to a JSON/YAML object".to_string(),
                ))
            }
        })
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
        let path_parameters = resolve_parameter_list(spec, operations.get("parameters"))?;
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
            let operation_parameters = resolve_parameter_list(spec, operation.get("parameters"))?;
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
                parameters: merge_parameters(path_parameters.clone(), operation_parameters),
                request_body: resolve_optional_value(spec, operation.get("requestBody"))?,
                responses: operation
                    .get("responses")
                    .and_then(Value::as_object)
                    .map(|responses| resolve_map_values(spec, responses))
                    .transpose()?
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

fn resolve_parameter_list(spec: &Value, parameters: Option<&Value>) -> Result<Vec<Value>> {
    let Some(parameters) = parameters.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    parameters
        .iter()
        .map(|parameter| resolve_local_refs(spec, parameter, 0))
        .collect()
}

fn merge_parameters(path_parameters: Vec<Value>, operation_parameters: Vec<Value>) -> Vec<Value> {
    let mut parameters = path_parameters;
    for operation_parameter in operation_parameters {
        if let Some(operation_key) = parameter_key(&operation_parameter) {
            if let Some(existing) = parameters
                .iter()
                .position(|parameter| parameter_key(parameter).as_ref() == Some(&operation_key))
            {
                parameters[existing] = operation_parameter;
                continue;
            }
        }
        parameters.push(operation_parameter);
    }
    parameters
}

fn parameter_key(parameter: &Value) -> Option<(String, String)> {
    let parameter = parameter.as_object()?;
    Some((
        parameter.get("name")?.as_str()?.to_string(),
        parameter.get("in")?.as_str()?.to_string(),
    ))
}

fn resolve_optional_value(spec: &Value, value: Option<&Value>) -> Result<Option<Value>> {
    value
        .map(|value| resolve_local_refs(spec, value, 0))
        .transpose()
}

pub(crate) fn resolve_openapi_local_refs(spec: &Value, value: &Value) -> Result<Value> {
    resolve_local_refs(spec, value, 0)
}

fn resolve_map_values(spec: &Value, values: &Map<String, Value>) -> Result<Map<String, Value>> {
    values
        .iter()
        .map(|(key, value)| Ok((key.clone(), resolve_local_refs(spec, value, 0)?)))
        .collect()
}

fn resolve_local_refs(spec: &Value, value: &Value, depth: usize) -> Result<Value> {
    if depth > MAX_LOCAL_REF_DEPTH {
        return Err(StoreError::Other(
            "OpenAPI local $ref resolution exceeded maximum depth".to_string(),
        ));
    }

    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                let mut resolved = resolve_local_ref(spec, reference, depth + 1)?;
                if let Value::Object(resolved_map) = &mut resolved {
                    for (key, sibling) in map.iter().filter(|(key, _)| key.as_str() != "$ref") {
                        resolved_map
                            .insert(key.clone(), resolve_local_refs(spec, sibling, depth + 1)?);
                    }
                }
                return Ok(resolved);
            }

            map.iter()
                .map(|(key, value)| Ok((key.clone(), resolve_local_refs(spec, value, depth + 1)?)))
                .collect::<Result<Map<String, Value>>>()
                .map(Value::Object)
        }
        Value::Array(items) => items
            .iter()
            .map(|item| resolve_local_refs(spec, item, depth + 1))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        _ => Ok(value.clone()),
    }
}

fn resolve_local_ref(spec: &Value, reference: &str, depth: usize) -> Result<Value> {
    let Some(pointer) = reference.strip_prefix('#') else {
        return Err(StoreError::Other(format!(
            "Unsupported OpenAPI $ref outside current document: {reference}"
        )));
    };
    if !pointer.starts_with('/') {
        return Err(StoreError::Other(format!(
            "Invalid OpenAPI local $ref: {reference}"
        )));
    }
    let target = spec.pointer(pointer).ok_or_else(|| {
        StoreError::Other(format!("OpenAPI local $ref target not found: {reference}"))
    })?;
    resolve_local_refs(spec, target, depth)
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
    let schema = request_body
        .get("content")
        .and_then(Value::as_object)
        .and_then(|content| {
            content
                .get("application/json")
                .or_else(|| content.values().next())
        })
        .and_then(|media| media.get("schema"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({ "type": "object" }));
    expose_binary_file_arguments(request_input_schema(schema))
}

fn request_input_schema(schema: Value) -> Value {
    match schema {
        Value::Object(mut object) => {
            let mut removed_read_only = read_only_properties_in_all_of(&object);
            if let Some(Value::Object(properties)) = object.get_mut("properties") {
                let direct_read_only = properties
                    .iter()
                    .filter_map(|(name, schema)| schema_is_read_only(schema).then(|| name.clone()))
                    .collect::<Vec<_>>();
                for name in &direct_read_only {
                    properties.remove(name);
                }
                append_unique(&mut removed_read_only, direct_read_only);
                for value in properties.values_mut() {
                    let converted = request_input_schema(std::mem::take(value));
                    *value = converted;
                }
            }
            remove_required_fields(&mut object, &removed_read_only);
            for name in ["items", "additionalProperties"] {
                if let Some(value) = object.get_mut(name) {
                    if value.is_object() {
                        let converted = request_input_schema(std::mem::take(value));
                        *value = converted;
                    }
                }
            }
            for name in ["allOf", "anyOf", "oneOf"] {
                if let Some(Value::Array(items)) = object.get_mut(name) {
                    for item in items {
                        let converted = request_input_schema(std::mem::take(item));
                        *item = converted;
                        if name == "allOf" {
                            remove_required_fields_from_schema(item, &removed_read_only);
                        }
                    }
                }
            }
            Value::Object(object)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(request_input_schema).collect()),
        other => other,
    }
}

fn read_only_properties_in_all_of(object: &Map<String, Value>) -> Vec<String> {
    object
        .get("allOf")
        .and_then(Value::as_array)
        .map(|items| {
            let mut names = Vec::new();
            for item in items {
                append_unique(&mut names, read_only_property_names(item));
            }
            names
        })
        .unwrap_or_default()
}

fn read_only_property_names(schema: &Value) -> Vec<String> {
    let Some(object) = schema.as_object() else {
        return Vec::new();
    };
    let mut names = object
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter_map(|(name, schema)| schema_is_read_only(schema).then(|| name.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for name in read_only_properties_in_all_of(object) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

fn append_unique(target: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        if !target.contains(&value) {
            target.push(value);
        }
    }
}

fn remove_required_fields_from_schema(schema: &mut Value, names: &[String]) {
    if let Value::Object(object) = schema {
        remove_required_fields(object, names);
    }
}

fn schema_is_read_only(schema: &Value) -> bool {
    schema
        .get("readOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn remove_required_fields(object: &mut Map<String, Value>, names: &[String]) {
    if names.is_empty() {
        return;
    }
    if let Some(Value::Array(required)) = object.get_mut("required") {
        required.retain(|value| {
            value
                .as_str()
                .map(|name| !names.iter().any(|removed| removed == name))
                .unwrap_or(true)
        });
    }
}

fn expose_binary_file_arguments(schema: Value) -> Value {
    match schema {
        Value::Object(mut object) if is_binary_string_schema(&object) => {
            if let Some(description) = object.remove("description") {
                serde_json::json!({
                    "type": "object",
                    "description": description,
                    "x_mcpstore_file": true,
                    "properties": file_argument_properties(),
                    "oneOf": [
                        { "required": ["bytes"] },
                        { "required": ["path"] }
                    ]
                })
            } else {
                serde_json::json!({
                    "type": "object",
                    "x_mcpstore_file": true,
                    "properties": file_argument_properties(),
                    "oneOf": [
                        { "required": ["bytes"] },
                        { "required": ["path"] }
                    ]
                })
            }
        }
        Value::Object(mut object) => {
            if let Some(Value::Object(properties)) = object.get_mut("properties") {
                for value in properties.values_mut() {
                    let converted = expose_binary_file_arguments(std::mem::take(value));
                    *value = converted;
                }
            }
            if let Some(items) = object.get_mut("items") {
                let converted = expose_binary_file_arguments(std::mem::take(items));
                *items = converted;
            }
            Value::Object(object)
        }
        other => other,
    }
}

fn file_argument_properties() -> Value {
    serde_json::json!({
        "bytes": {
            "type": "string",
            "contentEncoding": "base64",
            "description": "Base64-encoded file bytes. Use this for cross-process calls."
        },
        "path": {
            "type": "string",
            "description": "Local filesystem path readable by the Rust runtime process."
        },
        "filename": { "type": "string" },
        "mimeType": { "type": "string" },
        "mime_type": { "type": "string" }
    })
}

fn is_binary_string_schema(schema: &Map<String, Value>) -> bool {
    schema.get("type").and_then(Value::as_str) == Some("string")
        && schema
            .get("format")
            .and_then(Value::as_str)
            .map(|format| format.eq_ignore_ascii_case("binary"))
            .unwrap_or(false)
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
                    },
                    "put": {
                        "operationId": "uploadPetPhoto",
                        "requestBody": {
                            "required": true,
                            "content": {
                                "multipart/form-data": {
                                    "schema": {
                                        "type": "object",
                                        "properties": { "file": { "type": "string", "format": "binary" } }
                                    }
                                }
                            }
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
        assert_eq!(result.total_endpoints, 4);
        assert_eq!(result.component_types.tools, 2);
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
        let upload = result
            .components
            .iter()
            .find(|component| component.name == "uploadPetPhoto")
            .unwrap();
        assert_eq!(
            upload.input_schema["properties"]["body"]["properties"]["file"]["x_mcpstore_file"],
            serde_json::json!(true)
        );
        assert!(
            upload.input_schema["properties"]["body"]["properties"]["file"]["properties"]
                .get("bytes")
                .is_some()
        );
        assert!(
            upload.input_schema["properties"]["body"]["properties"]["file"]["properties"]
                .get("path")
                .is_some()
        );
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

    #[test]
    fn analyzes_path_parameters_and_local_refs() {
        let spec = serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "Demo", "version": "1.0" },
            "servers": [{ "url": "https://api.example.test" }],
            "components": {
                "parameters": {
                    "TenantId": {
                        "name": "tenant_id",
                        "in": "path",
                        "required": true,
                        "schema": { "$ref": "#/components/schemas/TenantId" }
                    },
                    "Verbose": {
                        "name": "verbose",
                        "in": "query",
                        "schema": { "type": "boolean" }
                    }
                },
                "requestBodies": {
                    "CreateItem": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/NewItem" }
                            }
                        }
                    }
                },
                "schemas": {
                    "TenantId": { "type": "string", "description": "tenant" },
                    "NewItem": {
                        "type": "object",
                        "properties": { "name": { "type": "string" } },
                        "required": ["name"]
                    }
                }
            },
            "paths": {
                "/tenants/{tenant_id}/items": {
                    "parameters": [{ "$ref": "#/components/parameters/TenantId" }],
                    "get": {
                        "operationId": "listTenantItems",
                        "parameters": [{ "$ref": "#/components/parameters/Verbose" }]
                    },
                    "post": {
                        "operationId": "createTenantItem",
                        "parameters": [{ "name": "tenant_id", "in": "path", "required": true, "schema": { "type": "integer" } }],
                        "requestBody": { "$ref": "#/components/requestBodies/CreateItem" }
                    }
                }
            }
        });

        let result = analyze_openapi_spec("tenant", "memory://spec", spec).unwrap();
        let list = result
            .components
            .iter()
            .find(|component| component.name == "listTenantItems")
            .unwrap();
        assert_eq!(
            list.input_schema["properties"]["tenant_id"],
            serde_json::json!({
                "type": "string",
                "description": "tenant",
                "x_mcpstore_parameter_in": "path"
            })
        );
        assert_eq!(
            list.input_schema["properties"]["verbose"]["type"],
            serde_json::json!("boolean")
        );

        let create = result
            .components
            .iter()
            .find(|component| component.name == "createTenantItem")
            .unwrap();
        assert_eq!(
            create.input_schema["properties"]["tenant_id"]["type"],
            serde_json::json!("integer")
        );
        assert_eq!(
            create.input_schema["properties"]["body"]["properties"]["name"]["type"],
            serde_json::json!("string")
        );
        assert_eq!(
            create.input_schema["required"],
            serde_json::json!(["tenant_id", "body"])
        );
    }

    #[test]
    fn request_input_schema_hides_read_only_body_fields() {
        let spec = serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "Demo", "version": "1.0" },
            "servers": [{ "url": "https://api.example.test" }],
            "paths": {
                "/items": {
                    "post": {
                        "operationId": "createItem",
                        "requestBody": {
                            "required": true,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "required": ["id", "name", "secret"],
                                        "allOf": [
                                            {
                                                "type": "object",
                                                "required": ["audit_id"],
                                                "properties": {
                                                    "audit_id": { "type": "string", "readOnly": true }
                                                }
                                            },
                                            {
                                                "type": "object",
                                                "required": ["audit_id", "name"],
                                                "properties": {
                                                    "name": { "type": "string" }
                                                }
                                            }
                                        ],
                                        "properties": {
                                            "id": { "type": "string", "readOnly": true },
                                            "name": { "type": "string" },
                                            "secret": { "type": "string", "writeOnly": true }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let result = analyze_openapi_spec("items", "memory://spec", spec).unwrap();
        let create = result
            .components
            .iter()
            .find(|component| component.name == "createItem")
            .unwrap();

        assert!(create.input_schema["properties"]["body"]["properties"]
            .get("id")
            .is_none());
        assert_eq!(
            create.input_schema["properties"]["body"]["properties"]["secret"]["writeOnly"],
            serde_json::json!(true)
        );
        assert_eq!(
            create.input_schema["properties"]["body"]["required"],
            serde_json::json!(["name", "secret"])
        );
        assert!(
            create.input_schema["properties"]["body"]["allOf"][0]["properties"]
                .get("audit_id")
                .is_none()
        );
        assert_eq!(
            create.input_schema["properties"]["body"]["allOf"][0]["required"],
            serde_json::json!([])
        );
        assert_eq!(
            create.input_schema["properties"]["body"]["allOf"][1]["required"],
            serde_json::json!(["name"])
        );
    }
}
