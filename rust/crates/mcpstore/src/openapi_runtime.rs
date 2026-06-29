use crate::openapi::{
    OpenApiComponent, OpenApiComponentType, OpenApiImportOptions, OpenApiImportResult,
};
use crate::transport::{ContentItem, ToolCallResult};
use crate::{Result, StoreError};
use base64::Engine;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub fn openapi_tool_infos(import: &OpenApiImportResult) -> Vec<crate::registry::ToolInfo> {
    import
        .components
        .iter()
        .filter(|component| component.component_type == OpenApiComponentType::Tool)
        .map(|component| crate::registry::ToolInfo {
            name: component.name.clone(),
            description: component.description.clone().unwrap_or_default(),
            schema: component.input_schema.clone(),
        })
        .collect()
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
                "mimeType": "application/json",
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
                "mimeType": "application/json",
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
    let text = if is_error {
        response_error_text(response.status, response.body)
    } else {
        response_text(response.body)
    };
    Ok(ToolCallResult {
        content: vec![ContentItem::Text {
            text,
            annotations: None,
            meta: None,
        }],
        is_error,
    })
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
            response_text(response.body)
        )));
    }
    Ok(serde_json::json!({
        "contents": [{
            "uri": uri,
            "mimeType": "application/json",
            "text": response_text(response.body),
        }]
    }))
}

struct OpenApiHttpResponse {
    status: reqwest::StatusCode,
    body: Value,
}

fn openapi_resource_uri(service_name: &str, component_name: &str) -> String {
    format!("openapi://{service_name}/{component_name}")
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

    let url = build_url(&import.base_url, &path)?;
    let method =
        reqwest::Method::from_bytes(component.endpoint.method.as_bytes()).map_err(|err| {
            StoreError::Other(format!(
                "Unsupported OpenAPI method {}: {err}",
                component.endpoint.method
            ))
        })?;
    let client = reqwest::Client::new();
    let mut request = client.request(method, url).query(&query);
    for (name, value) in request_headers {
        request = request.header(name, value);
    }
    if let Some(body) = args.get("body") {
        request = apply_request_body(request, component, body)?;
    }

    let response = request
        .send()
        .await
        .map_err(|err| StoreError::Other(format!("OpenAPI request failed: {err}")))?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let text = response
        .text()
        .await
        .map_err(|err| StoreError::Other(format!("OpenAPI response read failed: {err}")))?;
    let body = if content_type.contains("json") {
        match serde_json::from_str(&text) {
            Ok(body) => body,
            Err(err) if status.is_success() => {
                return Err(StoreError::Other(format!(
                    "OpenAPI JSON response decode failed: {err}"
                )));
            }
            Err(_) => Value::String(text),
        }
    } else {
        Value::String(text)
    };
    Ok(OpenApiHttpResponse { status, body })
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

fn apply_request_body(
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
        "multipart/form-data" => {
            reject_multipart_binary_fields(component)?;
            let mut form = reqwest::multipart::Form::new();
            for (name, value) in body_as_fields(body)? {
                form = form.text(name, value);
            }
            Ok(request.multipart(form))
        }
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

fn reject_multipart_binary_fields(component: &OpenApiComponent) -> Result<()> {
    let Some(schema) = component
        .endpoint
        .request_body
        .as_ref()
        .and_then(|request_body| request_body.get("content"))
        .and_then(|content| content.get("multipart/form-data"))
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
    else {
        return Ok(());
    };
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(());
    };
    for (name, property) in properties {
        let is_binary = property
            .get("format")
            .and_then(Value::as_str)
            .map(|format| format.eq_ignore_ascii_case("binary"))
            .unwrap_or(false);
        if is_binary {
            return Err(StoreError::Other(format!(
                "Unsupported OpenAPI multipart binary field for {}: {name}",
                component.name
            )));
        }
    }
    Ok(())
}

fn apply_security(
    import: &OpenApiImportResult,
    component: &OpenApiComponent,
    options: &OpenApiImportOptions,
    request_headers: &mut HashMap<String, String>,
    query: &mut Vec<(String, String)>,
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
    query: &mut Vec<(String, String)>,
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
                    query.push((name.to_string(), credential));
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
) -> Result<Vec<(String, String)>> {
    let style = parameter
        .get("style")
        .and_then(Value::as_str)
        .unwrap_or("form");
    let explode = parameter
        .get("explode")
        .and_then(Value::as_bool)
        .unwrap_or(style == "form");

    match style {
        "form" => Ok(serialize_form_parameter(name, value, explode)),
        "spaceDelimited" => Ok(vec![(
            name.to_string(),
            serialize_array_or_scalar(value, " ", true),
        )]),
        "pipeDelimited" => Ok(vec![(
            name.to_string(),
            serialize_array_or_scalar(value, "|", true),
        )]),
        "deepObject" => Err(StoreError::Other(format!(
            "Unsupported OpenAPI query parameter style for {name}: deepObject"
        ))),
        other => Err(StoreError::Other(format!(
            "Unsupported OpenAPI query parameter style for {name}: {other}"
        ))),
    }
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

fn build_url(base_url: &str, path: &str) -> Result<String> {
    let base = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let url = if path.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{path}")
    };
    reqwest::Url::parse(&url)
        .map(|url| url.to_string())
        .map_err(|err| StoreError::Other(format!("Invalid OpenAPI request URL {url}: {err}")))
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

fn response_text(value: Value) -> String {
    match value {
        Value::String(text) => text,
        other => serde_json::to_string(&other).unwrap_or_else(|_| "null".to_string()),
    }
}

fn response_error_text(status: reqwest::StatusCode, body: Value) -> String {
    response_text(serde_json::json!({
        "status": status.as_u16(),
        "reason": status.canonical_reason().unwrap_or_default(),
        "body": body,
    }))
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
