//! PyO3 wrapper for the MCPStore Rust runtime surface.

use mcpstore::config::{McpConfig, ServerConfig};
use mcpstore::core::perspective::ToolResolution;
use mcpstore::core::store::{
    BackendKind, CacheHealthReport, EventCapabilityReport, MCPStore, ScopedServiceEntry,
    ScopedServiceHealth, ScopedToolEntry, SourceMode, StoreOptions,
};
use mcpstore::{
    cache::models::{
        ContextToolVisibilityState, HealthStatus, ServiceStatus, SessionContextState,
        SessionEntity, SessionScope, SessionServiceItem, SessionServiceRelation, SessionStateData,
        SessionStatus, SessionStatusState, SessionToolItem, SessionToolVisibility,
        ToolAvailability, ToolPreferenceState, ToolStatusItem, ToolTransformRule,
    },
    ConnectionStatus, ContentItem, Event, ServiceEntry, StoreError, ToolCallResult,
    ToolDescription, ToolInfo,
};
use mcpstore::{
    CreateSessionRequest, OpenApiBundleOptions, OpenApiImportOptions, SessionRetryPolicy,
    SessionToolSelection, ToolTransformPatch, ToolVisibilityFilter,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use crate::py_value::{py_to_serde_value, serde_value_to_py};

#[pyclass(name = "MCPStore")]
pub struct PyMCPStore {
    inner: MCPStore,
}

fn map_store_err(err: StoreError) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(err.to_string())
}

fn parse_openapi_import_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<OpenApiImportOptions> {
    let Some(options) = options else {
        return Ok(OpenApiImportOptions::default());
    };
    if options.is_none() {
        return Ok(OpenApiImportOptions::default());
    }
    let value = py_to_serde_value(options, "OpenAPI import options")?;
    serde_json::from_value(value).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("Invalid OpenAPI import options: {err}"))
    })
}

fn parse_openapi_bundle_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<OpenApiBundleOptions> {
    let Some(options) = options else {
        return Ok(OpenApiBundleOptions::default());
    };
    if options.is_none() {
        return Ok(OpenApiBundleOptions::default());
    }
    let value = py_to_serde_value(options, "OpenAPI bundle options")?;
    serde_json::from_value(value).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("Invalid OpenAPI bundle options: {err}"))
    })
}

fn parse_source_mode(source_mode: Option<&str>) -> PyResult<SourceMode> {
    match source_mode {
        Some("db") => Ok(SourceMode::Db),
        Some("local") | None => Ok(SourceMode::Local),
        Some(other) => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported source_mode: {other}"
        ))),
    }
}

fn parse_backend(backend: Option<&str>) -> PyResult<Option<BackendKind>> {
    match backend {
        Some("memory") => Ok(Some(BackendKind::Memory)),
        Some("redis") => Ok(Some(BackendKind::Redis)),
        Some("openkeyv_memory") => Ok(Some(BackendKind::OpenKeyvMemory)),
        Some("openkeyv_redis") => Ok(Some(BackendKind::OpenKeyvRedis)),
        None => Ok(None),
        Some(other) => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported backend: {other}"
        ))),
    }
}

fn parse_tool_visibility_filter(filter: Option<&str>) -> PyResult<ToolVisibilityFilter> {
    match filter.unwrap_or("available") {
        "all" => Ok(ToolVisibilityFilter::All),
        "available" => Ok(ToolVisibilityFilter::Available),
        "removed" => Ok(ToolVisibilityFilter::Removed),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported tool visibility filter: {other}"
        ))),
    }
}

fn parse_session_scope(scope: Option<&str>) -> PyResult<SessionScope> {
    match scope {
        Some("store") | None => Ok(SessionScope::Store),
        Some("agent") => Ok(SessionScope::Agent),
        Some(other) => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported session scope: {other}"
        ))),
    }
}

fn session_scope_as_str(scope: &SessionScope) -> &'static str {
    match scope {
        SessionScope::Store => "store",
        SessionScope::Agent => "agent",
    }
}

fn session_status_as_str(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Active => "active",
        SessionStatus::Closed => "closed",
        SessionStatus::Expired => "expired",
    }
}

fn backend_as_str(backend: &BackendKind) -> &'static str {
    backend.as_str()
}

fn py_to_server_config(value: &Bound<'_, PyAny>, context: &str) -> PyResult<ServerConfig> {
    let value = py_to_serde_value(value, context)?;
    serde_json::from_value(value).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("{context} conversion failed: {err}"))
    })
}

fn string_map_to_py<'py>(
    py: Python<'py>,
    values: &std::collections::HashMap<String, String>,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    for (key, value) in values {
        dict.set_item(key, value)?;
    }
    Ok(dict)
}

fn server_config_to_py(py: Python<'_>, config: &ServerConfig) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("url", config.url.as_deref())?;
    dict.set_item("command", config.command.as_deref())?;
    dict.set_item("args", string_list_to_py(py, &config.args)?)?;
    dict.set_item("env", string_map_to_py(py, &config.env)?)?;
    dict.set_item("headers", string_map_to_py(py, &config.headers)?)?;
    dict.set_item("transport", config.transport.as_deref())?;
    dict.set_item("workingDir", config.working_dir.as_deref())?;
    dict.set_item("description", config.description.as_deref())?;
    Ok(dict.into_any().unbind())
}

fn mcp_config_to_py(py: Python<'_>, config: &McpConfig) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    let servers = PyDict::new(py);
    for (name, server_config) in &config.mcp_servers {
        servers.set_item(name, server_config_to_py(py, server_config)?)?;
    }
    dict.set_item("mcpServers", servers)?;
    if !config.agents.is_empty() {
        let agents = PyDict::new(py);
        for (agent_id, services) in &config.agents {
            agents.set_item(agent_id, string_list_to_py(py, services)?)?;
        }
        dict.set_item("agents", agents)?;
    }
    Ok(dict.into_any().unbind())
}

fn connection_status_as_str(status: ConnectionStatus) -> &'static str {
    match status {
        ConnectionStatus::Connecting => "connecting",
        ConnectionStatus::Connected => "connected",
        ConnectionStatus::Disconnected => "disconnected",
        ConnectionStatus::Error => "error",
    }
}

fn tool_info_to_py(py: Python<'_>, tool: &ToolInfo) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("name", &tool.name)?;
    dict.set_item("description", &tool.description)?;
    dict.set_item("schema", serde_value_to_py(py, tool.schema.clone())?)?;
    Ok(dict.into_any().unbind())
}

fn tool_description_to_py(py: Python<'_>, tool: &ToolDescription) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("name", &tool.name)?;
    dict.set_item("description", &tool.description)?;
    dict.set_item(
        "input_schema",
        serde_value_to_py(py, tool.input_schema.clone())?,
    )?;
    Ok(dict.into_any().unbind())
}

fn scoped_tool_entry_to_py(py: Python<'_>, tool: &ScopedToolEntry) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("name", &tool.name)?;
    dict.set_item("original_name", &tool.original_name)?;
    dict.set_item("description", &tool.description)?;
    dict.set_item("schema", serde_value_to_py(py, tool.schema.clone())?)?;
    dict.set_item(
        "input_schema",
        serde_value_to_py(py, tool.input_schema.clone())?,
    )?;
    dict.set_item("service_name", &tool.service_name)?;
    dict.set_item("global_service_name", &tool.global_service_name)?;
    dict.set_item("service_global_name", &tool.service_global_name)?;
    dict.set_item("global_tool_name", &tool.global_tool_name)?;
    dict.set_item("client_id", &tool.client_id)?;
    Ok(dict.into_any().unbind())
}

fn service_entry_dict<'py>(
    py: Python<'py>,
    service: &ServiceEntry,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    let tools = PyList::empty(py);
    for tool in &service.tools {
        tools.append(tool_info_to_py(py, tool)?)?;
    }

    dict.set_item("name", &service.name)?;
    dict.set_item("original_name", &service.original_name)?;
    dict.set_item("agent_id", &service.agent_id)?;
    dict.set_item("client_id", &service.name)?;
    dict.set_item("transport", &service.transport)?;
    dict.set_item("url", service.url.as_deref())?;
    dict.set_item("command", service.command.as_deref())?;
    dict.set_item("status", connection_status_as_str(service.status))?;
    dict.set_item("tools", tools)?;
    dict.set_item("config", serde_value_to_py(py, service.config.clone())?)?;
    dict.set_item("added_time", service.added_time)?;
    Ok(dict)
}

fn service_entry_to_py(py: Python<'_>, service: &ServiceEntry) -> PyResult<Py<PyAny>> {
    let dict = service_entry_dict(py, service)?;
    Ok(dict.into_any().unbind())
}

fn scoped_service_entry_to_py(py: Python<'_>, entry: &ScopedServiceEntry) -> PyResult<Py<PyAny>> {
    let dict = service_entry_dict(py, &entry.service)?;
    dict.set_item("tool_count", entry.tool_count)?;
    dict.set_item("client_id", &entry.client_id)?;
    if let Some(global_name) = &entry.global_name {
        dict.set_item("global_name", global_name)?;
    }
    Ok(dict.into_any().unbind())
}

fn event_to_py(py: Python<'_>, event: &Event) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("event_type", &event.event_type)?;
    dict.set_item("event_id", &event.event_id)?;
    dict.set_item("timestamp", event.timestamp)?;
    dict.set_item("priority", event.priority)?;
    dict.set_item("payload", serde_value_to_py(py, event.payload.clone())?)?;
    Ok(dict.into_any().unbind())
}

fn event_capability_report_to_py(
    py: Python<'_>,
    report: &EventCapabilityReport,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("event_bus", report.event_bus)?;
    dict.set_item("history", report.history)?;
    dict.set_item("history_capacity", report.history_capacity)?;
    dict.set_item("cache_event_layer", report.cache_event_layer)?;
    Ok(dict.into_any().unbind())
}

fn tool_resolution_to_py(py: Python<'_>, resolution: &ToolResolution) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("agent_id", &resolution.agent_id)?;
    dict.set_item("local_service_name", &resolution.local_service_name)?;
    dict.set_item("global_service_name", &resolution.global_service_name)?;
    dict.set_item("local_tool_name", &resolution.local_tool_name)?;
    dict.set_item("global_tool_name", &resolution.global_tool_name)?;
    dict.set_item("canonical_tool_name", &resolution.canonical_tool_name)?;
    dict.set_item("resolution_method", &resolution.resolution_method)?;
    dict.set_item("original_input", &resolution.original_input)?;
    Ok(dict.into_any().unbind())
}

fn content_item_to_py(py: Python<'_>, item: &ContentItem) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    match item {
        ContentItem::Text {
            text,
            annotations,
            meta,
        } => {
            dict.set_item("type", "text")?;
            dict.set_item("text", text)?;
            if let Some(value) = annotations {
                dict.set_item("annotations", serde_value_to_py(py, value.clone())?)?;
            }
            if let Some(value) = meta {
                dict.set_item("_meta", serde_value_to_py(py, value.clone())?)?;
            }
        }
        ContentItem::Image {
            data,
            mime_type,
            annotations,
            meta,
        } => {
            dict.set_item("type", "image")?;
            dict.set_item("data", data)?;
            dict.set_item("mime_type", mime_type)?;
            if let Some(value) = annotations {
                dict.set_item("annotations", serde_value_to_py(py, value.clone())?)?;
            }
            if let Some(value) = meta {
                dict.set_item("_meta", serde_value_to_py(py, value.clone())?)?;
            }
        }
        ContentItem::Audio {
            data,
            mime_type,
            annotations,
        } => {
            dict.set_item("type", "audio")?;
            dict.set_item("data", data)?;
            dict.set_item("mime_type", mime_type)?;
            if let Some(value) = annotations {
                dict.set_item("annotations", serde_value_to_py(py, value.clone())?)?;
            }
        }
        ContentItem::Resource {
            resource,
            annotations,
            meta,
        } => {
            dict.set_item("type", "resource")?;
            dict.set_item("resource", serde_value_to_py(py, resource.clone())?)?;
            if let Some(value) = annotations {
                dict.set_item("annotations", serde_value_to_py(py, value.clone())?)?;
            }
            if let Some(value) = meta {
                dict.set_item("_meta", serde_value_to_py(py, value.clone())?)?;
            }
        }
        ContentItem::ResourceLink {
            resource,
            annotations,
        } => {
            dict.set_item("type", "resource_link")?;
            dict.set_item("resource", serde_value_to_py(py, resource.clone())?)?;
            if let Some(value) = annotations {
                dict.set_item("annotations", serde_value_to_py(py, value.clone())?)?;
            }
        }
    }
    Ok(dict.into_any().unbind())
}

fn tool_call_result_to_py(py: Python<'_>, result: &ToolCallResult) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    let content = PyList::empty(py);
    for item in &result.content {
        content.append(content_item_to_py(py, item)?)?;
    }
    dict.set_item("content", content)?;
    dict.set_item("is_error", result.is_error)?;
    dict.set_item("data", py.None())?;
    Ok(dict.into_any().unbind())
}

fn health_status_as_str(status: &HealthStatus) -> &'static str {
    match status {
        HealthStatus::Init => "init",
        HealthStatus::Startup => "startup",
        HealthStatus::Ready => "ready",
        HealthStatus::Healthy => "healthy",
        HealthStatus::Degraded => "degraded",
        HealthStatus::CircuitOpen => "circuit_open",
        HealthStatus::HalfOpen => "half_open",
        HealthStatus::Disconnected => "disconnected",
    }
}

fn tool_availability_as_str(status: &ToolAvailability) -> &'static str {
    match status {
        ToolAvailability::Available => "available",
        ToolAvailability::Unavailable => "unavailable",
    }
}

fn tool_status_item_to_py(py: Python<'_>, item: &ToolStatusItem) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("tool_global_name", &item.tool_global_name)?;
    dict.set_item("tool_original_name", &item.tool_original_name)?;
    dict.set_item("status", tool_availability_as_str(&item.status))?;
    Ok(dict.into_any().unbind())
}

fn service_status_to_py(py: Python<'_>, status: &ServiceStatus) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    let tools = PyList::empty(py);
    for tool in &status.tools {
        tools.append(tool_status_item_to_py(py, tool)?)?;
    }

    dict.set_item("service_global_name", &status.service_global_name)?;
    dict.set_item("health_status", health_status_as_str(&status.health_status))?;
    dict.set_item("last_health_check", status.last_health_check)?;
    dict.set_item("connection_attempts", status.connection_attempts)?;
    dict.set_item("max_connection_attempts", status.max_connection_attempts)?;
    dict.set_item("current_error", status.current_error.as_deref())?;
    dict.set_item("tools", tools)?;
    dict.set_item("window_error_rate", status.window_error_rate)?;
    dict.set_item("latency_p95", status.latency_p95)?;
    dict.set_item("latency_p99", status.latency_p99)?;
    dict.set_item("sample_size", status.sample_size)?;
    dict.set_item("next_retry_time", status.next_retry_time)?;
    dict.set_item("hard_deadline", status.hard_deadline)?;
    dict.set_item("lease_deadline", status.lease_deadline)?;
    Ok(dict.into_any().unbind())
}

fn string_list_to_py(py: Python<'_>, values: &[String]) -> PyResult<Py<PyAny>> {
    let list = PyList::empty(py);
    for value in values {
        list.append(value)?;
    }
    Ok(list.into_any().unbind())
}

fn cache_health_report_to_py(py: Python<'_>, report: &CacheHealthReport) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("namespace", &report.namespace)?;
    dict.set_item("backend", &report.backend)?;
    dict.set_item("entities", string_list_to_py(py, &report.entities)?)?;
    dict.set_item("relations", string_list_to_py(py, &report.relations)?)?;
    dict.set_item("states", string_list_to_py(py, &report.states)?)?;
    dict.set_item("events", string_list_to_py(py, &report.events)?)?;
    Ok(dict.into_any().unbind())
}

fn scoped_service_health_to_py(
    py: Python<'_>,
    statuses: &[ScopedServiceHealth],
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for status in statuses {
        dict.set_item(
            &status.service_name,
            health_status_as_str(&status.health_status),
        )?;
    }
    Ok(dict.into_any().unbind())
}

fn session_entity_to_py(py: Python<'_>, session: &SessionEntity) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("session_key", &session.session_key)?;
    dict.set_item("session_id", &session.session_id)?;
    dict.set_item("scope", session_scope_as_str(&session.scope))?;
    dict.set_item("agent_id", session.agent_id.as_deref())?;
    dict.set_item("created_at", session.created_at)?;
    dict.set_item("updated_at", session.updated_at)?;
    dict.set_item("last_active", session.last_active)?;
    dict.set_item("lease_seconds", session.lease_seconds)?;
    dict.set_item("expires_at", session.expires_at)?;
    dict.set_item("version", session.version)?;
    dict.set_item("metadata", serde_value_to_py(py, session.metadata.clone())?)?;
    Ok(dict.into_any().unbind())
}

fn session_status_to_py(py: Python<'_>, status: &SessionStatusState) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("session_key", &status.session_key)?;
    dict.set_item("status", session_status_as_str(&status.status))?;
    dict.set_item("updated_at", status.updated_at)?;
    dict.set_item("version", status.version)?;
    dict.set_item("reason", status.reason.as_deref())?;
    Ok(dict.into_any().unbind())
}

fn session_state_to_py(py: Python<'_>, state: &SessionStateData) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("session_key", &state.session_key)?;
    dict.set_item(
        "values",
        serde_value_to_py(py, serde_json::Value::Object(state.values.clone()))?,
    )?;
    dict.set_item("updated_at", state.updated_at)?;
    dict.set_item("version", state.version)?;
    Ok(dict.into_any().unbind())
}

fn session_context_state_to_py(py: Python<'_>, state: &SessionContextState) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("context_key", &state.context_key)?;
    dict.set_item("active_session_key", state.active_session_key.as_deref())?;
    dict.set_item("auto_session_key", state.auto_session_key.as_deref())?;
    dict.set_item("updated_at", state.updated_at)?;
    dict.set_item("version", state.version)?;
    Ok(dict.into_any().unbind())
}

fn session_service_item_to_py(py: Python<'_>, service: &SessionServiceItem) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("service_global_name", &service.service_global_name)?;
    dict.set_item("service_original_name", &service.service_original_name)?;
    dict.set_item("source_agent", &service.source_agent)?;
    dict.set_item("bound_at", service.bound_at)?;
    Ok(dict.into_any().unbind())
}

fn session_service_relation_to_py(
    py: Python<'_>,
    relation: &SessionServiceRelation,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    let services = PyList::empty(py);
    for service in &relation.services {
        services.append(session_service_item_to_py(py, service)?)?;
    }
    dict.set_item("session_key", &relation.session_key)?;
    dict.set_item("services", services)?;
    dict.set_item("updated_at", relation.updated_at)?;
    dict.set_item("version", relation.version)?;
    Ok(dict.into_any().unbind())
}

fn session_tool_item_to_py(py: Python<'_>, tool: &SessionToolItem) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("service_global_name", &tool.service_global_name)?;
    dict.set_item("tool_global_name", &tool.tool_global_name)?;
    dict.set_item("tool_original_name", &tool.tool_original_name)?;
    Ok(dict.into_any().unbind())
}

fn session_tool_visibility_to_py(
    py: Python<'_>,
    visibility: &SessionToolVisibility,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    let tools = PyList::empty(py);
    for tool in &visibility.tools {
        tools.append(session_tool_item_to_py(py, tool)?)?;
    }
    dict.set_item("session_key", &visibility.session_key)?;
    dict.set_item("mode", "allowlist")?;
    dict.set_item("tools", tools)?;
    dict.set_item("updated_at", visibility.updated_at)?;
    dict.set_item("version", visibility.version)?;
    Ok(dict.into_any().unbind())
}

fn context_tool_visibility_to_py(
    py: Python<'_>,
    visibility: &ContextToolVisibilityState,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    let tools = PyList::empty(py);
    for tool in &visibility.tools {
        tools.append(session_tool_item_to_py(py, tool)?)?;
    }
    dict.set_item("context_key", &visibility.context_key)?;
    dict.set_item("service_global_name", &visibility.service_global_name)?;
    dict.set_item("mode", "allowlist")?;
    dict.set_item("tools", tools)?;
    dict.set_item("updated_at", visibility.updated_at)?;
    dict.set_item("version", visibility.version)?;
    Ok(dict.into_any().unbind())
}

fn tool_preference_state_to_py(py: Python<'_>, state: &ToolPreferenceState) -> PyResult<Py<PyAny>> {
    serde_value_to_py(
        py,
        serde_json::to_value(state).map_err(|err| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Tool preferences conversion failed: {err}"
            ))
        })?,
    )
}

fn tool_transform_rule_to_py(py: Python<'_>, rule: &ToolTransformRule) -> PyResult<Py<PyAny>> {
    serde_value_to_py(
        py,
        serde_json::to_value(rule).map_err(|err| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Tool transform conversion failed: {err}"
            ))
        })?,
    )
}

#[pymethods]
impl PyMCPStore {
    #[staticmethod]
    #[pyo3(signature = (config_path=None))]
    fn setup(config_path: Option<String>) -> PyResult<Self> {
        let inner = MCPStore::setup(config_path.as_deref()).map_err(map_store_err)?;
        Ok(Self { inner })
    }

    #[staticmethod]
    #[pyo3(signature = (config_path=None, source_mode=None, backend=None, redis_url=None, namespace=None))]
    fn setup_with_options(
        config_path: Option<String>,
        source_mode: Option<String>,
        backend: Option<String>,
        redis_url: Option<String>,
        namespace: Option<String>,
    ) -> PyResult<Self> {
        let inner = MCPStore::setup_with_options(StoreOptions {
            config_path,
            source_mode: parse_source_mode(source_mode.as_deref())?,
            backend: parse_backend(backend.as_deref())?,
            redis_url,
            namespace,
        })
        .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn namespace(&self) -> String {
        self.inner.namespace()
    }

    fn current_backend(&self) -> String {
        let backend =
            pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.current_backend());
        backend_as_str(&backend).to_string()
    }

    fn add_service(&self, name: &str, config: &Bound<'_, PyAny>) -> PyResult<()> {
        let config = py_to_server_config(config, "Service config")?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.add_service(name, config))
            .map_err(map_store_err)
    }

    fn add_service_for_agent(
        &self,
        agent_id: &str,
        local_name: &str,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<String> {
        let config = py_to_server_config(config, "Agent service config")?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .add_service_for_agent(agent_id, local_name, config),
            )
            .map_err(map_store_err)
    }

    fn patch_service(&self, name: &str, updates: &Bound<'_, PyAny>) -> PyResult<()> {
        let updates = py_to_serde_value(updates, "Service config patch")?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.patch_service(name, updates))
            .map_err(map_store_err)
    }

    fn update_service(&self, name: &str, config: &Bound<'_, PyAny>) -> PyResult<()> {
        let config = py_to_server_config(config, "Service config update")?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.update_service(name, config))
            .map_err(map_store_err)
    }

    fn remove_service(&self, name: &str) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.remove_service(name))
            .map_err(map_store_err)
    }

    fn connect_service(&self, name: &str) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.connect_service(name))
            .map_err(map_store_err)
    }

    fn disconnect_service(&self, name: &str) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.disconnect_service(name))
            .map_err(map_store_err)
    }

    fn event_history(&self, py: Python<'_>, count: usize) -> PyResult<Vec<Py<PyAny>>> {
        let events =
            pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.event_history(count));
        events.iter().map(|event| event_to_py(py, event)).collect()
    }

    fn event_capability_report(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let report = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.event_capability_report_entry());
        event_capability_report_to_py(py, &report)
    }

    fn restart_service(&self, name: &str) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.restart_service(name))
            .map_err(map_store_err)
    }

    fn load_from_config(&self) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.load_from_config())
            .map_err(map_store_err)
    }

    #[pyo3(signature = (name, spec_url, options=None))]
    fn import_openapi_service(
        &self,
        py: Python<'_>,
        name: &str,
        spec_url: &str,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let options = parse_openapi_import_options(options)?;
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .import_openapi_service_with_options(name, spec_url, options),
            )
            .map_err(map_store_err)?;
        serde_value_to_py(
            py,
            serde_json::to_value(result).map_err(|err| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "OpenAPI import result conversion failed: {err}"
                ))
            })?,
        )
    }

    #[pyo3(signature = (name, spec_url, spec, options=None))]
    fn import_openapi_service_from_spec(
        &self,
        py: Python<'_>,
        name: &str,
        spec_url: &str,
        spec: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let spec = py_to_serde_value(spec, "OpenAPI spec")?;
        let options = parse_openapi_import_options(options)?;
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .import_openapi_service_from_spec_with_options(name, spec_url, spec, options),
            )
            .map_err(map_store_err)?;
        serde_value_to_py(
            py,
            serde_json::to_value(result).map_err(|err| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "OpenAPI import result conversion failed: {err}"
                ))
            })?,
        )
    }

    #[pyo3(signature = (name, spec_url, spec_text, options=None))]
    fn import_openapi_service_from_spec_text(
        &self,
        py: Python<'_>,
        name: &str,
        spec_url: &str,
        spec_text: &str,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let options = parse_openapi_import_options(options)?;
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .import_openapi_service_from_spec_text_with_options(
                        name, spec_url, spec_text, options,
                    ),
            )
            .map_err(map_store_err)?;
        serde_value_to_py(
            py,
            serde_json::to_value(result).map_err(|err| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "OpenAPI import result conversion failed: {err}"
                ))
            })?,
        )
    }

    #[pyo3(signature = (spec_url, options=None))]
    fn bundle_openapi_spec(
        &self,
        py: Python<'_>,
        spec_url: &str,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let options = parse_openapi_bundle_options(options)?;
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .bundle_openapi_spec_with_options(spec_url, options),
            )
            .map_err(map_store_err)?;
        serde_value_to_py(py, result)
    }

    #[pyo3(signature = (spec_url, spec, options=None))]
    fn bundle_openapi_spec_from_spec(
        &self,
        py: Python<'_>,
        spec_url: &str,
        spec: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let options = parse_openapi_bundle_options(options)?;
        let result = if let Ok(spec_text) = spec.extract::<String>() {
            pyo3_async_runtimes::tokio::get_runtime()
                .block_on(
                    self.inner
                        .bundle_openapi_spec_from_text_with_options(spec_url, &spec_text, options),
                )
                .map_err(map_store_err)?
        } else {
            let spec = py_to_serde_value(spec, "OpenAPI spec")?;
            pyo3_async_runtimes::tokio::get_runtime()
                .block_on(
                    self.inner
                        .bundle_openapi_spec_from_value_with_options(spec_url, spec, options),
                )
                .map_err(map_store_err)?
        };
        serde_value_to_py(py, result)
    }

    #[pyo3(signature = (spec_url, options=None))]
    fn bundle_openapi_artifact(
        &self,
        py: Python<'_>,
        spec_url: &str,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let options = parse_openapi_bundle_options(options)?;
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .bundle_openapi_artifact_with_options(spec_url, options),
            )
            .map_err(map_store_err)?;
        serde_value_to_py(
            py,
            serde_json::to_value(result).map_err(|err| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "OpenAPI bundle artifact conversion failed: {err}"
                ))
            })?,
        )
    }

    #[pyo3(signature = (spec_url, spec, options=None))]
    fn bundle_openapi_artifact_from_spec(
        &self,
        py: Python<'_>,
        spec_url: &str,
        spec: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let options = parse_openapi_bundle_options(options)?;
        let result = if let Ok(spec_text) = spec.extract::<String>() {
            pyo3_async_runtimes::tokio::get_runtime()
                .block_on(
                    self.inner.bundle_openapi_artifact_from_text_with_options(
                        spec_url, &spec_text, options,
                    ),
                )
                .map_err(map_store_err)?
        } else {
            let spec = py_to_serde_value(spec, "OpenAPI spec")?;
            pyo3_async_runtimes::tokio::get_runtime()
                .block_on(
                    self.inner
                        .bundle_openapi_artifact_from_value_with_options(spec_url, spec, options),
                )
                .map_err(map_store_err)?
        };
        serde_value_to_py(
            py,
            serde_json::to_value(result).map_err(|err| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "OpenAPI bundle artifact conversion failed: {err}"
                ))
            })?,
        )
    }

    fn get_openapi_import(&self, py: Python<'_>, name: &str) -> PyResult<Option<Py<PyAny>>> {
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_openapi_import(name))
            .map_err(map_store_err)?;
        result
            .map(|result| {
                serde_value_to_py(
                    py,
                    serde_json::to_value(result).map_err(|err| {
                        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                            "OpenAPI import result conversion failed: {err}"
                        ))
                    })?,
                )
            })
            .transpose()
    }

    fn list_openapi_imports(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let imports = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_openapi_imports())
            .map_err(map_store_err)?;
        imports
            .into_iter()
            .map(|result| {
                serde_value_to_py(
                    py,
                    serde_json::to_value(result).map_err(|err| {
                        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                            "OpenAPI import result conversion failed: {err}"
                        ))
                    })?,
                )
            })
            .collect()
    }

    fn last_openapi_import(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.last_openapi_import())
            .map_err(map_store_err)?;
        result
            .map(|result| {
                serde_value_to_py(
                    py,
                    serde_json::to_value(result).map_err(|err| {
                        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                            "OpenAPI import result conversion failed: {err}"
                        ))
                    })?,
                )
            })
            .transpose()
    }

    fn find_service(&self, py: Python<'_>, name: &str) -> PyResult<Option<Py<PyAny>>> {
        let service =
            pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.find_service(name));
        service
            .as_ref()
            .map(|entry| service_entry_to_py(py, entry))
            .transpose()
    }

    fn get_service_config(&self, py: Python<'_>, name: &str) -> PyResult<Option<Py<PyAny>>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_service_config(name))
            .map_err(map_store_err)?;
        config
            .map(|config| serde_value_to_py(py, config))
            .transpose()
    }

    fn list_services(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let services =
            pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.list_services());
        services
            .iter()
            .map(|entry| service_entry_to_py(py, entry))
            .collect()
    }

    fn list_agents(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let agents = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_agents())
            .map_err(map_store_err)?;
        agents
            .into_iter()
            .map(|agent| serde_value_to_py(py, agent))
            .collect()
    }

    fn list_tools(&self, py: Python<'_>, service_name: &str) -> PyResult<Vec<Py<PyAny>>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tools(service_name))
            .map_err(map_store_err)?;
        tools
            .iter()
            .map(|tool| tool_description_to_py(py, tool))
            .collect()
    }

    fn call_tool(
        &self,
        py: Python<'_>,
        service_name: &str,
        tool_name: &str,
        args: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let args = py_to_serde_value(args, "Tool arguments")?;
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.call_tool(service_name, tool_name, args))
            .map_err(map_store_err)?;
        tool_call_result_to_py(py, &result)
    }

    fn set_tool_transform(
        &self,
        py: Python<'_>,
        service_name: &str,
        tool_name: &str,
        transform: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let value = py_to_serde_value(transform, "Tool transform")?;
        let patch: ToolTransformPatch = serde_json::from_value(value).map_err(|err| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "Tool transform conversion failed: {err}"
            ))
        })?;
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .set_tool_transform(service_name, tool_name, patch),
            )
            .map_err(map_store_err)?;
        tool_transform_rule_to_py(py, &rule)
    }

    fn get_tool_transform(
        &self,
        py: Python<'_>,
        service_name: &str,
        tool_name: &str,
    ) -> PyResult<Option<Py<PyAny>>> {
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_tool_transform(service_name, tool_name))
            .map_err(map_store_err)?;
        rule.as_ref()
            .map(|rule| tool_transform_rule_to_py(py, rule))
            .transpose()
    }

    fn list_tool_transforms(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let rules = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tool_transforms())
            .map_err(map_store_err)?;
        rules
            .iter()
            .map(|rule| tool_transform_rule_to_py(py, rule))
            .collect()
    }

    fn delete_tool_transform(&self, service_name: &str, tool_name: &str) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.delete_tool_transform(service_name, tool_name))
            .map_err(map_store_err)
    }

    #[pyo3(signature = (session_id, scope=None, agent_id=None, lease_seconds=None, metadata=None))]
    fn create_session(
        &self,
        py: Python<'_>,
        session_id: &str,
        scope: Option<String>,
        agent_id: Option<String>,
        lease_seconds: Option<i64>,
        metadata: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let request = CreateSessionRequest {
            session_id: session_id.to_string(),
            scope: parse_session_scope(scope.as_deref())?,
            agent_id,
            lease_seconds,
            metadata: match metadata {
                Some(value) => py_to_serde_value(value, "Session metadata")?,
                None => serde_json::json!({}),
            },
        };
        let session = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.create_session(request))
            .map_err(map_store_err)?;
        session_entity_to_py(py, &session)
    }

    fn get_session(&self, py: Python<'_>, session_key: &str) -> PyResult<Option<Py<PyAny>>> {
        let session = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_session(session_key))
            .map_err(map_store_err)?;
        session
            .as_ref()
            .map(|session| session_entity_to_py(py, session))
            .transpose()
    }

    #[pyo3(signature = (session_id, scope=None, agent_id=None))]
    fn find_session(
        &self,
        py: Python<'_>,
        session_id: &str,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let session = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.find_session(
                parse_session_scope(scope.as_deref())?,
                agent_id.as_deref(),
                session_id,
            ))
            .map_err(map_store_err)?;
        session
            .as_ref()
            .map(|session| session_entity_to_py(py, session))
            .transpose()
    }

    #[pyo3(signature = (scope=None, agent_id=None))]
    fn list_sessions(
        &self,
        py: Python<'_>,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let scope = match scope {
            Some(value) => Some(parse_session_scope(Some(value.as_str()))?),
            None => None,
        };
        let sessions = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_sessions(scope, agent_id.as_deref()))
            .map_err(map_store_err)?;
        sessions
            .iter()
            .map(|session| session_entity_to_py(py, session))
            .collect()
    }

    fn find_session_by_user_session_id(
        &self,
        py: Python<'_>,
        user_session_id: &str,
    ) -> PyResult<Option<Py<PyAny>>> {
        let session = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.find_session_by_user_session_id(user_session_id))
            .map_err(map_store_err)?;
        session
            .as_ref()
            .map(|session| session_entity_to_py(py, session))
            .transpose()
    }

    fn update_session_metadata(
        &self,
        py: Python<'_>,
        session_key: &str,
        metadata: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let metadata = py_to_serde_value(metadata, "Session metadata")?;
        let session = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.update_session_metadata(session_key, metadata))
            .map_err(map_store_err)?;
        session_entity_to_py(py, &session)
    }

    fn get_session_status(&self, py: Python<'_>, session_key: &str) -> PyResult<Option<Py<PyAny>>> {
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_session_status(session_key))
            .map_err(map_store_err)?;
        status
            .as_ref()
            .map(|status| session_status_to_py(py, status))
            .transpose()
    }

    #[pyo3(signature = (session_key, reason=None))]
    fn close_session(
        &self,
        py: Python<'_>,
        session_key: &str,
        reason: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.close_session(session_key, reason))
            .map_err(map_store_err)?;
        session_status_to_py(py, &status)
    }

    fn extend_session(
        &self,
        py: Python<'_>,
        session_key: &str,
        lease_seconds: i64,
    ) -> PyResult<Py<PyAny>> {
        let session = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.extend_session(session_key, lease_seconds))
            .map_err(map_store_err)?;
        session_entity_to_py(py, &session)
    }

    #[pyo3(signature = (session_key, lease_seconds, max_attempts=3, delay_millis=0))]
    fn extend_session_with_retry(
        &self,
        py: Python<'_>,
        session_key: &str,
        lease_seconds: i64,
        max_attempts: usize,
        delay_millis: u64,
    ) -> PyResult<Py<PyAny>> {
        let policy = SessionRetryPolicy::new(max_attempts).delay_millis(delay_millis);
        let session = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .extend_session_with_retry(session_key, lease_seconds, policy),
            )
            .map_err(map_store_err)?;
        session_entity_to_py(py, &session)
    }

    fn bind_service_to_session(
        &self,
        py: Python<'_>,
        session_key: &str,
        service_name: &str,
    ) -> PyResult<Py<PyAny>> {
        let relation = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .bind_service_to_session(session_key, service_name),
            )
            .map_err(map_store_err)?;
        session_service_relation_to_py(py, &relation)
    }

    #[pyo3(signature = (session_key, service_name, max_attempts=3, delay_millis=0))]
    fn bind_service_to_session_with_retry(
        &self,
        py: Python<'_>,
        session_key: &str,
        service_name: &str,
        max_attempts: usize,
        delay_millis: u64,
    ) -> PyResult<Py<PyAny>> {
        let policy = SessionRetryPolicy::new(max_attempts).delay_millis(delay_millis);
        let relation = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.bind_service_to_session_with_retry(
                session_key,
                service_name,
                policy,
            ))
            .map_err(map_store_err)?;
        session_service_relation_to_py(py, &relation)
    }

    fn unbind_service_from_session(
        &self,
        py: Python<'_>,
        session_key: &str,
        service_name: &str,
    ) -> PyResult<Py<PyAny>> {
        let relation = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .unbind_service_from_session(session_key, service_name),
            )
            .map_err(map_store_err)?;
        session_service_relation_to_py(py, &relation)
    }

    #[pyo3(signature = (session_key, service_name, max_attempts=3, delay_millis=0))]
    fn unbind_service_from_session_with_retry(
        &self,
        py: Python<'_>,
        session_key: &str,
        service_name: &str,
        max_attempts: usize,
        delay_millis: u64,
    ) -> PyResult<Py<PyAny>> {
        let policy = SessionRetryPolicy::new(max_attempts).delay_millis(delay_millis);
        let relation = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.unbind_service_from_session_with_retry(
                session_key,
                service_name,
                policy,
            ))
            .map_err(map_store_err)?;
        session_service_relation_to_py(py, &relation)
    }

    fn list_session_services(&self, py: Python<'_>, session_key: &str) -> PyResult<Vec<Py<PyAny>>> {
        let services = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_session_services(session_key))
            .map_err(map_store_err)?;
        services
            .iter()
            .map(|service| session_service_item_to_py(py, service))
            .collect()
    }

    fn set_session_tool_visibility(
        &self,
        py: Python<'_>,
        session_key: &str,
        selections: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let values = py_to_serde_value(selections, "Session tool selections")?;
        let selections: Vec<SessionToolSelection> =
            serde_json::from_value(values).map_err(|err| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "Session tool selections conversion failed: {err}"
                ))
            })?;
        let visibility = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .set_session_tool_visibility(session_key, selections),
            )
            .map_err(map_store_err)?;
        session_tool_visibility_to_py(py, &visibility)
    }

    fn list_session_tools(&self, py: Python<'_>, session_key: &str) -> PyResult<Vec<Py<PyAny>>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_session_tools(session_key))
            .map_err(map_store_err)?;
        tools
            .iter()
            .map(|tool| session_tool_item_to_py(py, tool))
            .collect()
    }

    #[pyo3(signature = (agent_id, service_name))]
    fn get_context_tool_visibility(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: &str,
    ) -> PyResult<Py<PyAny>> {
        let visibility = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .get_context_tool_visibility(agent_id.as_deref(), service_name),
            )
            .map_err(map_store_err)?;
        match visibility {
            Some(visibility) => context_tool_visibility_to_py(py, &visibility),
            None => Ok(py.None()),
        }
    }

    #[pyo3(signature = (agent_id, service_name, tool_names))]
    fn set_context_tool_visibility(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: &str,
        tool_names: Vec<String>,
    ) -> PyResult<Py<PyAny>> {
        let visibility = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.set_context_tool_visibility(
                agent_id.as_deref(),
                service_name,
                tool_names,
            ))
            .map_err(map_store_err)?;
        context_tool_visibility_to_py(py, &visibility)
    }

    #[pyo3(signature = (agent_id, service_name))]
    fn clear_context_tool_visibility(
        &self,
        agent_id: Option<String>,
        service_name: &str,
    ) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .clear_context_tool_visibility(agent_id.as_deref(), service_name),
            )
            .map_err(map_store_err)
    }

    #[pyo3(signature = (agent_id, service_name, tool_name))]
    fn get_tool_preferences(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: &str,
        tool_name: &str,
    ) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .get_tool_preferences(agent_id.as_deref(), service_name, tool_name),
            )
            .map_err(map_store_err)?;
        match state {
            Some(state) => tool_preference_state_to_py(py, &state),
            None => Ok(py.None()),
        }
    }

    #[pyo3(signature = (agent_id, service_name, tool_name, key, default_value=None))]
    fn get_tool_preference(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: &str,
        tool_name: &str,
        key: &str,
        default_value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let value = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_tool_preference(
                agent_id.as_deref(),
                service_name,
                tool_name,
                key,
            ))
            .map_err(map_store_err)?;
        match value {
            Some(value) => serde_value_to_py(py, value),
            None => match default_value {
                Some(value) => Ok(value.clone().unbind()),
                None => Ok(py.None()),
            },
        }
    }

    #[pyo3(signature = (agent_id, service_name, tool_name, key, value))]
    fn set_tool_preference(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: &str,
        tool_name: &str,
        key: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let value = py_to_serde_value(value, "Tool preference value")?;
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.set_tool_preference(
                agent_id.as_deref(),
                service_name,
                tool_name,
                key,
                value,
            ))
            .map_err(map_store_err)?;
        tool_preference_state_to_py(py, &state)
    }

    #[pyo3(signature = (agent_id, service_name, tool_name, key))]
    fn clear_tool_preference(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: &str,
        tool_name: &str,
        key: &str,
    ) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.clear_tool_preference(
                agent_id.as_deref(),
                service_name,
                tool_name,
                key,
            ))
            .map_err(map_store_err)?;
        match state {
            Some(state) => tool_preference_state_to_py(py, &state),
            None => Ok(py.None()),
        }
    }

    fn get_session_state_value(
        &self,
        py: Python<'_>,
        session_key: &str,
        key: &str,
    ) -> PyResult<Py<PyAny>> {
        let value = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_session_state_value(session_key, key))
            .map_err(map_store_err)?;
        serde_value_to_py(py, value.unwrap_or(serde_json::Value::Null))
    }

    fn list_session_state(&self, py: Python<'_>, session_key: &str) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_session_state(session_key))
            .map_err(map_store_err)?;
        session_state_to_py(py, &state)
    }

    fn set_session_state(
        &self,
        py: Python<'_>,
        session_key: &str,
        key: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let value = py_to_serde_value(value, "Session state value")?;
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.set_session_state(session_key, key, value))
            .map_err(map_store_err)?;
        session_state_to_py(py, &state)
    }

    #[pyo3(signature = (session_key, key, value, max_attempts=3, delay_millis=0))]
    fn set_session_state_with_retry(
        &self,
        py: Python<'_>,
        session_key: &str,
        key: &str,
        value: &Bound<'_, PyAny>,
        max_attempts: usize,
        delay_millis: u64,
    ) -> PyResult<Py<PyAny>> {
        let value = py_to_serde_value(value, "Session state value")?;
        let policy = SessionRetryPolicy::new(max_attempts).delay_millis(delay_millis);
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .set_session_state_with_retry(session_key, key, value, policy),
            )
            .map_err(map_store_err)?;
        session_state_to_py(py, &state)
    }

    fn delete_session_state(
        &self,
        py: Python<'_>,
        session_key: &str,
        key: &str,
    ) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.delete_session_state(session_key, key))
            .map_err(map_store_err)?;
        session_state_to_py(py, &state)
    }

    #[pyo3(signature = (session_key, key, max_attempts=3, delay_millis=0))]
    fn delete_session_state_with_retry(
        &self,
        py: Python<'_>,
        session_key: &str,
        key: &str,
        max_attempts: usize,
        delay_millis: u64,
    ) -> PyResult<Py<PyAny>> {
        let policy = SessionRetryPolicy::new(max_attempts).delay_millis(delay_millis);
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .delete_session_state_with_retry(session_key, key, policy),
            )
            .map_err(map_store_err)?;
        session_state_to_py(py, &state)
    }

    fn clear_session_state(&self, py: Python<'_>, session_key: &str) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.clear_session_state(session_key))
            .map_err(map_store_err)?;
        session_state_to_py(py, &state)
    }

    #[pyo3(signature = (scope=None, agent_id=None))]
    fn get_session_context_state(
        &self,
        py: Python<'_>,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_session_context_state(
                parse_session_scope(scope.as_deref())?,
                agent_id.as_deref(),
            ))
            .map_err(map_store_err)?;
        state
            .as_ref()
            .map(|state| session_context_state_to_py(py, state))
            .transpose()
    }

    #[pyo3(signature = (session_key=None, scope=None, agent_id=None))]
    fn set_active_session_for_context(
        &self,
        py: Python<'_>,
        session_key: Option<String>,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.set_active_session_for_context(
                parse_session_scope(scope.as_deref())?,
                agent_id.as_deref(),
                session_key.as_deref(),
            ))
            .map_err(map_store_err)?;
        session_context_state_to_py(py, &state)
    }

    #[pyo3(signature = (scope=None, agent_id=None))]
    fn get_active_session_for_context(
        &self,
        py: Python<'_>,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let session = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_active_session_for_context(
                parse_session_scope(scope.as_deref())?,
                agent_id.as_deref(),
            ))
            .map_err(map_store_err)?;
        session
            .as_ref()
            .map(|session| session_entity_to_py(py, session))
            .transpose()
    }

    #[pyo3(signature = (session_key, scope=None, agent_id=None))]
    fn enable_auto_session_for_context(
        &self,
        py: Python<'_>,
        session_key: &str,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.enable_auto_session_for_context(
                parse_session_scope(scope.as_deref())?,
                agent_id.as_deref(),
                session_key,
            ))
            .map_err(map_store_err)?;
        session_context_state_to_py(py, &state)
    }

    #[pyo3(signature = (scope=None, agent_id=None))]
    fn disable_auto_session_for_context(
        &self,
        py: Python<'_>,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.disable_auto_session_for_context(
                parse_session_scope(scope.as_deref())?,
                agent_id.as_deref(),
            ))
            .map_err(map_store_err)?;
        session_context_state_to_py(py, &state)
    }

    #[pyo3(signature = (scope=None, agent_id=None))]
    fn is_auto_session_enabled_for_context(
        &self,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<bool> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.is_auto_session_enabled_for_context(
                parse_session_scope(scope.as_deref())?,
                agent_id.as_deref(),
            ))
            .map_err(map_store_err)
    }

    fn list_tools_in_session(&self, py: Python<'_>, session_key: &str) -> PyResult<Vec<Py<PyAny>>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tools_in_session(session_key))
            .map_err(map_store_err)?;
        tools
            .iter()
            .map(|tool| scoped_tool_entry_to_py(py, tool))
            .collect()
    }

    fn call_tool_in_session(
        &self,
        py: Python<'_>,
        session_key: &str,
        tool_name: &str,
        args: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let args = py_to_serde_value(args, "Tool arguments")?;
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .call_tool_in_session(session_key, tool_name, args),
            )
            .map_err(map_store_err)?;
        tool_call_result_to_py(py, &result)
    }

    fn resolve_tool_for_agent(
        &self,
        py: Python<'_>,
        agent_id: &str,
        user_input: &str,
    ) -> PyResult<Py<PyAny>> {
        let resolution = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.resolve_tool_for_agent(agent_id, user_input))
            .map_err(map_store_err)?;
        tool_resolution_to_py(py, &resolution)
    }

    fn resolve_service_name_for_agent(
        &self,
        agent_id: &str,
        service_name: &str,
    ) -> PyResult<String> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .resolve_service_name_for_agent(agent_id, service_name),
            )
            .map_err(map_store_err)
    }

    #[pyo3(signature = (agent_id=None))]
    fn list_services_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let services = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_service_entries_scoped(agent_id.as_deref()))
            .map_err(map_store_err)?;
        services
            .iter()
            .map(|service| scoped_service_entry_to_py(py, service))
            .collect()
    }

    #[pyo3(signature = (agent_id=None, service_name=None, filter="all"))]
    fn list_tools_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: Option<String>,
        filter: Option<&str>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let filter = parse_tool_visibility_filter(filter)?;
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tool_entries_scoped_with_filter(
                agent_id.as_deref(),
                service_name.as_deref(),
                filter,
            ))
            .map_err(map_store_err)?;
        tools
            .iter()
            .map(|tool| scoped_tool_entry_to_py(py, tool))
            .collect()
    }

    #[pyo3(signature = (agent_id=None, service_name=None, force_refresh=false))]
    fn list_changed_tools_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: Option<String>,
        force_refresh: bool,
    ) -> PyResult<Py<PyAny>> {
        let changes = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_changed_tools_scoped(
                agent_id.as_deref(),
                service_name.as_deref(),
                force_refresh,
            ))
            .map_err(map_store_err)?;
        serde_value_to_py(
            py,
            serde_json::to_value(changes).map_err(|err| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Tool change summary conversion failed: {err}"
                ))
            })?,
        )
    }

    #[pyo3(signature = (agent_id=None))]
    fn check_services_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.check_service_health_scoped(agent_id.as_deref()))
            .map_err(map_store_err)?;
        scoped_service_health_to_py(py, &status)
    }

    #[pyo3(signature = (agent_id, service_name))]
    fn service_status_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: &str,
    ) -> PyResult<Py<PyAny>> {
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .service_status_entry_scoped(agent_id.as_deref(), service_name),
            )
            .map_err(map_store_err)?;
        service_status_to_py(py, &status)
    }

    #[pyo3(signature = (agent_id=None, service_name=None))]
    fn list_resources_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: Option<String>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let resources = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .list_resources_scoped(agent_id.as_deref(), service_name.as_deref()),
            )
            .map_err(map_store_err)?;
        resources
            .into_iter()
            .map(|resource| serde_value_to_py(py, resource))
            .collect()
    }

    #[pyo3(signature = (agent_id=None, service_name=None))]
    fn list_resource_templates_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: Option<String>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let templates = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .list_resource_templates_scoped(agent_id.as_deref(), service_name.as_deref()),
            )
            .map_err(map_store_err)?;
        templates
            .into_iter()
            .map(|template| serde_value_to_py(py, template))
            .collect()
    }

    #[pyo3(signature = (agent_id, uri, service_name=None))]
    fn read_resource_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        uri: &str,
        service_name: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let resource = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.read_resource_scoped(
                agent_id.as_deref(),
                uri,
                service_name.as_deref(),
            ))
            .map_err(map_store_err)?;
        serde_value_to_py(py, resource)
    }

    #[pyo3(signature = (agent_id=None, service_name=None))]
    fn list_prompts_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: Option<String>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let prompts = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .list_prompts_scoped(agent_id.as_deref(), service_name.as_deref()),
            )
            .map_err(map_store_err)?;
        prompts
            .into_iter()
            .map(|prompt| serde_value_to_py(py, prompt))
            .collect()
    }

    #[pyo3(signature = (agent_id, prompt_name, arguments, service_name=None))]
    fn get_prompt_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        prompt_name: &str,
        arguments: &Bound<'_, PyAny>,
        service_name: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let arguments = py_to_serde_value(arguments, "Prompt arguments")?;
        let prompt = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_prompt_scoped(
                agent_id.as_deref(),
                prompt_name,
                arguments,
                service_name.as_deref(),
            ))
            .map_err(map_store_err)?;
        serde_value_to_py(py, prompt)
    }

    fn show_config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.show_config_entry())
            .map_err(map_store_err)?;
        mcp_config_to_py(py, &config)
    }

    #[pyo3(signature = (agent_id=None))]
    fn show_config_scoped(&self, py: Python<'_>, agent_id: Option<String>) -> PyResult<Py<PyAny>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.show_config_scoped(agent_id.as_deref()))
            .map_err(map_store_err)?;
        serde_value_to_py(py, config)
    }

    fn cache_health_check(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let health = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.cache_health_report())
            .map_err(map_store_err)?;
        cache_health_report_to_py(py, &health)
    }

    fn cache_inspect(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let inspect = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.cache_inspect())
            .map_err(map_store_err)?;
        serde_value_to_py(py, inspect)
    }

    fn reset_cache_request_metrics(&self) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.reset_cache_request_metrics())
            .map_err(map_store_err)
    }

    fn export_sessions_snapshot(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let snapshot = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.export_sessions_snapshot())
            .map_err(map_store_err)?;
        serde_value_to_py(py, snapshot)
    }

    fn import_sessions_snapshot(
        &self,
        py: Python<'_>,
        snapshot: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let snapshot = py_to_serde_value(snapshot, "Session snapshot")?;
        let report = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.import_sessions_snapshot(snapshot))
            .map_err(map_store_err)?;
        serde_value_to_py(
            py,
            serde_json::to_value(report).map_err(|err| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Session import report conversion failed: {err}"
                ))
            })?,
        )
    }

    #[pyo3(signature = (backend, redis_url=None, namespace=None))]
    fn switch_cache_storage(
        &self,
        py: Python<'_>,
        backend: &str,
        redis_url: Option<String>,
        namespace: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let backend = parse_backend(Some(backend))?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("backend is required"))?;
        let snapshot = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .switch_cache_storage(backend, redis_url, namespace),
            )
            .map_err(map_store_err)?;
        serde_value_to_py(
            py,
            serde_json::to_value(snapshot).map_err(|err| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Cache migration snapshot conversion failed: {err}"
                ))
            })?,
        )
    }

    fn reset_config(&self) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.reset_config())
            .map_err(map_store_err)
    }

    fn reset_agent_config(&self, agent_id: &str) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.reset_agent_config(agent_id))
            .map_err(map_store_err)
    }

    #[pyo3(signature = (scope=None))]
    fn reset_mcp_json_scope(&self, scope: Option<String>) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.reset_mcp_json_scope(scope.as_deref()))
            .map_err(map_store_err)
    }

    #[pyo3(signature = (name, timeout_secs=10))]
    fn wait_service_ready(
        &self,
        py: Python<'_>,
        name: &str,
        timeout_secs: u64,
    ) -> PyResult<Py<PyAny>> {
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.wait_service_ready(name, timeout_secs))
            .map_err(map_store_err)?;
        service_status_to_py(py, &status)
    }

    fn __repr__(&self) -> String {
        format!(
            "MCPStore(namespace='{}', backend='{}')",
            self.namespace(),
            self.current_backend()
        )
    }
}
