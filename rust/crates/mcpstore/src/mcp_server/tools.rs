use super::*;

pub(super) fn build_session_state_tools() -> HashMap<String, Tool> {
    [
        session_state_tool(
            SESSION_STATE_LIST_TOOL,
            "List all JSON session_state values for a MCPStore business session.",
            session_state_schema(&[]),
            true,
        ),
        session_state_tool(
            SESSION_STATE_GET_TOOL,
            "Get one JSON session_state value for a MCPStore business session.",
            session_state_schema(&["key"]),
            true,
        ),
        session_state_tool(
            SESSION_STATE_SET_TOOL,
            "Set one JSON session_state value for a MCPStore business session.",
            session_state_schema(&["key", "value"]),
            false,
        ),
        session_state_tool(
            SESSION_STATE_DELETE_TOOL,
            "Delete one JSON session_state value for a MCPStore business session.",
            session_state_schema(&["key"]),
            false,
        ),
        session_state_tool(
            SESSION_STATE_CLEAR_TOOL,
            "Clear all JSON session_state values for a MCPStore business session.",
            session_state_schema(&[]),
            false,
        ),
        session_state_tool(
            SESSION_SNAPSHOT_EXPORT_TOOL,
            "Export all MCPStore business sessions and session state as a portable snapshot.",
            session_snapshot_schema(&[]),
            true,
        ),
        session_state_tool(
            SESSION_SNAPSHOT_IMPORT_TOOL,
            "Import a MCPStore business session snapshot without overwriting changed local state.",
            session_snapshot_schema(&["snapshot"]),
            false,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

pub(super) fn session_state_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(!read_only)
        .idempotent(matches!(
            name,
            SESSION_STATE_LIST_TOOL
                | SESSION_STATE_GET_TOOL
                | SESSION_STATE_CLEAR_TOOL
                | SESSION_SNAPSHOT_EXPORT_TOOL
                | SESSION_SNAPSHOT_IMPORT_TOOL
        ))
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

pub(super) fn session_snapshot_schema(required: &[&str]) -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "snapshot".to_string(),
        serde_json::json!({
            "type": "object",
            "description": "Snapshot returned by mcpstore_session_snapshot_export."
        }),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

pub(super) fn session_state_schema(required: &[&str]) -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "session_key".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "MCPStore business session key. Optional when the server was started with a default session key."
        }),
    );
    properties.insert(
        "_mcpstore_session_key".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Alias for session_key used by MCPStore business session routing."
        }),
    );
    properties.insert(
        "key".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Session state key. Must be non-empty and must not contain ':'."
        }),
    );
    properties.insert(
        "value".to_string(),
        serde_json::json!({
            "description": "JSON-serializable session state value."
        }),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

pub(super) fn build_tool_transform_tools() -> HashMap<String, Tool> {
    [
        tool_transform_tool(
            TOOL_TRANSFORM_LIST_TOOL,
            "List all Rust-backed MCPStore tool transform rules.",
            tool_transform_schema(&[]),
            true,
        ),
        tool_transform_tool(
            TOOL_TRANSFORM_GET_TOOL,
            "Get one Rust-backed MCPStore tool transform rule.",
            tool_transform_schema(&["instance_id", "tool_name"]),
            true,
        ),
        tool_transform_tool(
            TOOL_TRANSFORM_SET_TOOL,
            "Set one Rust-backed MCPStore tool transform rule.",
            tool_transform_schema(&["instance_id", "tool_name"]),
            false,
        ),
        tool_transform_tool(
            TOOL_TRANSFORM_DELETE_TOOL,
            "Delete one Rust-backed MCPStore tool transform rule.",
            tool_transform_schema(&["instance_id", "tool_name"]),
            false,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

pub(super) fn tool_transform_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(!read_only)
        .idempotent(matches!(
            name,
            TOOL_TRANSFORM_LIST_TOOL | TOOL_TRANSFORM_GET_TOOL | TOOL_TRANSFORM_DELETE_TOOL
        ))
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

pub(super) fn tool_transform_schema(required: &[&str]) -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "instance_id".to_string(),
        serde_json::json!({
            "type": "string",
            "format": "uuid",
            "description": "Exact deterministic MCPStore service instance id."
        }),
    );
    properties.insert(
        "tool_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Original tool name or current transformed display name."
        }),
    );
    properties.insert(
        "display_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Optional display name exposed on scoped agent/tool surfaces."
        }),
    );
    properties.insert(
        "description".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Optional description override exposed on scoped agent/tool surfaces."
        }),
    );
    properties.insert(
        "arguments".to_string(),
        serde_json::json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "original_name": {"type": "string"},
                    "new_name": {"type": "string"},
                    "hidden": {"type": "boolean"},
                    "default_value": {},
                    "description": {"type": "string"}
                },
                "required": ["original_name", "hidden"],
                "additionalProperties": false
            }
        }),
    );
    properties.insert(
        "tags".to_string(),
        serde_json::json!({"type": "array", "items": {"type": "string"}}),
    );
    properties.insert(
        "enabled".to_string(),
        serde_json::json!({"type": "boolean"}),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

pub(super) fn build_openapi_tools() -> HashMap<String, Tool> {
    [
        openapi_tool(
            OPENAPI_IMPORT_LIST_TOOL,
            "List all Rust-backed MCPStore OpenAPI import analysis records.",
            openapi_schema(&[], false, false, false),
            true,
        ),
        openapi_tool(
            OPENAPI_IMPORT_GET_TOOL,
            "Get one Rust-backed MCPStore OpenAPI import analysis record.",
            openapi_schema(&["name"], true, false, false),
            true,
        ),
        openapi_tool(
            OPENAPI_IMPORT_SET_TOOL,
            "Import an OpenAPI spec into MCPStore shared state and register an executable HTTP virtual service.",
            openapi_schema(&["name", "spec_url"], true, true, true),
            false,
        ),
        openapi_tool(
            OPENAPI_BUNDLE_TOOL,
            "Bundle an OpenAPI spec with external references resolved without importing or registering a virtual service.",
            openapi_schema(&["spec_url"], false, true, false),
            true,
        ),
        openapi_tool(
            OPENAPI_BUNDLE_ARTIFACT_TOOL,
            "Bundle an OpenAPI spec and return dependency metadata without importing or registering a virtual service.",
            openapi_schema(&["spec_url"], false, true, false),
            true,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

pub(super) fn openapi_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(false)
        .idempotent(matches!(
            name,
            OPENAPI_IMPORT_LIST_TOOL
                | OPENAPI_IMPORT_GET_TOOL
                | OPENAPI_BUNDLE_TOOL
                | OPENAPI_BUNDLE_ARTIFACT_TOOL
        ))
        .open_world(matches!(
            name,
            OPENAPI_IMPORT_SET_TOOL | OPENAPI_BUNDLE_TOOL | OPENAPI_BUNDLE_ARTIFACT_TOOL
        ));
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

pub(super) fn openapi_schema(
    required: &[&str],
    include_name: bool,
    include_spec_input: bool,
    include_import_options: bool,
) -> Map<String, Value> {
    let mut properties = Map::new();
    if include_name {
        properties.insert(
            "name".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "MCPStore OpenAPI import/service name."
            }),
        );
    }
    if include_spec_input {
        properties.insert(
            "spec_url".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "OpenAPI spec URL. When spec or spec_text is also provided, this is used as source metadata and as the base URI for relative external references."
            }),
        );
        properties.insert(
            "spec".to_string(),
            serde_json::json!({
                "type": "object",
                "description": "Optional OpenAPI document. If omitted, MCPStore fetches spec_url."
            }),
        );
        properties.insert(
            "spec_text".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "Optional OpenAPI JSON or YAML document text. Mutually exclusive with spec."
            }),
        );
    }
    if include_import_options {
        properties.insert(
            "headers".to_string(),
            serde_json::json!({
                "type": "object",
                "description": "Optional static HTTP headers sent by the OpenAPI virtual service.",
                "additionalProperties": { "type": "string" }
            }),
        );
        properties.insert(
            "auth".to_string(),
            serde_json::json!({
                "type": "object",
                "description": "Optional credentials keyed by OpenAPI security scheme name. Values may be strings, token/value objects, or username/password objects for basic auth."
            }),
        );
    }
    if include_spec_input {
        properties.insert(
            "ref_cache".to_string(),
            serde_json::json!({
                "type": "object",
                "description": "Optional external $ref shared document cache policy.",
                "properties": {
                    "enabled": { "type": "boolean" },
                    "ttl_seconds": { "type": "integer", "minimum": 0 }
                },
                "additionalProperties": false
            }),
        );
        properties.insert(
            "fetch_timeout_millis".to_string(),
            serde_json::json!({
                "type": "integer",
                "minimum": 1,
                "description": "Optional timeout in milliseconds for fetching OpenAPI specs and external references."
            }),
        );
        properties.insert(
            "timeout_millis".to_string(),
            serde_json::json!({
                "type": "integer",
                "minimum": 1,
                "description": "Optional timeout in milliseconds for OpenAPI import runtime operations. Bundle tools use this as a fallback when fetch_timeout_millis is omitted."
            }),
        );
    }

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

pub(super) fn build_cache_tools() -> HashMap<String, Tool> {
    [
        cache_tool(
            CACHE_HEALTH_TOOL,
            "Read MCPStore cache backend health and collection coverage from Rust core.",
            empty_object_schema(),
            true,
        ),
        cache_tool(
            CACHE_INSPECT_TOOL,
            "Inspect MCPStore cache backend state, counts, collections, and request metrics from Rust core.",
            empty_object_schema(),
            true,
        ),
        cache_tool(
            CACHE_SWITCH_TOOL,
            "Switch the MCPStore cache backend and migrate existing Rust core state into the target backend.",
            cache_switch_schema(),
            false,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

pub(super) fn build_service_tools() -> HashMap<String, Tool> {
    [
        service_tool(
            SERVICE_LIST_TOOL,
            "List MCPStore service instances in the server scope from Rust core.",
            empty_object_schema(),
            true,
        ),
        service_tool(
            SERVICE_INFO_TOOL,
            "Read one MCPStore service instance selected by service_name and scope from Rust core.",
            service_scope_schema(),
            true,
        ),
        service_tool(
            SERVICE_STATE_TOOL,
            "Read one MCPStore service instance status from Rust core.",
            instance_id_schema(),
            true,
        ),
        service_tool(
            SERVICE_CHECK_TOOL,
            "Run a health check for one MCPStore service instance from Rust core.",
            instance_id_schema(),
            true,
        ),
        service_tool(
            SERVICE_ADD_TOOL,
            "Add one MCPStore service definition with an explicit initial scope through Rust core.",
            service_add_schema(),
            false,
        ),
        service_tool(
            SERVICE_PATCH_TOOL,
            "Set one MCPStore service scope descriptor through Rust core.",
            service_scope_descriptor_schema(),
            false,
        ),
        service_tool(
            SERVICE_REMOVE_TOOL,
            "Remove one explicit MCPStore service scope through Rust core.",
            service_scope_schema(),
            false,
        ),
        service_tool(
            SERVICE_CONNECT_TOOL,
            "Connect one MCPStore service instance through Rust core.",
            instance_id_schema(),
            false,
        ),
        service_tool(
            SERVICE_DISCONNECT_TOOL,
            "Disconnect one MCPStore service instance through Rust core.",
            instance_id_schema(),
            false,
        ),
        service_tool(
            SERVICE_RESTART_TOOL,
            "Restart one MCPStore service instance through Rust core.",
            instance_id_schema(),
            false,
        ),
        service_tool(
            SERVICE_WAIT_TOOL,
            "Wait for one MCPStore service instance to become ready through Rust core.",
            service_wait_schema(),
            false,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

pub(super) fn build_event_tools() -> HashMap<String, Tool> {
    [
        event_tool(
            EVENT_HISTORY_TOOL,
            "Read recent MCPStore event history from Rust core.",
            event_history_schema(),
        ),
        event_tool(
            EVENT_CAPABILITY_REPORT_TOOL,
            "Read MCPStore event capability report from Rust core.",
            empty_object_schema(),
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

pub(super) fn service_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(matches!(name, SERVICE_REMOVE_TOOL))
        .idempotent(read_only)
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

pub(super) fn event_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(true)
        .destructive(false)
        .idempotent(true)
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

pub(super) fn cache_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(false)
        .idempotent(read_only)
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

pub(super) fn empty_object_schema() -> Map<String, Value> {
    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(Map::new()));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    schema
}

pub(super) fn object_schema(
    properties: Map<String, Value>,
    required: &[&str],
) -> Map<String, Value> {
    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

pub(super) fn event_history_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "count".to_string(),
        serde_json::json!({
            "type": "integer",
            "minimum": 1,
            "description": "Maximum number of recent events to return. Defaults to 100."
        }),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    schema
}

pub(super) fn scope_ref_schema() -> Value {
    serde_json::json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "type": {"const": "store"}
                },
                "required": ["type"],
                "additionalProperties": false
            },
            {
                "type": "object",
                "properties": {
                    "type": {"const": "agent"},
                    "agent_id": {"type": "string", "minLength": 1}
                },
                "required": ["type", "agent_id"],
                "additionalProperties": false
            }
        ]
    })
}

pub(super) fn service_scope_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "service_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Exact MCPStore service definition name."
        }),
    );
    properties.insert("scope".to_string(), scope_ref_schema());
    object_schema(properties, &["service_name", "scope"])
}

pub(super) fn instance_id_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "instance_id".to_string(),
        serde_json::json!({
            "type": "string",
            "format": "uuid",
            "description": "Exact deterministic MCPStore service instance id."
        }),
    );
    object_schema(properties, &["instance_id"])
}

pub(super) fn service_add_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "service_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Exact MCPStore service definition name."
        }),
    );
    properties.insert("scope".to_string(), scope_ref_schema());
    properties.insert(
        "config".to_string(),
        serde_json::json!({
            "type": "object",
            "description": "MCP service base config used to create the new definition."
        }),
    );
    object_schema(properties, &["service_name", "scope", "config"])
}

pub(super) fn service_scope_descriptor_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "service_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Exact MCPStore service definition name."
        }),
    );
    properties.insert("scope".to_string(), scope_ref_schema());
    properties.insert(
        "descriptor".to_string(),
        serde_json::json!({
            "type": "object",
            "description": "Complete scope descriptor. Its config object is a partial override; null deletes inherited fields."
        }),
    );
    object_schema(properties, &["service_name", "scope", "descriptor"])
}

pub(super) fn service_wait_schema() -> Map<String, Value> {
    let mut properties = instance_id_schema()
        .remove("properties")
        .and_then(|value| value.as_object().cloned())
        .expect("instance id schema must contain properties");
    properties.insert(
        "timeout".to_string(),
        serde_json::json!({
            "type": "integer",
            "minimum": 1,
            "description": "Maximum wait time in seconds. Defaults to 10."
        }),
    );
    object_schema(properties, &["instance_id"])
}

pub(super) fn cache_switch_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "backend".to_string(),
        serde_json::json!({
            "type": "string",
            "enum": ["memory", "redis", "openkeyv_memory", "openkeyv_redis"],
            "description": "Target MCPStore cache backend. Redis backends require redis_url unless the store already has one."
        }),
    );
    properties.insert(
        "redis_url".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Optional Redis URL for redis/openkeyv_redis backends."
        }),
    );
    properties.insert(
        "namespace".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Optional target namespace. Use the same namespace to share state across processes."
        }),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    schema.insert(
        "required".to_string(),
        Value::Array(vec![Value::String("backend".to_string())]),
    );
    schema
}

pub(super) async fn call_event_tool(
    store: &MCPStore,
    tool_name: &str,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        EVENT_HISTORY_TOOL => {
            let count = optional_positive_usize_argument(&arguments, "count")?.unwrap_or(100);
            let events = store.event_history(count).await;
            serde_json::json!({"events": events, "total": events.len()})
        }
        EVENT_CAPABILITY_REPORT_TOOL => {
            let report = store.event_capability_report().await;
            serde_json::json!({"report": report})
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore event 观测工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

pub(super) async fn call_service_tool(
    store: &MCPStore,
    tool_name: &str,
    server_scope: &ScopeRef,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        SERVICE_LIST_TOOL => {
            let services = store
                .list_services_scoped(server_scope)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"services": services, "total": services.len()})
        }
        SERVICE_INFO_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?;
            let scope = required_scope_argument(&arguments)?;
            let instance_id = store
                .instance_id_for_scope(service_name, &scope)
                .await
                .map_err(map_store_error)?;
            let service = store
                .service_info_scoped(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"service": service})
        }
        SERVICE_STATE_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let status = store
                .service_state(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": status})
        }
        SERVICE_CHECK_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let check = store
                .health_check(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"check": check})
        }
        SERVICE_ADD_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?.to_string();
            let scope = required_scope_argument(&arguments)?;
            let mut config = service_config_from_arguments(&arguments)?;
            if config.mcpstore.is_some() {
                return Err(ErrorData::invalid_params(
                    "config must contain only MCP service fields; scope is provided separately",
                    None,
                ));
            }
            let mut scopes = ScopeDeclarations::default();
            match &scope {
                ScopeRef::Store => scopes.store = Some(ScopeDescriptor::default()),
                ScopeRef::Agent { agent_id } => {
                    scopes
                        .agents
                        .insert(agent_id.clone(), ScopeDescriptor::default());
                }
            };
            config.mcpstore = Some(McpStoreExtension {
                scopes,
                revision: 1,
                ..McpStoreExtension::default()
            });
            store
                .add_service(&service_name, config)
                .await
                .map_err(map_store_error)?;
            let instance_id = store
                .instance_id_for_scope(&service_name, &scope)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({
                "status": "ok",
                "service_name": service_name,
                "scope": scope,
                "instance_id": instance_id
            })
        }
        SERVICE_PATCH_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?;
            let scope = required_scope_argument(&arguments)?;
            let descriptor = service_scope_descriptor_from_arguments(&arguments)?;
            let instance_id = store
                .declare_service_scope(service_name, &scope, descriptor)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({
                "status": "ok",
                "service_name": service_name,
                "scope": scope,
                "instance_id": instance_id
            })
        }
        SERVICE_REMOVE_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?;
            let scope = required_scope_argument(&arguments)?;
            let instance_id = store
                .instance_id_for_scope(service_name, &scope)
                .await
                .map_err(map_store_error)?;
            store
                .remove_service_scope(service_name, &scope)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({
                "status": "ok",
                "service_name": service_name,
                "scope": scope,
                "instance_id": instance_id
            })
        }
        SERVICE_CONNECT_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            store
                .connect_service(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": "ok", "instance_id": instance_id})
        }
        SERVICE_DISCONNECT_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            store
                .disconnect_service(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": "ok", "instance_id": instance_id})
        }
        SERVICE_RESTART_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            store
                .restart_service(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": "ok", "instance_id": instance_id})
        }
        SERVICE_WAIT_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let timeout =
                optional_positive_u64_argument_with_label(&arguments, "timeout", "service wait")?
                    .unwrap_or(10);
            let status = store
                .wait_instance_ready(instance_id, timeout)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({
                "status": status,
                "instance_id": instance_id,
                "timeout": timeout
            })
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore service 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

pub(super) async fn call_session_state_tool(
    store: &MCPStore,
    tool_name: &str,
    meta: Option<&rmcp::model::Meta>,
    arguments: Map<String, Value>,
    default_session_key: Option<&str>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        SESSION_SNAPSHOT_EXPORT_TOOL => {
            let snapshot = store
                .export_sessions_snapshot()
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"snapshot": snapshot})
        }
        SESSION_SNAPSHOT_IMPORT_TOOL => {
            let snapshot = arguments
                .get("snapshot")
                .cloned()
                .ok_or_else(|| ErrorData::invalid_params("缺少参数: snapshot", None))?;
            let report = store
                .import_sessions_snapshot(snapshot)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"report": report})
        }
        SESSION_STATE_LIST_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let state = store
                .list_session_state(&session_key)
                .await
                .map_err(map_store_error)?;
            let values = state.values.clone();
            serde_json::json!({"state": state, "values": values})
        }
        SESSION_STATE_GET_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let key = required_argument_string(&arguments, "key")?;
            let value = store
                .get_session_state_value(&session_key, key)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"key": key, "value": value})
        }
        SESSION_STATE_SET_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let key = required_argument_string(&arguments, "key")?;
            let value = arguments
                .get("value")
                .cloned()
                .ok_or_else(|| ErrorData::invalid_params("缺少参数: value", None))?;
            let state = store
                .set_session_state(&session_key, key, value)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"state": state})
        }
        SESSION_STATE_DELETE_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let key = required_argument_string(&arguments, "key")?;
            let state = store
                .delete_session_state(&session_key, key)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"state": state})
        }
        SESSION_STATE_CLEAR_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let state = store
                .clear_session_state(&session_key)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"state": state})
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore session_state 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

pub(super) async fn call_tool_transform_tool(
    store: &MCPStore,
    tool_name: &str,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        TOOL_TRANSFORM_LIST_TOOL => {
            let transforms = store
                .list_tool_transforms()
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"transforms": transforms, "total": transforms.len()})
        }
        TOOL_TRANSFORM_GET_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let tool_name = required_argument_string(&arguments, "tool_name")?;
            let transform = store
                .get_tool_transform(instance_id, tool_name)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"transform": transform})
        }
        TOOL_TRANSFORM_SET_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let tool_name = required_argument_string(&arguments, "tool_name")?.to_string();
            let patch = serde_json::from_value::<ToolTransformPatch>(Value::Object(arguments))
                .map_err(|error| {
                    ErrorData::invalid_params(
                        format!("工具转换规则参数反序列化失败: {error}"),
                        None,
                    )
                })?;
            let transform = store
                .set_tool_transform(instance_id, &tool_name, patch)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"transform": transform})
        }
        TOOL_TRANSFORM_DELETE_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let tool_name = required_argument_string(&arguments, "tool_name")?;
            store
                .delete_tool_transform(instance_id, tool_name)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": "ok"})
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore tool transform 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

pub(super) async fn call_openapi_tool(
    store: &MCPStore,
    tool_name: &str,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        OPENAPI_IMPORT_LIST_TOOL => {
            let imports = store
                .list_openapi_imports()
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"imports": imports, "total": imports.len()})
        }
        OPENAPI_IMPORT_GET_TOOL => {
            let name = required_argument_string(&arguments, "name")?;
            let import = store
                .get_openapi_import(name)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"import": import})
        }
        OPENAPI_IMPORT_SET_TOOL => {
            let name = required_argument_string(&arguments, "name")?.to_string();
            let spec_url = required_argument_string(&arguments, "spec_url")?.to_string();
            let options = openapi_import_options_from_arguments(&arguments)?;
            let spec = arguments.get("spec").cloned();
            let spec_text = arguments
                .get("spec_text")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty());
            let import = match (spec, spec_text) {
                (Some(_), Some(_)) => {
                    return Err(ErrorData::invalid_params(
                        "spec and spec_text cannot both be provided",
                        None,
                    ));
                }
                (Some(spec), None) => {
                    store
                        .import_openapi_service_from_spec_with_options(
                            &name, &spec_url, spec, options,
                        )
                        .await
                }
                (None, Some(spec_text)) => {
                    store
                        .import_openapi_service_from_spec_text_with_options(
                            &name, &spec_url, spec_text, options,
                        )
                        .await
                }
                (None, None) => {
                    store
                        .import_openapi_service_with_options(&name, &spec_url, options)
                        .await
                }
            }
            .map_err(map_store_error)?;
            serde_json::json!({"import": import})
        }
        OPENAPI_BUNDLE_TOOL => {
            let spec_url = required_argument_string(&arguments, "spec_url")?.to_string();
            let options = openapi_bundle_options_from_arguments(&arguments)?;
            let spec = arguments.get("spec").cloned();
            let spec_text = arguments
                .get("spec_text")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty());
            let bundle = match (spec, spec_text) {
                (Some(_), Some(_)) => {
                    return Err(ErrorData::invalid_params(
                        "spec and spec_text cannot both be provided",
                        None,
                    ));
                }
                (Some(spec), None) => {
                    store
                        .bundle_openapi_spec_from_value_with_options(&spec_url, spec, options)
                        .await
                }
                (None, Some(spec_text)) => {
                    store
                        .bundle_openapi_spec_from_text_with_options(&spec_url, spec_text, options)
                        .await
                }
                (None, None) => {
                    store
                        .bundle_openapi_spec_with_options(&spec_url, options)
                        .await
                }
            }
            .map_err(map_store_error)?;
            serde_json::json!({"bundle": bundle})
        }
        OPENAPI_BUNDLE_ARTIFACT_TOOL => {
            let spec_url = required_argument_string(&arguments, "spec_url")?.to_string();
            let options = openapi_bundle_options_from_arguments(&arguments)?;
            let spec = arguments.get("spec").cloned();
            let spec_text = arguments
                .get("spec_text")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty());
            let artifact = match (spec, spec_text) {
                (Some(_), Some(_)) => {
                    return Err(ErrorData::invalid_params(
                        "spec and spec_text cannot both be provided",
                        None,
                    ));
                }
                (Some(spec), None) => {
                    store
                        .bundle_openapi_artifact_from_value_with_options(&spec_url, spec, options)
                        .await
                }
                (None, Some(spec_text)) => {
                    store
                        .bundle_openapi_artifact_from_text_with_options(
                            &spec_url, spec_text, options,
                        )
                        .await
                }
                (None, None) => {
                    store
                        .bundle_openapi_artifact_with_options(&spec_url, options)
                        .await
                }
            }
            .map_err(map_store_error)?;
            serde_json::json!({"artifact": artifact})
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore OpenAPI 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

pub(super) async fn call_cache_tool(
    store: &Arc<MCPStore>,
    tool_name: &str,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        CACHE_HEALTH_TOOL => {
            let health = store.cache_health_check().await.map_err(map_store_error)?;
            serde_json::json!({"health": health})
        }
        CACHE_INSPECT_TOOL => {
            let inspect = store.cache_inspect().await.map_err(map_store_error)?;
            serde_json::json!({"inspect": inspect})
        }
        CACHE_SWITCH_TOOL => {
            let backend = required_argument_string(&arguments, "backend")?;
            let storage = parse_cache_storage_argument(backend)?;
            let backend_label = storage.as_str();
            let redis_url = optional_string_argument(&arguments, "redis_url");
            let namespace = optional_string_argument(&arguments, "namespace");
            let had_reactor = store.has_reactor().await;
            let snapshot = store
                .switch_cache_storage(storage, redis_url, namespace)
                .await
                .map_err(map_store_error)?;
            if had_reactor {
                store
                    .restart_control_reactor()
                    .await
                    .map_err(map_store_error)?;
            }
            serde_json::json!({
                "backend": backend_label,
                "namespace": store.namespace(),
                "snapshot": snapshot,
            })
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore cache 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

pub(super) fn resolve_session_state_key(
    meta: Option<&rmcp::model::Meta>,
    arguments: &Map<String, Value>,
    default_session_key: Option<&str>,
) -> Result<String, ErrorData> {
    meta.and_then(|meta| meta.0.get("_mcpstore_session_key"))
        .and_then(Value::as_str)
        .or_else(|| {
            arguments
                .get("_mcpstore_session_key")
                .or_else(|| arguments.get("session_key"))
                .and_then(Value::as_str)
        })
        .or(default_session_key)
        .filter(|session_key| !session_key.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ErrorData::invalid_params("缺少参数: session_key", None))
}

pub(super) fn required_argument_string<'a>(
    arguments: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, ErrorData> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ErrorData::invalid_params(format!("缺少参数: {field}"), None))
}

pub(super) fn required_scope_argument(
    arguments: &Map<String, Value>,
) -> Result<ScopeRef, ErrorData> {
    let value = arguments
        .get("scope")
        .cloned()
        .ok_or_else(|| ErrorData::invalid_params("缺少参数: scope", None))?;
    serde_json::from_value(value)
        .map_err(|error| ErrorData::invalid_params(format!("scope 参数无效: {error}"), None))
}

pub(super) fn required_instance_id_argument(
    arguments: &Map<String, Value>,
) -> Result<InstanceId, ErrorData> {
    let value = required_argument_string(arguments, "instance_id")?;
    InstanceId::from_str(value)
        .map_err(|error| ErrorData::invalid_params(format!("instance_id 参数无效: {error}"), None))
}

pub(super) fn optional_string_argument(
    arguments: &Map<String, Value>,
    field: &str,
) -> Option<String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn optional_positive_usize_argument(
    arguments: &Map<String, Value>,
    field: &str,
) -> Result<Option<usize>, ErrorData> {
    let Some(value) = arguments.get(field) else {
        return Ok(None);
    };
    let parsed = match value {
        Value::Null => return Ok(None),
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse::<u64>().ok(),
        _ => None,
    }
    .filter(|value| *value > 0)
    .and_then(|value| usize::try_from(value).ok())
    .ok_or_else(|| {
        ErrorData::invalid_params(format!("{field} must be a positive integer"), None)
    })?;
    Ok(Some(parsed))
}

pub(super) fn service_config_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<ServerConfig, ErrorData> {
    let config = arguments
        .get("config")
        .cloned()
        .ok_or_else(|| ErrorData::invalid_params("缺少参数: config", None))?;
    serde_json::from_value::<ServerConfig>(config)
        .map_err(|error| ErrorData::invalid_params(format!("服务配置解析失败: {error}"), None))
}

pub(super) fn service_scope_descriptor_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<ScopeDescriptor, ErrorData> {
    let descriptor = arguments
        .get("descriptor")
        .cloned()
        .ok_or_else(|| ErrorData::invalid_params("缺少参数: descriptor", None))?;
    serde_json::from_value(descriptor).map_err(|error| {
        ErrorData::invalid_params(format!("服务作用域描述解析失败: {error}"), None)
    })
}

pub(super) fn parse_cache_storage_argument(value: &str) -> Result<CacheStorage, ErrorData> {
    match value {
        "memory" => Ok(CacheStorage::Memory),
        "redis" => Ok(CacheStorage::Redis),
        "openkeyv_memory" => Ok(CacheStorage::OpenKeyvMemory),
        "openkeyv_redis" => Ok(CacheStorage::OpenKeyvRedis),
        other => Err(ErrorData::invalid_params(
            format!("不支持的 cache backend: {other}"),
            None,
        )),
    }
}

pub(super) fn openapi_import_options_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<OpenApiImportOptions, ErrorData> {
    let headers = match arguments.get("headers") {
        Some(value) => serde_json::from_value(value.clone()).map_err(|err| {
            ErrorData::invalid_params(format!("OpenAPI headers must be a string map: {err}"), None)
        })?,
        None => HashMap::new(),
    };
    let auth = arguments
        .get("auth")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Ok(OpenApiImportOptions {
        headers,
        auth,
        ref_cache: openapi_ref_cache_policy_from_arguments(arguments)?,
        timeout_millis: optional_positive_u64_argument(arguments, "timeout_millis")?
            .unwrap_or_else(OpenApiImportOptions::default_timeout_millis),
        fetch_timeout_millis: optional_positive_u64_argument(arguments, "fetch_timeout_millis")?
            .unwrap_or_else(OpenApiImportOptions::default_fetch_timeout_millis),
    })
}

pub(super) fn openapi_bundle_options_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<OpenApiBundleOptions, ErrorData> {
    Ok(OpenApiBundleOptions {
        ref_cache: openapi_ref_cache_policy_from_arguments(arguments)?,
        timeout_millis: optional_positive_u64_argument(arguments, "fetch_timeout_millis")?
            .or(optional_positive_u64_argument(arguments, "timeout_millis")?)
            .unwrap_or_else(OpenApiBundleOptions::default_timeout_millis),
    })
}

pub(super) fn optional_positive_u64_argument(
    arguments: &Map<String, Value>,
    field: &str,
) -> Result<Option<u64>, ErrorData> {
    optional_positive_u64_argument_with_label(arguments, field, "OpenAPI")
}

pub(super) fn optional_positive_u64_argument_with_label(
    arguments: &Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<Option<u64>, ErrorData> {
    let Some(value) = arguments.get(field) else {
        return Ok(None);
    };
    let parsed = match value {
        Value::Null => return Ok(None),
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse::<u64>().ok(),
        _ => None,
    }
    .filter(|value| *value > 0)
    .ok_or_else(|| {
        ErrorData::invalid_params(format!("{label} {field} must be a positive integer"), None)
    })?;
    Ok(Some(parsed))
}

pub(super) fn openapi_ref_cache_policy_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<OpenApiRefCachePolicy, ErrorData> {
    match arguments.get("ref_cache") {
        Some(value) => serde_json::from_value(value.clone()).map_err(|err| {
            ErrorData::invalid_params(format!("OpenAPI ref_cache is invalid: {err}"), None)
        }),
        None => Ok(OpenApiRefCachePolicy::default()),
    }
}

pub(super) fn extract_business_session_key(
    meta: Option<&rmcp::model::Meta>,
    mut arguments: Map<String, Value>,
    default_session_key: Option<&str>,
) -> (Value, Option<String>) {
    let meta_session_key = meta
        .and_then(|meta| meta.0.get("_mcpstore_session_key"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let argument_session_key = arguments
        .remove("_mcpstore_session_key")
        .and_then(|value| value.as_str().map(str::to_string));
    let session_key = meta_session_key
        .or(argument_session_key)
        .or_else(|| default_session_key.map(str::to_string));
    (Value::Object(arguments), session_key)
}

pub(super) fn read_required_string(payload: &Value, field: &str) -> Result<String, BoxErr> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("工具元数据缺少字符串字段: {field}").into())
}

pub(super) fn read_required_instance_id(
    payload: &Value,
    field: &str,
) -> Result<InstanceId, BoxErr> {
    let value = payload
        .get(field)
        .ok_or_else(|| format!("工具元数据缺少字段: {field}"))?;
    serde_json::from_value(value.clone())
        .map_err(|error| format!("工具元数据字段 {field} 不是有效 instance_id: {error}").into())
}

pub(super) fn read_required_object(
    payload: &Value,
    field: &str,
) -> Result<Map<String, Value>, BoxErr> {
    payload
        .get(field)
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| format!("工具元数据缺少对象字段: {field}").into())
}

pub(super) fn map_store_error(error: StoreError) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}

pub(super) fn deserialize_items<T>(items: Vec<Value>, label: &str) -> Result<Vec<T>, ErrorData>
where
    T: DeserializeOwned,
{
    items
        .into_iter()
        .map(|item| deserialize_item(item, label))
        .collect()
}

pub(super) fn deserialize_item<T>(item: Value, label: &str) -> Result<T, ErrorData>
where
    T: DeserializeOwned,
{
    serde_json::from_value(item)
        .map_err(|error| ErrorData::internal_error(format!("{label} 反序列化失败: {error}"), None))
}
