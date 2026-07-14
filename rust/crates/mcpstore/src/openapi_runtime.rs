use crate::openapi::{
    OpenApiComponent, OpenApiComponentType, OpenApiImportOptions, OpenApiImportResult,
};
use crate::transport::{ContentItem, ToolCallResult};
use crate::{Result, StoreError};
use base64::Engine;
use iri_string::types::{IriReferenceStr, IriStr, UriReferenceStr};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub fn openapi_tool_infos(import: &OpenApiImportResult) -> Vec<crate::registry::ToolInfo> {
    import
        .components
        .iter()
        .filter(|component| component.component_type == OpenApiComponentType::Tool)
        .map(|component| crate::registry::ToolInfo {
            name: component.name.clone(),
            title: None,
            description: component.description.clone().unwrap_or_default(),
            input_schema: component.input_schema.clone(),
            output_schema: tool_output_schema(component),
            annotations: None,
            meta: None,
        })
        .collect()
}

fn tool_output_schema(component: &OpenApiComponent) -> Option<Value> {
    component
        .endpoint
        .responses
        .get("200")
        .and_then(|response| response_body_schema_for_status(Some(response), "application/json"))
        .cloned()
        .or_else(|| {
            component
                .endpoint
                .responses
                .get("default")
                .and_then(|response| {
                    response_body_schema_for_status(Some(response), "application/json")
                })
                .cloned()
        })
}

pub fn list_openapi_resources(import: &OpenApiImportResult) -> Vec<Value> {
    import
        .components
        .iter()
        .filter(|component| component.component_type == OpenApiComponentType::Resource)
        .map(|component| {
            serde_json::json!({
                "uri": openapi_resource_uri(&import.service_name, &component.name),
                "name": component.name,
                "description": component.description,
                "mimeType": declared_response_mime_type(component),
            })
        })
        .collect()
}

pub fn list_openapi_resource_templates(import: &OpenApiImportResult) -> Vec<Value> {
    import
        .components
        .iter()
        .filter(|component| component.component_type == OpenApiComponentType::ResourceTemplate)
        .map(|component| {
            serde_json::json!({
                "uriTemplate": openapi_resource_template_uri(import, component),
                "name": component.name,
                "description": component.description,
                "mimeType": declared_response_mime_type(component),
            })
        })
        .collect()
}

pub async fn call_openapi_tool(
    import: &OpenApiImportResult,
    tool_name: &str,
    args: Value,
    options: &OpenApiImportOptions,
) -> Result<ToolCallResult> {
    let component = import
        .components
        .iter()
        .find(|component| {
            component.name == tool_name && component.component_type == OpenApiComponentType::Tool
        })
        .ok_or_else(|| StoreError::Other(format!("OpenAPI tool not found: {tool_name}")))?;
    let response = execute_component(import, component, args, options).await?;
    let is_error = !response.status.is_success();
    let content = if is_error {
        vec![ContentItem::Text {
            text: response_error_text(response.status, response.mime_type, response.body),
            annotations: None,
            meta: None,
        }]
    } else {
        vec![response_content_item(
            openapi_response_uri(&import.service_name, &component.name),
            response.mime_type,
            response.body,
        )]
    };
    Ok(ToolCallResult { content, is_error })
}

pub async fn read_openapi_resource(
    import: &OpenApiImportResult,
    uri: &str,
    options: &OpenApiImportOptions,
) -> Result<Value> {
    let (component_name, args) = parse_resource_uri(import, uri)?;
    let component = import
        .components
        .iter()
        .find(|component| {
            component.name == component_name
                && matches!(
                    component.component_type,
                    OpenApiComponentType::Resource | OpenApiComponentType::ResourceTemplate
                )
        })
        .ok_or_else(|| StoreError::Other(format!("OpenAPI resource not found: {uri}")))?;
    let response = execute_component(import, component, Value::Object(args), options).await?;
    if !response.status.is_success() {
        return Err(StoreError::Other(format!(
            "OpenAPI resource request failed with status {}: {}",
            response.status,
            response_error_text(response.status, response.mime_type, response.body)
        )));
    }
    Ok(serde_json::json!({
        "contents": [response_resource_content(uri.to_string(), response.mime_type, response.body)]
    }))
}

struct OpenApiHttpResponse {
    status: reqwest::StatusCode,
    mime_type: String,
    body: OpenApiResponseBody,
}

enum OpenApiResponseBody {
    Json(Value),
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Clone)]
struct QueryParameter {
    name: String,
    value: String,
    allow_reserved: bool,
}

fn openapi_resource_uri(service_name: &str, component_name: &str) -> String {
    format!("openapi://{service_name}/{component_name}")
}

fn openapi_response_uri(service_name: &str, component_name: &str) -> String {
    format!("openapi://{service_name}/{component_name}/response")
}

fn openapi_resource_template_uri(
    import: &OpenApiImportResult,
    component: &OpenApiComponent,
) -> String {
    let mut uri = openapi_resource_uri(&import.service_name, &component.name);
    for name in path_parameter_names(&component.endpoint.path) {
        uri.push_str("/{");
        uri.push_str(&name);
        uri.push('}');
    }
    uri
}

fn parse_resource_uri(
    import: &OpenApiImportResult,
    uri: &str,
) -> Result<(String, Map<String, Value>)> {
    let prefix = format!("openapi://{}/", import.service_name);
    let rest = uri
        .strip_prefix(&prefix)
        .ok_or_else(|| StoreError::Other(format!("Invalid OpenAPI resource URI: {uri}")))?;
    let mut segments = rest.split('/');
    let component_name = segments
        .next()
        .filter(|segment| !segment.is_empty())
        .ok_or_else(|| StoreError::Other(format!("Invalid OpenAPI resource URI: {uri}")))?
        .to_string();
    let Some(component) = import
        .components
        .iter()
        .find(|component| component.name == component_name)
    else {
        return Ok((component_name, Map::new()));
    };

    let provided_segments: Vec<&str> = segments.collect();
    let required_path_params = path_parameter_names(&component.endpoint.path);
    match component.component_type {
        OpenApiComponentType::ResourceTemplate => {
            if provided_segments.len() != required_path_params.len() {
                return Err(StoreError::Other(format!(
                    "Invalid OpenAPI resource URI {uri}: expected {} path parameter(s), got {}",
                    required_path_params.len(),
                    provided_segments.len()
                )));
            }
        }
        OpenApiComponentType::Resource => {
            if !provided_segments.is_empty() {
                return Err(StoreError::Other(format!(
                    "Invalid OpenAPI resource URI {uri}: resource does not accept path parameters"
                )));
            }
        }
        OpenApiComponentType::Tool => {}
    }

    let mut args = Map::new();
    for (name, value) in required_path_params.into_iter().zip(provided_segments) {
        args.insert(name, Value::String(percent_decode(value)?));
    }
    Ok((component_name, args))
}

async fn execute_component(
    import: &OpenApiImportResult,
    component: &OpenApiComponent,
    args: Value,
    options: &OpenApiImportOptions,
) -> Result<OpenApiHttpResponse> {
    if import.base_url.is_empty() {
        return Err(StoreError::Other(format!(
            "OpenAPI service has no base URL: {}",
            import.service_name
        )));
    }

    let args = args.as_object().cloned().unwrap_or_default();
    validate_required_arguments(component, &args)?;
    validate_input_schema(component, &args)?;
    let mut path = component.endpoint.path.clone();
    let mut query = Vec::new();
    let mut request_headers = options.headers.clone();
    apply_security(import, component, options, &mut request_headers, &mut query)?;
    for parameter in &component.endpoint.parameters {
        let Some(parameter) = parameter.as_object() else {
            continue;
        };
        let Some(name) = parameter.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(value) = args.get(name) else {
            continue;
        };
        match parameter
            .get("in")
            .and_then(Value::as_str)
            .unwrap_or("query")
        {
            "path" => {
                path = path.replace(
                    &format!("{{{name}}}"),
                    &serialize_path_parameter(parameter, value)?,
                );
            }
            "header" => {
                request_headers.insert(
                    name.to_string(),
                    serialize_header_parameter(parameter, value)?,
                );
            }
            "query" => query.extend(serialize_query_parameter(parameter, name, value)?),
            _ => {}
        }
    }
    if !contains_header(&request_headers, "accept") {
        if let Some(accept) = response_accept_header(component) {
            request_headers.insert("accept".to_string(), accept);
        }
    }

    let url = build_url(&import.base_url, &path, &query)?;
    let method =
        reqwest::Method::from_bytes(component.endpoint.method.as_bytes()).map_err(|err| {
            StoreError::Other(format!(
                "Unsupported OpenAPI method {}: {err}",
                component.endpoint.method
            ))
        })?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(options.timeout_millis.max(1)))
        .build()
        .map_err(|err| StoreError::Other(format!("OpenAPI HTTP client creation failed: {err}")))?;
    let mut request = client.request(method, url);
    for (name, value) in request_headers {
        request = request.header(name, value);
    }
    if let Some(body) = args.get("body") {
        request = apply_request_body(request, component, body).await?;
    }

    let response = request
        .send()
        .await
        .map_err(|err| StoreError::Other(format!("OpenAPI request failed: {err}")))?;
    let status = response.status();
    let mime_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(normalize_mime_type)
        .unwrap_or_else(|| declared_response_mime_type(component));
    let body = if is_binary_response(component, &mime_type) {
        OpenApiResponseBody::Bytes(
            response
                .bytes()
                .await
                .map_err(|err| StoreError::Other(format!("OpenAPI response read failed: {err}")))?
                .to_vec(),
        )
    } else {
        let text = response
            .text()
            .await
            .map_err(|err| StoreError::Other(format!("OpenAPI response read failed: {err}")))?;
        if is_json_mime_type(&mime_type) {
            match serde_json::from_str(&text) {
                Ok(body) => OpenApiResponseBody::Json(body),
                Err(err) if status.is_success() => {
                    return Err(StoreError::Other(format!(
                        "OpenAPI JSON response decode failed: {err}"
                    )));
                }
                Err(_) => OpenApiResponseBody::Text(text),
            }
        } else {
            OpenApiResponseBody::Text(text)
        }
    };
    let body = filter_write_only_response_fields(component, status, &mime_type, body);
    validate_response_schema(component, status, &mime_type, &body)?;
    Ok(OpenApiHttpResponse {
        status,
        mime_type,
        body,
    })
}

fn validate_required_arguments(
    component: &OpenApiComponent,
    args: &Map<String, Value>,
) -> Result<()> {
    let mut missing = Vec::new();
    for parameter in &component.endpoint.parameters {
        let Some(parameter) = parameter.as_object() else {
            continue;
        };
        if !parameter
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(name) = parameter.get("name").and_then(Value::as_str) else {
            continue;
        };
        if is_missing_argument(args.get(name)) {
            let location = parameter
                .get("in")
                .and_then(Value::as_str)
                .unwrap_or("query");
            missing.push(format!("{location}.{name}"));
        }
    }

    if component
        .endpoint
        .request_body
        .as_ref()
        .and_then(|request_body| request_body.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && is_missing_argument(args.get("body"))
    {
        missing.push("body".to_string());
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(StoreError::Other(format!(
            "Missing required OpenAPI argument(s) for {}: {}",
            component.name,
            missing.join(", ")
        )))
    }
}

fn is_missing_argument(value: Option<&Value>) -> bool {
    matches!(value, None | Some(Value::Null))
}

fn validate_input_schema(component: &OpenApiComponent, args: &Map<String, Value>) -> Result<()> {
    let Some(properties) = component
        .input_schema
        .get("properties")
        .and_then(Value::as_object)
    else {
        return Ok(());
    };

    let mut errors = Vec::new();
    validate_read_only_request_fields(component, args, &mut errors);
    for (name, value) in args {
        let Some(schema) = properties.get(name) else {
            continue;
        };
        validate_json_schema_value(schema, value, &argument_path(component, name), &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(StoreError::Other(format!(
            "Invalid OpenAPI argument(s) for {}: {}",
            component.name,
            errors.join(", ")
        )))
    }
}

fn validate_read_only_request_fields(
    component: &OpenApiComponent,
    args: &Map<String, Value>,
    errors: &mut Vec<String>,
) {
    let Some(body) = args.get("body") else {
        return;
    };
    let Some(schema) = request_body_schema_for_validation(component) else {
        return;
    };
    collect_read_only_request_fields(schema, body, "body", errors);
}

fn request_body_schema_for_validation(component: &OpenApiComponent) -> Option<&Value> {
    let content = component
        .endpoint
        .request_body
        .as_ref()?
        .get("content")?
        .as_object()?;
    content
        .get("application/json")
        .or_else(|| content.values().next())
        .and_then(|media| media.get("schema"))
}

fn collect_read_only_request_fields(
    schema: &Value,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    if schema
        .get("readOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        errors.push(format!(
            "{path} is readOnly and cannot be sent in a request"
        ));
        return;
    }

    if let Some(schemas) = schema.get("allOf").and_then(Value::as_array) {
        for child_schema in schemas {
            collect_read_only_request_fields(child_schema, value, path, errors);
        }
    }
    for name in ["anyOf", "oneOf"] {
        if let Some(schemas) = schema.get(name).and_then(Value::as_array) {
            for child_schema in schemas
                .iter()
                .filter(|child_schema| schema_matches(child_schema, value))
            {
                collect_read_only_request_fields(child_schema, value, path, errors);
            }
        }
    }

    if let (Some(properties), Some(object)) = (
        schema.get("properties").and_then(Value::as_object),
        value.as_object(),
    ) {
        for (name, child_schema) in properties {
            if let Some(child_value) = object.get(name) {
                collect_read_only_request_fields(
                    child_schema,
                    child_value,
                    &format!("{path}.{name}"),
                    errors,
                );
            }
        }
    }

    if let (Some(item_schema), Some(items)) = (schema.get("items"), value.as_array()) {
        for (index, item) in items.iter().enumerate() {
            collect_read_only_request_fields(
                item_schema,
                item,
                &format!("{path}[{index}]"),
                errors,
            );
        }
    }

    if let (Some(additional_schema), Some(object)) = (
        schema
            .get("additionalProperties")
            .and_then(Value::as_object),
        value.as_object(),
    ) {
        for (name, child_value) in object {
            if !schema
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|properties| properties.contains_key(name))
            {
                collect_read_only_request_fields(
                    &Value::Object(additional_schema.clone()),
                    child_value,
                    &format!("{path}.{name}"),
                    errors,
                );
            }
        }
    }
}

pub(crate) fn validate_json_schema_value(
    schema: &Value,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    if value.is_null()
        && schema
            .get("nullable")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return;
    }

    if is_file_argument_schema(schema) {
        validate_binary_file_argument(value, path, errors);
        return;
    }

    if let Some(enum_values) = schema.get("enum").and_then(Value::as_array) {
        if !enum_values.iter().any(|enum_value| enum_value == value) {
            errors.push(format!("{path} must match one of the declared enum values"));
            return;
        }
    }

    validate_composition_schema(schema, value, path, errors);

    let Some(schema_type) = schema_type(schema) else {
        return;
    };

    match schema_type {
        "object" => validate_object_schema(schema, value, path, errors),
        "array" => validate_array_schema(schema, value, path, errors),
        "string" if is_binary_string_schema(schema) => {
            validate_binary_file_argument(value, path, errors)
        }
        "string" if !value.is_string() => errors.push(format!("{path} must be a string")),
        "string" => validate_string_constraints(schema, value, path, errors),
        "number" if !value.is_number() => errors.push(format!("{path} must be a number")),
        "number" => validate_numeric_constraints(schema, value, path, errors),
        "integer" if !is_json_integer(value) => errors.push(format!("{path} must be an integer")),
        "integer" => validate_numeric_constraints(schema, value, path, errors),
        "boolean" if !value.is_boolean() => errors.push(format!("{path} must be a boolean")),
        _ => {}
    }
}

fn validate_composition_schema(
    schema: &Value,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    validate_all_of_schema(schema, value, path, errors);
    validate_any_of_schema(schema, value, path, errors);
    validate_one_of_schema(schema, value, path, errors);
    validate_not_schema(schema, value, path, errors);
}

fn validate_all_of_schema(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(all_of) = schema.get("allOf") else {
        return;
    };
    let Some(schemas) = all_of.as_array() else {
        errors.push(format!("{path} has invalid allOf: must be an array"));
        return;
    };
    for child_schema in schemas {
        validate_json_schema_value(child_schema, value, path, errors);
    }
}

fn validate_any_of_schema(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(any_of) = schema.get("anyOf") else {
        return;
    };
    let Some(schemas) = any_of.as_array() else {
        errors.push(format!("{path} has invalid anyOf: must be an array"));
        return;
    };
    if !schemas
        .iter()
        .any(|child_schema| schema_matches(child_schema, value))
    {
        errors.push(format!("{path} must match at least one anyOf schema"));
    }
}

fn validate_one_of_schema(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(one_of) = schema.get("oneOf") else {
        return;
    };
    let Some(schemas) = one_of.as_array() else {
        errors.push(format!("{path} has invalid oneOf: must be an array"));
        return;
    };
    let matches = schemas
        .iter()
        .filter(|child_schema| schema_matches(child_schema, value))
        .count();
    if matches != 1 {
        errors.push(format!("{path} must match exactly one oneOf schema"));
    }
}

fn validate_not_schema(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(not_schema) = schema.get("not") else {
        return;
    };
    if !not_schema.is_object() {
        errors.push(format!("{path} has invalid not: must be an object"));
        return;
    }
    if schema_matches(not_schema, value) {
        errors.push(format!("{path} must not match not schema"));
    }
}

fn schema_matches(schema: &Value, value: &Value) -> bool {
    let mut errors = Vec::new();
    validate_json_schema_value(schema, value, "value", &mut errors);
    errors.is_empty()
}

fn validate_string_constraints(
    schema: &Value,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    let Some(text) = value.as_str() else {
        return;
    };
    let length = text.chars().count();
    if let Some(min_length) = schema.get("minLength").and_then(Value::as_u64) {
        if length < min_length as usize {
            errors.push(format!("{path} length must be at least {min_length}"));
        }
    }
    if let Some(max_length) = schema.get("maxLength").and_then(Value::as_u64) {
        if length > max_length as usize {
            errors.push(format!("{path} length must be at most {max_length}"));
        }
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        match regex::Regex::new(pattern) {
            Ok(regex) if !regex.is_match(text) => {
                errors.push(format!("{path} must match pattern {pattern}"));
            }
            Ok(_) => {}
            Err(err) => errors.push(format!("{path} has invalid pattern {pattern}: {err}")),
        }
    }
    validate_string_format(schema, text, path, errors);
}

fn validate_string_format(schema: &Value, text: &str, path: &str, errors: &mut Vec<String>) {
    let Some(format) = schema.get("format").and_then(Value::as_str) else {
        return;
    };
    match format {
        "date" if chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_err() => {
            errors.push(format!("{path} must match date format YYYY-MM-DD"));
        }
        "date-time" if chrono::DateTime::parse_from_rfc3339(text).is_err() => {
            errors.push(format!("{path} must match RFC3339 date-time format"));
        }
        "uuid" if uuid::Uuid::parse_str(text).is_err() => {
            errors.push(format!("{path} must be a valid UUID"));
        }
        "email" if !email_address::EmailAddress::is_valid(text) => {
            errors.push(format!("{path} must be a valid email address"));
        }
        "hostname" if !is_valid_hostname(text) => {
            errors.push(format!("{path} must be a valid hostname"));
        }
        "ipv4" if text.parse::<std::net::Ipv4Addr>().is_err() => {
            errors.push(format!("{path} must be a valid IPv4 address"));
        }
        "ipv6" if text.parse::<std::net::Ipv6Addr>().is_err() => {
            errors.push(format!("{path} must be a valid IPv6 address"));
        }
        "uri" => validate_uri_format(text, path, errors),
        "uri-reference" if UriReferenceStr::new(text).is_err() => {
            errors.push(format!("{path} must be a valid URI reference"));
        }
        "iri" if IriStr::new(text).is_err() => {
            errors.push(format!("{path} must be a valid IRI"));
        }
        "iri-reference" if IriReferenceStr::new(text).is_err() => {
            errors.push(format!("{path} must be a valid IRI reference"));
        }
        "url" => validate_url_format(text, path, errors),
        "regex" if regex::Regex::new(text).is_err() => {
            errors.push(format!("{path} must be a valid regular expression"));
        }
        "json-pointer" if !is_valid_json_pointer(text) => {
            errors.push(format!("{path} must be a valid JSON Pointer"));
        }
        "relative-json-pointer" if !is_valid_relative_json_pointer(text) => {
            errors.push(format!("{path} must be a valid relative JSON Pointer"));
        }
        _ => {}
    }
}

fn is_valid_hostname(text: &str) -> bool {
    let hostname = text.strip_suffix('.').unwrap_or(text);
    if hostname.is_empty() || hostname.len() > 253 || !hostname.is_ascii() {
        return false;
    }
    hostname.split('.').all(is_valid_hostname_label)
}

fn is_valid_hostname_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn validate_uri_format(text: &str, path: &str, errors: &mut Vec<String>) {
    match text.parse::<http::Uri>() {
        Ok(uri) if uri.scheme().is_some() => {}
        _ => errors.push(format!("{path} must be a valid absolute URI")),
    }
}

fn validate_url_format(text: &str, path: &str, errors: &mut Vec<String>) {
    match text.parse::<http::Uri>() {
        Ok(uri)
            if matches!(uri.scheme_str(), Some("http" | "https")) && uri.authority().is_some() => {}
        _ => errors.push(format!("{path} must be a valid HTTP(S) URL")),
    }
}

fn is_valid_json_pointer(text: &str) -> bool {
    text.is_empty()
        || text
            .strip_prefix('/')
            .map(tokens_have_valid_json_pointer_escapes)
            .unwrap_or(false)
}

fn is_valid_relative_json_pointer(text: &str) -> bool {
    let Some((offset, tail)) = split_relative_json_pointer_offset(text) else {
        return false;
    };
    if offset.len() > 1 && offset.starts_with('0') {
        return false;
    }
    tail.is_empty()
        || tail == "#"
        || tail
            .strip_prefix('/')
            .map(tokens_have_valid_json_pointer_escapes)
            .unwrap_or(false)
}

fn split_relative_json_pointer_offset(text: &str) -> Option<(&str, &str)> {
    let split_at = text
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_ascii_digit()).then_some(index))
        .unwrap_or(text.len());
    if split_at == 0 {
        return None;
    }
    Some(text.split_at(split_at))
}

fn tokens_have_valid_json_pointer_escapes(tokens: &str) -> bool {
    let mut chars = tokens.chars();
    while let Some(ch) = chars.next() {
        if ch == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}

fn validate_numeric_constraints(
    schema: &Value,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    let Some(number) = value.as_f64() else {
        return;
    };
    let has_boolean_exclusive_minimum = schema
        .get("exclusiveMinimum")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let has_boolean_exclusive_maximum = schema
        .get("exclusiveMaximum")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !has_boolean_exclusive_minimum {
        if let Some(minimum) = schema_number(schema, "minimum") {
            if number < minimum {
                errors.push(format!("{path} must be greater than or equal to {minimum}"));
            }
        }
    }
    if !has_boolean_exclusive_maximum {
        if let Some(maximum) = schema_number(schema, "maximum") {
            if number > maximum {
                errors.push(format!("{path} must be less than or equal to {maximum}"));
            }
        }
    }
    if let Some(exclusive_minimum) = exclusive_limit(schema, "exclusiveMinimum") {
        if number <= exclusive_minimum {
            errors.push(format!("{path} must be greater than {exclusive_minimum}"));
        }
    }
    if let Some(exclusive_maximum) = exclusive_limit(schema, "exclusiveMaximum") {
        if number >= exclusive_maximum {
            errors.push(format!("{path} must be less than {exclusive_maximum}"));
        }
    }
    if let Some(multiple_of) = schema_number(schema, "multipleOf") {
        if multiple_of <= 0.0 {
            errors.push(format!(
                "{path} has invalid multipleOf {multiple_of}: must be greater than 0"
            ));
        } else if !is_multiple_of(number, multiple_of) {
            errors.push(format!("{path} must be a multiple of {multiple_of}"));
        }
    }
}

fn validate_object_schema(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(object) = value.as_object() else {
        errors.push(format!("{path} must be an object"));
        return;
    };

    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if is_missing_argument(object.get(name)) {
                errors.push(format!("{path}.{name} is required"));
            }
        }
    }

    let additional_properties = schema.get("additionalProperties");
    for (name, child_value) in object {
        if let Some(child_schema) = properties.and_then(|properties| properties.get(name)) {
            validate_json_schema_value(
                child_schema,
                child_value,
                &format!("{path}.{name}"),
                errors,
            );
        } else if additional_properties == Some(&Value::Bool(false)) {
            errors.push(format!("{path}.{name} is not allowed"));
        } else if let Some(child_schema) = additional_properties.and_then(Value::as_object) {
            validate_json_schema_value(
                &Value::Object(child_schema.clone()),
                child_value,
                &format!("{path}.{name}"),
                errors,
            );
        }
    }
}

fn validate_array_schema(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(items) = value.as_array() else {
        errors.push(format!("{path} must be an array"));
        return;
    };
    if let Some(min_items) = schema.get("minItems").and_then(Value::as_u64) {
        if items.len() < min_items as usize {
            errors.push(format!("{path} must contain at least {min_items} item(s)"));
        }
    }
    if let Some(max_items) = schema.get("maxItems").and_then(Value::as_u64) {
        if items.len() > max_items as usize {
            errors.push(format!("{path} must contain at most {max_items} item(s)"));
        }
    }
    if schema
        .get("uniqueItems")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && has_duplicate_json_value(items)
    {
        errors.push(format!("{path} must contain unique items"));
    }
    let Some(item_schema) = schema.get("items") else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        validate_json_schema_value(item_schema, item, &format!("{path}[{index}]"), errors);
    }
}

fn validate_binary_file_argument(value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(object) = value.as_object() else {
        errors.push(format!("{path} must be a file object with bytes or path"));
        return;
    };
    let bytes = object.get("bytes").filter(|value| value.is_string());
    let file_path = object.get("path").filter(|value| value.is_string());
    if bytes.is_some() == file_path.is_some() {
        errors.push(format!(
            "{path} must provide exactly one string field: bytes or path"
        ));
    }
    for name in ["filename", "mimeType", "mime_type"] {
        if object.get(name).is_some_and(|value| !value.is_string()) {
            errors.push(format!("{path}.{name} must be a string"));
        }
    }
}

fn schema_type(schema: &Value) -> Option<&str> {
    match schema.get("type")? {
        Value::String(schema_type) => Some(schema_type),
        Value::Array(types) => types.iter().find_map(Value::as_str),
        _ => None,
    }
}

fn schema_number(schema: &Value, name: &str) -> Option<f64> {
    schema.get(name)?.as_f64()
}

fn exclusive_limit(schema: &Value, name: &str) -> Option<f64> {
    match schema.get(name)? {
        Value::Bool(true) if name == "exclusiveMinimum" => schema_number(schema, "minimum"),
        Value::Bool(true) if name == "exclusiveMaximum" => schema_number(schema, "maximum"),
        Value::Number(number) => number.as_f64(),
        _ => None,
    }
}

fn is_multiple_of(number: f64, multiple_of: f64) -> bool {
    if !number.is_finite() || !multiple_of.is_finite() {
        return false;
    }
    let quotient = number / multiple_of;
    let nearest = quotient.round();
    (quotient - nearest).abs() <= f64::EPSILON * quotient.abs().max(1.0) * 16.0
}

fn has_duplicate_json_value(items: &[Value]) -> bool {
    for (index, item) in items.iter().enumerate() {
        if items[index + 1..].iter().any(|other| other == item) {
            return true;
        }
    }
    false
}

fn argument_path(component: &OpenApiComponent, name: &str) -> String {
    component
        .endpoint
        .parameters
        .iter()
        .filter_map(Value::as_object)
        .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some(name))
        .and_then(|parameter| parameter.get("in").and_then(Value::as_str))
        .map(|location| format!("{location}.{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn is_json_integer(value: &Value) -> bool {
    value.as_i64().is_some() || value.as_u64().is_some()
}

async fn apply_request_body(
    request: reqwest::RequestBuilder,
    component: &OpenApiComponent,
    body: &Value,
) -> Result<reqwest::RequestBuilder> {
    let Some(media_type) = request_body_media_type(component) else {
        return Ok(request.json(body));
    };
    match media_type.as_str() {
        media_type if media_type.contains("json") => Ok(request.json(body)),
        "application/x-www-form-urlencoded" => Ok(request.form(&body_as_fields(body)?)),
        "multipart/form-data" => Ok(request.multipart(multipart_form(component, body).await?)),
        "text/plain" => Ok(request
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(argument_as_string(body))),
        other => Err(StoreError::Other(format!(
            "Unsupported OpenAPI request body media type for {}: {other}",
            component.name
        ))),
    }
}

fn request_body_media_type(component: &OpenApiComponent) -> Option<String> {
    let content = component
        .endpoint
        .request_body
        .as_ref()?
        .get("content")?
        .as_object()?;
    [
        "application/json",
        "application/x-www-form-urlencoded",
        "multipart/form-data",
        "text/plain",
    ]
    .iter()
    .find(|media_type| content.contains_key(**media_type))
    .map(|media_type| (*media_type).to_string())
    .or_else(|| content.keys().next().cloned())
}

fn declared_response_mime_type(component: &OpenApiComponent) -> String {
    for status in ["200", "201", "202", "204", "default"] {
        if let Some(mime_type) = response_mime_type(component.endpoint.responses.get(status)) {
            return mime_type;
        }
    }
    component
        .endpoint
        .responses
        .values()
        .find_map(|response| response_mime_type(Some(response)))
        .unwrap_or_else(|| "application/json".to_string())
}

fn response_accept_header(component: &OpenApiComponent) -> Option<String> {
    let mut media_types = Vec::new();
    for status in ["200", "201", "202", "204", "default"] {
        collect_supported_response_media_types(
            component.endpoint.responses.get(status),
            &mut media_types,
        );
    }
    for response in component.endpoint.responses.values() {
        collect_supported_response_media_types(Some(response), &mut media_types);
    }
    (!media_types.is_empty()).then(|| media_types.join(", "))
}

fn collect_supported_response_media_types(response: Option<&Value>, media_types: &mut Vec<String>) {
    let Some(content) = response
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
    else {
        return;
    };
    for (declared_media_type, media) in content {
        let Some(media_type) = normalize_mime_type(declared_media_type) else {
            continue;
        };
        if is_supported_response_media_type(&media_type, Some(media))
            && !media_types.contains(&media_type)
        {
            media_types.push(media_type);
        }
    }
}

fn response_mime_type(response: Option<&Value>) -> Option<String> {
    let content = response?.get("content")?.as_object()?;
    content
        .iter()
        .filter_map(|(mime_type, media)| {
            normalize_mime_type(mime_type).map(|mime_type| (mime_type, media))
        })
        .find_map(|(mime_type, media)| {
            is_supported_response_media_type(&mime_type, Some(media)).then_some(mime_type)
        })
        .or_else(|| {
            content
                .keys()
                .find_map(|mime_type| normalize_mime_type(mime_type))
        })
}

fn normalize_mime_type(value: &str) -> Option<String> {
    let mime_type = value.split(';').next()?.trim().to_ascii_lowercase();
    (!mime_type.is_empty()).then_some(mime_type)
}

fn is_json_mime_type(mime_type: &str) -> bool {
    mime_type == "application/json" || mime_type.ends_with("+json")
}

fn is_binary_mime_type(mime_type: &str) -> bool {
    mime_type.starts_with("image/")
        || mime_type.starts_with("audio/")
        || matches!(mime_type, "application/octet-stream" | "application/pdf")
}

fn is_supported_response_media_type(mime_type: &str, media: Option<&Value>) -> bool {
    is_json_mime_type(mime_type)
        || mime_type.starts_with("text/")
        || is_binary_mime_type(mime_type)
        || media.is_some_and(media_type_has_binary_schema)
}

fn is_binary_response(component: &OpenApiComponent, mime_type: &str) -> bool {
    is_binary_mime_type(mime_type) || response_media_has_binary_schema(component, mime_type)
}

fn response_media_has_binary_schema(component: &OpenApiComponent, mime_type: &str) -> bool {
    component.endpoint.responses.values().any(|response| {
        response
            .get("content")
            .and_then(Value::as_object)
            .and_then(|content| find_media_type(content, mime_type))
            .is_some_and(media_type_has_binary_schema)
    })
}

fn filter_write_only_response_fields(
    component: &OpenApiComponent,
    status: reqwest::StatusCode,
    mime_type: &str,
    body: OpenApiResponseBody,
) -> OpenApiResponseBody {
    let OpenApiResponseBody::Json(mut value) = body else {
        return body;
    };
    let Some(schema) = response_body_schema_for_validation(component, status, mime_type) else {
        return OpenApiResponseBody::Json(value);
    };
    if remove_write_only_response_fields(schema, &mut value) {
        value = Value::Null;
    }
    OpenApiResponseBody::Json(value)
}

fn validate_response_schema(
    component: &OpenApiComponent,
    status: reqwest::StatusCode,
    mime_type: &str,
    body: &OpenApiResponseBody,
) -> Result<()> {
    if !status.is_success() {
        return Ok(());
    }
    let OpenApiResponseBody::Json(value) = body else {
        return Ok(());
    };
    let Some(schema) = response_body_schema_for_validation(component, status, mime_type) else {
        return Ok(());
    };

    let mut schema = schema.clone();
    if remove_write_only_schema_fields(&mut schema) {
        schema = serde_json::json!({});
    }

    let mut errors = Vec::new();
    validate_json_schema_value(&schema, value, "response", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(StoreError::Other(format!(
            "Invalid OpenAPI response for {}: {}",
            component.name,
            errors.join(", ")
        )))
    }
}

fn response_body_schema_for_validation<'a>(
    component: &'a OpenApiComponent,
    status: reqwest::StatusCode,
    mime_type: &str,
) -> Option<&'a Value> {
    let status_code = status.as_u16().to_string();
    let status_class = format!("{}XX", status.as_u16() / 100);
    response_body_schema_for_status(component.endpoint.responses.get(&status_code), mime_type)
        .or_else(|| {
            response_body_schema_for_status(
                component.endpoint.responses.get(&status_class),
                mime_type,
            )
        })
        .or_else(|| {
            response_body_schema_for_status(component.endpoint.responses.get("default"), mime_type)
        })
}

fn response_body_schema_for_status<'a>(
    response: Option<&'a Value>,
    mime_type: &str,
) -> Option<&'a Value> {
    let content = response?.get("content")?.as_object()?;
    find_media_type(content, mime_type)
        .or_else(|| {
            is_json_mime_type(mime_type).then(|| {
                content.iter().find_map(|(declared, media)| {
                    normalize_mime_type(declared)
                        .filter(|declared| is_json_mime_type(declared))
                        .map(|_| media)
                })
            })?
        })
        .and_then(|media| media.get("schema"))
}

fn remove_write_only_response_fields(schema: &Value, value: &mut Value) -> bool {
    if schema
        .get("writeOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }

    if let Some(schemas) = schema.get("allOf").and_then(Value::as_array) {
        for child_schema in schemas {
            if remove_write_only_response_fields(child_schema, value) {
                return true;
            }
        }
    }
    for name in ["anyOf", "oneOf"] {
        if let Some(schemas) = schema.get(name).and_then(Value::as_array) {
            let matching = schemas
                .iter()
                .filter(|child_schema| schema_matches(child_schema, value))
                .collect::<Vec<_>>();
            for child_schema in matching {
                if remove_write_only_response_fields(child_schema, value) {
                    return true;
                }
            }
        }
    }

    if let (Some(properties), Some(object)) = (
        schema.get("properties").and_then(Value::as_object),
        value.as_object_mut(),
    ) {
        for (name, child_schema) in properties {
            if let Some(child_value) = object.get_mut(name) {
                if remove_write_only_response_fields(child_schema, child_value) {
                    object.remove(name);
                }
            }
        }
    }

    if let (Some(item_schema), Some(items)) = (schema.get("items"), value.as_array_mut()) {
        let mut index = 0;
        while index < items.len() {
            if remove_write_only_response_fields(item_schema, &mut items[index]) {
                items.remove(index);
            } else {
                index += 1;
            }
        }
    }

    if let (Some(additional_schema), Some(object)) = (
        schema
            .get("additionalProperties")
            .and_then(Value::as_object),
        value.as_object_mut(),
    ) {
        let additional_schema = Value::Object(additional_schema.clone());
        let declared = schema.get("properties").and_then(Value::as_object);
        let names = object.keys().cloned().collect::<Vec<_>>();
        for name in names {
            if declared.is_some_and(|properties| properties.contains_key(&name)) {
                continue;
            }
            if let Some(child_value) = object.get_mut(&name) {
                if remove_write_only_response_fields(&additional_schema, child_value) {
                    object.remove(&name);
                }
            }
        }
    }

    false
}

fn remove_write_only_schema_fields(schema: &mut Value) -> bool {
    if schema
        .get("writeOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }

    for name in ["allOf", "anyOf", "oneOf"] {
        if let Some(schemas) = schema.get_mut(name).and_then(Value::as_array_mut) {
            schemas.retain_mut(|child_schema| !remove_write_only_schema_fields(child_schema));
        }
    }

    let write_only_required = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter_map(|(name, child_schema)| {
                    child_schema
                        .get("writeOnly")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                        .then_some(name.clone())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|name| {
            name.as_str()
                .map(|name| !write_only_required.iter().any(|required| required == name))
                .unwrap_or(true)
        });
    }

    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        properties.retain(|_, child_schema| !remove_write_only_schema_fields(child_schema));
    }

    if let Some(item_schema) = schema.get_mut("items") {
        if remove_write_only_schema_fields(item_schema) {
            *item_schema = serde_json::json!({});
        }
    }

    if let Some(additional_schema) = schema.get_mut("additionalProperties") {
        if additional_schema.is_object() && remove_write_only_schema_fields(additional_schema) {
            *additional_schema = Value::Bool(true);
        }
    }

    false
}

fn find_media_type<'a>(content: &'a Map<String, Value>, mime_type: &str) -> Option<&'a Value> {
    content.iter().find_map(|(declared, media)| {
        (normalize_mime_type(declared).as_deref() == Some(mime_type)).then_some(media)
    })
}

fn media_type_has_binary_schema(media: &Value) -> bool {
    media.get("schema").is_some_and(is_binary_string_schema)
}

fn body_as_fields(body: &Value) -> Result<Vec<(String, String)>> {
    let Some(object) = body.as_object() else {
        return Err(StoreError::Other(
            "OpenAPI form request body must be an object".to_string(),
        ));
    };
    Ok(object
        .iter()
        .map(|(name, value)| (name.clone(), argument_as_string(value)))
        .collect())
}

async fn multipart_form(
    component: &OpenApiComponent,
    body: &Value,
) -> Result<reqwest::multipart::Form> {
    let Some(object) = body.as_object() else {
        return Err(StoreError::Other(
            "OpenAPI multipart request body must be an object".to_string(),
        ));
    };
    let binary_fields = multipart_binary_fields(component);
    let mut form = reqwest::multipart::Form::new();
    for (name, value) in object {
        if matches!(binary_fields.get(name), Some(MultipartBinaryField::File)) {
            form = form.part(
                name.clone(),
                multipart_file_part(component, name, value).await?,
            );
        } else if matches!(
            binary_fields.get(name),
            Some(MultipartBinaryField::FileArray)
        ) {
            let Some(files) = value.as_array() else {
                return Err(StoreError::Other(format!(
                    "OpenAPI multipart binary field for {}.{name} must be an array of file objects",
                    component.name
                )));
            };
            for file in files {
                form = form.part(
                    name.clone(),
                    multipart_file_part(component, name, file).await?,
                );
            }
        } else {
            form = form.text(name.clone(), argument_as_string(value));
        }
    }
    Ok(form)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MultipartBinaryField {
    File,
    FileArray,
}

fn multipart_binary_fields(component: &OpenApiComponent) -> HashMap<String, MultipartBinaryField> {
    let Some(schema) = component
        .endpoint
        .request_body
        .as_ref()
        .and_then(|request_body| request_body.get("content"))
        .and_then(|content| content.get("multipart/form-data"))
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
    else {
        return HashMap::new();
    };
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return HashMap::new();
    };
    properties
        .iter()
        .filter_map(|(name, property)| {
            if is_binary_string_schema(property) {
                Some((name.clone(), MultipartBinaryField::File))
            } else if is_binary_string_array_schema(property) {
                Some((name.clone(), MultipartBinaryField::FileArray))
            } else {
                None
            }
        })
        .collect()
}

async fn multipart_file_part(
    component: &OpenApiComponent,
    field_name: &str,
    value: &Value,
) -> Result<reqwest::multipart::Part> {
    let Some(object) = value.as_object() else {
        return Err(StoreError::Other(format!(
            "OpenAPI multipart binary field for {}.{field_name} must be an object with bytes or path",
            component.name
        )));
    };
    let bytes_value = object.get("bytes");
    let path_value = object.get("path");
    if bytes_value.is_some() == path_value.is_some() {
        return Err(StoreError::Other(format!(
            "OpenAPI multipart binary field for {}.{field_name} must provide exactly one of bytes or path",
            component.name
        )));
    }

    let (bytes, derived_filename) = if let Some(value) = bytes_value {
        let encoded = value.as_str().ok_or_else(|| {
            StoreError::Other(format!(
                "OpenAPI multipart binary field for {}.{field_name} bytes must be a base64 string",
                component.name
            ))
        })?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|err| {
                StoreError::Other(format!(
                    "OpenAPI multipart binary field for {}.{field_name} bytes is not valid base64: {err}",
                    component.name
                ))
            })?;
        (bytes, None)
    } else {
        let path = path_value.and_then(Value::as_str).ok_or_else(|| {
            StoreError::Other(format!(
                "OpenAPI multipart binary field for {}.{field_name} path must be a string",
                component.name
            ))
        })?;
        let bytes = tokio::fs::read(path).await.map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI multipart binary field for {}.{field_name} could not read path {path}: {err}",
                component.name
            ))
        })?;
        let filename = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string);
        (bytes, filename)
    };

    let filename = object
        .get("filename")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or(derived_filename)
        .unwrap_or_else(|| field_name.to_string());
    let mut part = reqwest::multipart::Part::bytes(bytes).file_name(filename);
    if let Some(mime_type) = file_mime_type(object) {
        part = part.mime_str(mime_type).map_err(|err| {
            StoreError::Other(format!(
                "OpenAPI multipart binary field for {}.{field_name} has invalid mimeType {mime_type}: {err}",
                component.name
            ))
        })?;
    }
    Ok(part)
}

fn file_mime_type(object: &Map<String, Value>) -> Option<&str> {
    object
        .get("mimeType")
        .or_else(|| object.get("mime_type"))
        .and_then(Value::as_str)
}

fn is_binary_string_schema(schema: &Value) -> bool {
    schema_type(schema) == Some("string")
        && schema
            .get("format")
            .and_then(Value::as_str)
            .map(|format| format.eq_ignore_ascii_case("binary"))
            .unwrap_or(false)
}

fn is_binary_string_array_schema(schema: &Value) -> bool {
    schema_type(schema) == Some("array")
        && schema
            .get("items")
            .map(is_binary_string_schema)
            .unwrap_or(false)
}

fn is_file_argument_schema(schema: &Value) -> bool {
    schema
        .get("x_mcpstore_file")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn apply_security(
    import: &OpenApiImportResult,
    component: &OpenApiComponent,
    options: &OpenApiImportOptions,
    request_headers: &mut HashMap<String, String>,
    query: &mut Vec<QueryParameter>,
) -> Result<()> {
    let requirements = effective_security(import, component);
    if requirements.is_empty() {
        return Ok(());
    }

    let mut unsupported = Vec::new();
    for requirement in requirements {
        let Some(requirement) = requirement.as_object() else {
            continue;
        };
        if requirement.is_empty() {
            return Ok(());
        }

        let mut candidate_headers = request_headers.clone();
        let mut candidate_query = query.clone();
        let mut satisfied = true;
        for scheme_name in requirement.keys() {
            let Some(scheme) = import.security_schemes.get(scheme_name) else {
                satisfied = false;
                unsupported.push(format!("{scheme_name}: security scheme not found"));
                break;
            };
            match apply_security_scheme(
                scheme_name,
                scheme,
                &options.auth,
                &mut candidate_headers,
                &mut candidate_query,
            ) {
                Ok(()) => {}
                Err(err) => {
                    satisfied = false;
                    unsupported.push(err);
                    break;
                }
            }
        }

        if satisfied {
            *request_headers = candidate_headers;
            *query = candidate_query;
            return Ok(());
        }
    }

    let reason = if unsupported.is_empty() {
        "no supported security requirement was satisfied".to_string()
    } else {
        unsupported.join("; ")
    };
    Err(StoreError::Other(format!(
        "OpenAPI security requirement not satisfied for {}: {reason}",
        component.name
    )))
}

fn effective_security<'a>(
    import: &'a OpenApiImportResult,
    component: &'a OpenApiComponent,
) -> &'a [Value] {
    if component.endpoint.security_defined {
        &component.endpoint.security
    } else {
        &import.security
    }
}

fn apply_security_scheme(
    scheme_name: &str,
    scheme: &Value,
    auth: &Map<String, Value>,
    request_headers: &mut HashMap<String, String>,
    query: &mut Vec<QueryParameter>,
) -> std::result::Result<(), String> {
    let Some(scheme) = scheme.as_object() else {
        return Err(format!("{scheme_name}: security scheme must be an object"));
    };

    match scheme
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "apiKey" => {
            let name = scheme
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{scheme_name}: apiKey scheme missing name"))?;
            match scheme.get("in").and_then(Value::as_str).unwrap_or_default() {
                "header" => {
                    if let Some(auth_value) = auth.get(scheme_name) {
                        let credential = auth_string(auth_value).ok_or_else(|| {
                            format!("{scheme_name}: auth value must be a string or token")
                        })?;
                        request_headers.insert(name.to_string(), credential);
                    } else if !contains_header(request_headers, name) {
                        return Err(format!("{scheme_name}: missing auth value"));
                    }
                    Ok(())
                }
                "query" => {
                    let Some(auth_value) = auth.get(scheme_name) else {
                        return Err(format!("{scheme_name}: missing auth value"));
                    };
                    let credential = auth_string(auth_value).ok_or_else(|| {
                        format!("{scheme_name}: auth value must be a string or token")
                    })?;
                    query.push(QueryParameter {
                        name: name.to_string(),
                        value: credential,
                        allow_reserved: false,
                    });
                    Ok(())
                }
                "cookie" => {
                    let Some(auth_value) = auth.get(scheme_name) else {
                        return Err(format!("{scheme_name}: missing auth value"));
                    };
                    let credential = auth_string(auth_value).ok_or_else(|| {
                        format!("{scheme_name}: auth value must be a string or token")
                    })?;
                    append_cookie(request_headers, name, &credential);
                    Ok(())
                }
                other => Err(format!(
                    "{scheme_name}: unsupported apiKey location {other}"
                )),
            }
        }
        "http" => match scheme
            .get("scheme")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "bearer" => {
                if let Some(auth_value) = auth.get(scheme_name) {
                    let token = auth_string(auth_value)
                        .ok_or_else(|| format!("{scheme_name}: bearer auth requires token"))?;
                    request_headers.insert("authorization".to_string(), format!("Bearer {token}"));
                } else if !contains_header(request_headers, "authorization") {
                    return Err(format!("{scheme_name}: missing auth value"));
                }
                Ok(())
            }
            "basic" => {
                if let Some(auth_value) = auth.get(scheme_name) {
                    let Some((username, password)) = basic_credentials(auth_value) else {
                        return Err(format!(
                            "{scheme_name}: basic auth requires username and password"
                        ));
                    };
                    let encoded = base64::engine::general_purpose::STANDARD
                        .encode(format!("{username}:{password}"));
                    request_headers.insert("authorization".to_string(), format!("Basic {encoded}"));
                } else if !contains_header(request_headers, "authorization") {
                    return Err(format!("{scheme_name}: missing auth value"));
                }
                Ok(())
            }
            other => Err(format!(
                "{scheme_name}: unsupported http auth scheme {other}"
            )),
        },
        "oauth2" | "openIdConnect" => Err(format!(
            "{scheme_name}: OAuth/OpenID Connect flows are not supported by the OpenAPI runtime yet"
        )),
        other => Err(format!(
            "{scheme_name}: unsupported security scheme type {other}"
        )),
    }
}

fn contains_header(headers: &HashMap<String, String>, name: &str) -> bool {
    headers.keys().any(|key| key.eq_ignore_ascii_case(name))
}

fn auth_string(value: &Value) -> Option<String> {
    value.as_str().map(ToString::to_string).or_else(|| {
        value
            .as_object()
            .and_then(|object| object.get("token").or_else(|| object.get("value")))
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

fn basic_credentials(value: &Value) -> Option<(String, String)> {
    let object = value.as_object()?;
    let username = object.get("username")?.as_str()?.to_string();
    let password = object.get("password")?.as_str()?.to_string();
    Some((username, password))
}

fn append_cookie(request_headers: &mut HashMap<String, String>, name: &str, value: &str) {
    let entry = request_headers.entry("cookie".to_string()).or_default();
    if !entry.is_empty() {
        entry.push_str("; ");
    }
    entry.push_str(name);
    entry.push('=');
    entry.push_str(value);
}

fn serialize_query_parameter(
    parameter: &Map<String, Value>,
    name: &str,
    value: &Value,
) -> Result<Vec<QueryParameter>> {
    let style = parameter
        .get("style")
        .and_then(Value::as_str)
        .unwrap_or("form");
    let explode = parameter
        .get("explode")
        .and_then(Value::as_bool)
        .unwrap_or(style == "form");
    let allow_reserved = parameter
        .get("allowReserved")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    match style {
        "form" => Ok(query_parameters(
            serialize_form_parameter(name, value, explode),
            allow_reserved,
        )),
        "spaceDelimited" => Ok(query_parameters(
            vec![(
                name.to_string(),
                serialize_array_or_scalar(value, " ", true),
            )],
            allow_reserved,
        )),
        "pipeDelimited" => Ok(query_parameters(
            vec![(
                name.to_string(),
                serialize_array_or_scalar(value, "|", true),
            )],
            allow_reserved,
        )),
        "deepObject" => serialize_deep_object_query_parameter(name, value, explode, allow_reserved),
        other => Err(StoreError::Other(format!(
            "Unsupported OpenAPI query parameter style for {name}: {other}"
        ))),
    }
}

fn serialize_deep_object_query_parameter(
    name: &str,
    value: &Value,
    explode: bool,
    allow_reserved: bool,
) -> Result<Vec<QueryParameter>> {
    if !explode {
        return Err(StoreError::Other(format!(
            "Unsupported OpenAPI query parameter style for {name}: deepObject requires explode=true"
        )));
    }
    let Some(object) = value.as_object() else {
        return Err(StoreError::Other(format!(
            "Unsupported OpenAPI query parameter style for {name}: deepObject requires an object value"
        )));
    };
    let mut parameters = Vec::with_capacity(object.len());
    for (key, value) in object {
        if matches!(value, Value::Array(_) | Value::Object(_)) {
            return Err(StoreError::Other(format!(
                "Unsupported OpenAPI query parameter style for {name}: deepObject nested value {key} must be a scalar"
            )));
        }
        parameters.push(QueryParameter {
            name: format!("{name}[{key}]"),
            value: argument_as_string(value),
            allow_reserved,
        });
    }
    Ok(parameters)
}

fn query_parameters(
    parameters: Vec<(String, String)>,
    allow_reserved: bool,
) -> Vec<QueryParameter> {
    parameters
        .into_iter()
        .map(|(name, value)| QueryParameter {
            name,
            value,
            allow_reserved,
        })
        .collect()
}

fn serialize_form_parameter(name: &str, value: &Value, explode: bool) -> Vec<(String, String)> {
    match value {
        Value::Array(items) if explode => items
            .iter()
            .map(|item| (name.to_string(), argument_as_string(item)))
            .collect(),
        Value::Array(items) => vec![(name.to_string(), join_values(items.iter(), ","))],
        Value::Object(object) if explode => object
            .iter()
            .map(|(key, value)| (key.clone(), argument_as_string(value)))
            .collect(),
        Value::Object(object) => vec![(name.to_string(), join_object(object, ",", false))],
        _ => vec![(name.to_string(), argument_as_string(value))],
    }
}

fn serialize_path_parameter(parameter: &Map<String, Value>, value: &Value) -> Result<String> {
    let name = parameter
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("parameter");
    let style = parameter
        .get("style")
        .and_then(Value::as_str)
        .unwrap_or("simple");
    let explode = parameter
        .get("explode")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    match style {
        "simple" => Ok(serialize_simple_path_parameter(value, explode)),
        "label" => Ok(serialize_label_path_parameter(value, explode)),
        "matrix" => Ok(serialize_matrix_path_parameter(name, value, explode)),
        other => Err(StoreError::Other(format!(
            "Unsupported OpenAPI path parameter style for {name}: {other}"
        ))),
    }
}

fn serialize_header_parameter(parameter: &Map<String, Value>, value: &Value) -> Result<String> {
    let name = parameter
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("parameter");
    let style = parameter
        .get("style")
        .and_then(Value::as_str)
        .unwrap_or("simple");
    let explode = parameter
        .get("explode")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    match style {
        "simple" => Ok(serialize_simple_parameter(value, explode)),
        other => Err(StoreError::Other(format!(
            "Unsupported OpenAPI header parameter style for {name}: {other}"
        ))),
    }
}

fn serialize_simple_parameter(value: &Value, explode: bool) -> String {
    match value {
        Value::Array(items) => join_values(items.iter(), ","),
        Value::Object(object) => join_object(object, ",", explode),
        _ => argument_as_string(value),
    }
}

fn serialize_simple_path_parameter(value: &Value, explode: bool) -> String {
    match value {
        Value::Array(items) => join_encoded_values(items.iter(), ","),
        Value::Object(object) => join_encoded_object(object, ",", explode),
        _ => percent_encode(&argument_as_string(value)),
    }
}

fn serialize_label_path_parameter(value: &Value, explode: bool) -> String {
    match value {
        Value::Array(items) => format!(".{}", join_encoded_values(items.iter(), ".")),
        Value::Object(object) => format!(".{}", join_encoded_object(object, ".", explode)),
        _ => format!(".{}", percent_encode(&argument_as_string(value))),
    }
}

fn serialize_matrix_path_parameter(name: &str, value: &Value, explode: bool) -> String {
    match value {
        Value::Array(items) if explode => items
            .iter()
            .map(|item| {
                format!(
                    ";{}={}",
                    percent_encode(name),
                    percent_encode(&argument_as_string(item))
                )
            })
            .collect::<Vec<_>>()
            .join(""),
        Value::Array(items) => format!(
            ";{}={}",
            percent_encode(name),
            join_encoded_values(items.iter(), ",")
        ),
        Value::Object(object) if explode => object
            .iter()
            .map(|(key, value)| {
                format!(
                    ";{}={}",
                    percent_encode(key),
                    percent_encode(&argument_as_string(value))
                )
            })
            .collect::<Vec<_>>()
            .join(""),
        Value::Object(object) => format!(
            ";{}={}",
            percent_encode(name),
            join_encoded_object(object, ",", false)
        ),
        _ => format!(
            ";{}={}",
            percent_encode(name),
            percent_encode(&argument_as_string(value))
        ),
    }
}

fn serialize_array_or_scalar(value: &Value, separator: &str, scalar_passthrough: bool) -> String {
    match value {
        Value::Array(items) => join_values(items.iter(), separator),
        _ if scalar_passthrough => argument_as_string(value),
        _ => String::new(),
    }
}

fn join_values<'a>(items: impl Iterator<Item = &'a Value>, separator: &str) -> String {
    items
        .map(argument_as_string)
        .collect::<Vec<_>>()
        .join(separator)
}

fn join_object(object: &Map<String, Value>, separator: &str, explode: bool) -> String {
    object
        .iter()
        .flat_map(|(key, value)| {
            if explode {
                vec![format!("{key}={}", argument_as_string(value))]
            } else {
                vec![key.clone(), argument_as_string(value)]
            }
        })
        .collect::<Vec<_>>()
        .join(separator)
}

fn join_encoded_values<'a>(items: impl Iterator<Item = &'a Value>, separator: &str) -> String {
    items
        .map(|item| percent_encode(&argument_as_string(item)))
        .collect::<Vec<_>>()
        .join(separator)
}

fn join_encoded_object(object: &Map<String, Value>, separator: &str, explode: bool) -> String {
    object
        .iter()
        .flat_map(|(key, value)| {
            let key = percent_encode(key);
            let value = percent_encode(&argument_as_string(value));
            if explode {
                vec![format!("{key}={value}")]
            } else {
                vec![key, value]
            }
        })
        .collect::<Vec<_>>()
        .join(separator)
}

fn build_url(base_url: &str, path: &str, query: &[QueryParameter]) -> Result<String> {
    let base = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let mut url = if path.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{path}")
    };
    append_query_string(&mut url, query);
    reqwest::Url::parse(&url)
        .map(|url| url.to_string())
        .map_err(|err| StoreError::Other(format!("Invalid OpenAPI request URL {url}: {err}")))
}

fn append_query_string(url: &mut String, query: &[QueryParameter]) {
    if query.is_empty() {
        return;
    }
    url.push(if url.contains('?') { '&' } else { '?' });
    url.push_str(
        &query
            .iter()
            .map(|parameter| {
                format!(
                    "{}={}",
                    percent_encode(&parameter.name),
                    percent_encode_query_value(&parameter.value, parameter.allow_reserved)
                )
            })
            .collect::<Vec<_>>()
            .join("&"),
    );
}

fn path_parameter_names(path: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('}') else {
            break;
        };
        names.push(after_start[..end].to_string());
        rest = &after_start[end + 1..];
    }
    names
}

fn argument_as_string(value: &Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn response_text(value: OpenApiResponseBody) -> String {
    match value {
        OpenApiResponseBody::Text(text) => text,
        OpenApiResponseBody::Json(value) => {
            serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string())
        }
        OpenApiResponseBody::Bytes(bytes) => {
            base64::engine::general_purpose::STANDARD.encode(bytes)
        }
    }
}

fn response_resource_content(uri: String, mime_type: String, body: OpenApiResponseBody) -> Value {
    match body {
        OpenApiResponseBody::Bytes(bytes) => serde_json::json!({
            "uri": uri,
            "mimeType": mime_type,
            "blob": base64::engine::general_purpose::STANDARD.encode(bytes),
        }),
        body => serde_json::json!({
            "uri": uri,
            "mimeType": mime_type,
            "text": response_text(body),
        }),
    }
}

fn response_content_item(uri: String, mime_type: String, body: OpenApiResponseBody) -> ContentItem {
    match body {
        OpenApiResponseBody::Bytes(bytes) if mime_type.starts_with("image/") => {
            ContentItem::Image {
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
                mime_type,
                annotations: None,
                meta: None,
            }
        }
        OpenApiResponseBody::Bytes(bytes) if mime_type.starts_with("audio/") => {
            ContentItem::Audio {
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
                mime_type,
                annotations: None,
                meta: None,
            }
        }
        OpenApiResponseBody::Bytes(bytes) => ContentItem::Resource {
            resource: serde_json::json!({
                "uri": uri,
                "mimeType": mime_type,
                "blob": base64::engine::general_purpose::STANDARD.encode(bytes),
            }),
            annotations: None,
            meta: None,
        },
        other => ContentItem::Text {
            text: response_text(other),
            annotations: None,
            meta: None,
        },
    }
}

fn response_error_text(
    status: reqwest::StatusCode,
    mime_type: String,
    body: OpenApiResponseBody,
) -> String {
    let body = match body {
        OpenApiResponseBody::Bytes(bytes) => serde_json::json!({
            "mimeType": mime_type,
            "blob": base64::engine::general_purpose::STANDARD.encode(bytes),
        }),
        OpenApiResponseBody::Json(value) => value,
        OpenApiResponseBody::Text(text) => Value::String(text),
    };
    response_text(OpenApiResponseBody::Json(serde_json::json!({
        "status": status.as_u16(),
        "reason": status.canonical_reason().unwrap_or_default(),
        "body": body,
    })))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn percent_encode_query_value(value: &str, allow_reserved: bool) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~')
            || (allow_reserved && is_reserved_query_byte(byte))
        {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn is_reserved_query_byte(byte: u8) -> bool {
    matches!(
        byte,
        b':' | b'/'
            | b'?'
            | b'#'
            | b'['
            | b']'
            | b'@'
            | b'!'
            | b'$'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'='
    )
}

fn percent_decode(value: &str) -> Result<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut iter = value.as_bytes().iter().copied();
    while let Some(byte) = iter.next() {
        if byte == b'%' {
            let hi = iter
                .next()
                .ok_or_else(|| StoreError::Other(format!("Invalid percent encoding: {value}")))?;
            let lo = iter
                .next()
                .ok_or_else(|| StoreError::Other(format!("Invalid percent encoding: {value}")))?;
            let hex = [hi, lo];
            let decoded = u8::from_str_radix(
                std::str::from_utf8(&hex)
                    .map_err(|err| StoreError::Other(format!("Invalid percent encoding: {err}")))?,
                16,
            )
            .map_err(|err| StoreError::Other(format!("Invalid percent encoding: {err}")))?;
            bytes.push(decoded);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes)
        .map_err(|err| StoreError::Other(format!("Invalid UTF-8 in resource URI: {err}")))
}
