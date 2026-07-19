//! PyO3 wrapper for the MCPStore Rust runtime surface.

use mcpstore::config::ScopeDescriptor;
use mcpstore::config::ServerConfig;
use mcpstore::config_formats::ConfigFormat;
use mcpstore::core::store::{
    BackendKind, CacheHealthReport, EventCapabilityReport, MCPStore, ScopedServiceEntry,
    ScopedToolEntry, SourceMode, StoreOptions,
};
use mcpstore::{
    cache::models::{
        ContextToolVisibilityState, SessionContextState, SessionEntity, SessionScope,
        SessionServiceItem, SessionServiceRelation, SessionStateData, SessionStatus,
        SessionStatusState, SessionToolItem, SessionToolVisibility, ToolPreferenceState,
        ToolTransformRule,
    },
    ContentItem, Event, InstanceId, ScopeRef, ServiceInstance, StoreError, ToolCallResult,
    ToolInfo,
};
use mcpstore::{
    CreateSessionRequest, OpenApiBundleOptions, OpenApiImportOptions, SessionCleanupReport,
    SessionRestartReport, SessionRetryPolicy, SessionToolSelection, ToolTransformPatch,
    ToolVisibilityFilter,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::str::FromStr;

use crate::py_value::{py_to_serde_value, serde_value_to_py};

#[pyclass(name = "MCPStore")]
pub struct PyMCPStore {
    inner: std::sync::Arc<MCPStore>,
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

fn parse_config_format(format: Option<&str>) -> PyResult<ConfigFormat> {
    format
        .unwrap_or("native")
        .parse()
        .map_err(|err: StoreError| pyo3::exceptions::PyValueError::new_err(err.to_string()))
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

fn py_to_scope_ref(value: &Bound<'_, PyAny>) -> PyResult<ScopeRef> {
    let value = py_to_serde_value(value, "scope")?;
    serde_json::from_value(value)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("Invalid scope: {err}")))
}

fn py_to_scope_descriptor(value: &Bound<'_, PyAny>) -> PyResult<ScopeDescriptor> {
    let value = py_to_serde_value(value, "scope descriptor")?;
    serde_json::from_value(value).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("Invalid scope descriptor: {err}"))
    })
}

fn parse_instance_id(value: &str) -> PyResult<InstanceId> {
    InstanceId::from_str(value).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("Invalid instance_id '{value}': {err}"))
    })
}

fn serializable_to_py<T: serde::Serialize>(
    py: Python<'_>,
    value: &T,
    context: &str,
) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(value).map_err(|err| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("{context} conversion failed: {err}"))
    })?;
    serde_value_to_py(py, value)
}

fn tool_info_to_py(py: Python<'_>, tool: &ToolInfo) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("name", &tool.name)?;
    dict.set_item("title", tool.title.as_deref())?;
    dict.set_item("description", &tool.description)?;
    dict.set_item(
        "input_schema",
        serde_value_to_py(py, tool.input_schema.clone())?,
    )?;
    dict.set_item(
        "output_schema",
        serde_value_to_py(
            py,
            tool.output_schema
                .clone()
                .unwrap_or(serde_json::Value::Null),
        )?,
    )?;
    dict.set_item(
        "annotations",
        serde_value_to_py(
            py,
            tool.annotations.clone().unwrap_or(serde_json::Value::Null),
        )?,
    )?;
    dict.set_item(
        "_meta",
        serde_value_to_py(py, tool.meta.clone().unwrap_or(serde_json::Value::Null))?,
    )?;
    Ok(dict.into_any().unbind())
}

fn scoped_tool_entry_to_py(py: Python<'_>, tool: &ScopedToolEntry) -> PyResult<Py<PyAny>> {
    serializable_to_py(py, tool, "Scoped tool")
}

fn service_entry_dict<'py>(
    py: Python<'py>,
    service: &ServiceInstance,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    let tools = PyList::empty(py);
    for tool in &service.tools {
        tools.append(tool_info_to_py(py, tool)?)?;
    }

    dict.set_item("instance_id", service.instance_id.to_string())?;
    dict.set_item("service_name", &service.service_name)?;
    dict.set_item("scope", serializable_to_py(py, &service.scope, "Scope")?)?;
    dict.set_item("transport", &service.transport)?;
    dict.set_item("url", service.url.as_deref())?;
    dict.set_item("command", service.command.as_deref())?;
    dict.set_item("tools", tools)?;
    dict.set_item(
        "effective_config",
        serde_value_to_py(
            py,
            serde_json::Value::Object(service.effective_config.clone()),
        )?,
    )?;
    dict.set_item(
        "config_revision",
        serializable_to_py(py, &service.config_revision, "Config revision")?,
    )?;
    dict.set_item(
        "applied_config_revision",
        serializable_to_py(
            py,
            &service.applied_config_revision,
            "Applied config revision",
        )?,
    )?;
    dict.set_item("restart_required", service.restart_required())?;
    dict.set_item("added_time", service.added_time)?;
    Ok(dict)
}

fn service_entry_to_py(py: Python<'_>, service: &ServiceInstance) -> PyResult<Py<PyAny>> {
    let dict = service_entry_dict(py, service)?;
    Ok(dict.into_any().unbind())
}

fn scoped_service_entry_to_py(py: Python<'_>, entry: &ScopedServiceEntry) -> PyResult<Py<PyAny>> {
    let dict = service_entry_dict(py, &entry.instance)?;
    dict.set_item("tool_count", entry.tool_count)?;
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
            meta,
        } => {
            dict.set_item("type", "audio")?;
            dict.set_item("data", data)?;
            dict.set_item("mime_type", mime_type)?;
            if let Some(value) = annotations {
                dict.set_item("annotations", serde_value_to_py(py, value.clone())?)?;
            }
            if let Some(value) = meta {
                dict.set_item("_meta", serde_value_to_py(py, value.clone())?)?;
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

fn session_cleanup_report_to_py(
    py: Python<'_>,
    report: &SessionCleanupReport,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("refreshed_sessions", report.refreshed_sessions)?;
    dict.set_item("expired_sessions", report.expired_sessions)?;
    dict.set_item("cleared_active_session", report.cleared_active_session)?;
    dict.set_item("cleared_auto_session", report.cleared_auto_session)?;
    Ok(dict.into_any().unbind())
}

fn session_restart_report_to_py(
    py: Python<'_>,
    report: &SessionRestartReport,
) -> PyResult<Py<PyAny>> {
    serializable_to_py(py, report, "Session restart report")
}

fn session_service_item_to_py(py: Python<'_>, service: &SessionServiceItem) -> PyResult<Py<PyAny>> {
    serializable_to_py(py, service, "Session service")
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
    serializable_to_py(py, tool, "Session tool")
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
    dict.set_item("instance_id", visibility.instance_id.to_string())?;
    dict.set_item("service_name", &visibility.service_name)?;
    dict.set_item("scope", serializable_to_py(py, &visibility.scope, "Scope")?)?;
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

    /// Add a service definition. Native configs declare scopes in `_mcpstore.scopes`.
    fn add_service(&self, service_name: &str, config: &Bound<'_, PyAny>) -> PyResult<()> {
        let config = py_to_server_config(config, "Service config")?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.add_service(service_name, config))
            .map_err(map_store_err)
    }

    /// Declare or replace one scope descriptor for an existing service definition.
    fn declare_service_scope(
        &self,
        service_name: &str,
        scope: &Bound<'_, PyAny>,
        descriptor: &Bound<'_, PyAny>,
    ) -> PyResult<String> {
        let scope = py_to_scope_ref(scope)?;
        let descriptor = py_to_scope_descriptor(descriptor)?;
        let instance_id = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .declare_service_scope(service_name, &scope, descriptor),
            )
            .map_err(map_store_err)?;
        Ok(instance_id.to_string())
    }

    /// Remove exactly one service scope and its runtime instance.
    fn remove_service_scope(&self, service_name: &str, scope: &Bound<'_, PyAny>) -> PyResult<()> {
        let scope = py_to_scope_ref(scope)?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.remove_service_scope(service_name, &scope))
            .map_err(map_store_err)
    }

    /// Patch only base MCP fields; `_mcpstore` must be changed through scope APIs.
    fn patch_service(&self, service_name: &str, base_updates: &Bound<'_, PyAny>) -> PyResult<()> {
        let base_updates = py_to_serde_value(base_updates, "Service base config patch")?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.patch_service(service_name, base_updates))
            .map_err(map_store_err)
    }

    /// Replace only base MCP fields while preserving definition metadata and scopes.
    ///
    /// Configs containing `_mcpstore` are rejected. Use `declare_service_scope`
    /// or `remove_service_scope` for scope changes.
    fn update_service(&self, service_name: &str, base_config: &Bound<'_, PyAny>) -> PyResult<()> {
        let base_config = py_to_server_config(base_config, "Service base config update")?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.update_service(service_name, base_config))
            .map_err(map_store_err)
    }

    fn remove_service(&self, service_name: &str) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.remove_service(service_name))
            .map_err(map_store_err)
    }

    fn connect_service(&self, instance_id: &str) -> PyResult<()> {
        let instance_id = parse_instance_id(instance_id)?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.connect_service(instance_id))
            .map_err(map_store_err)
    }

    fn disconnect_service(&self, instance_id: &str) -> PyResult<()> {
        let instance_id = parse_instance_id(instance_id)?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.disconnect_service(instance_id))
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

    fn restart_service(&self, instance_id: &str) -> PyResult<()> {
        let instance_id = parse_instance_id(instance_id)?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.restart_service(instance_id))
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
        let result = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner
                        .import_openapi_service_with_options(name, spec_url, options),
                )
            })
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
        let result = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner.import_openapi_service_from_spec_with_options(
                        name, spec_url, spec, options,
                    ),
                )
            })
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
        let result = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner
                        .import_openapi_service_from_spec_text_with_options(
                            name, spec_url, spec_text, options,
                        ),
                )
            })
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
        let result = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner
                        .bundle_openapi_spec_with_options(spec_url, options),
                )
            })
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
            py.allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner
                        .bundle_openapi_spec_from_text_with_options(spec_url, &spec_text, options),
                )
            })
            .map_err(map_store_err)?
        } else {
            let spec = py_to_serde_value(spec, "OpenAPI spec")?;
            py.allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner
                        .bundle_openapi_spec_from_value_with_options(spec_url, spec, options),
                )
            })
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
        let result = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner
                        .bundle_openapi_artifact_with_options(spec_url, options),
                )
            })
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
            py.allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner.bundle_openapi_artifact_from_text_with_options(
                        spec_url, &spec_text, options,
                    ),
                )
            })
            .map_err(map_store_err)?
        } else {
            let spec = py_to_serde_value(spec, "OpenAPI spec")?;
            py.allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner
                        .bundle_openapi_artifact_from_value_with_options(spec_url, spec, options),
                )
            })
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

    fn find_instance(&self, py: Python<'_>, instance_id: &str) -> PyResult<Option<Py<PyAny>>> {
        let instance_id = parse_instance_id(instance_id)?;
        let service = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.find_instance(instance_id));
        service
            .as_ref()
            .map(|entry| service_entry_to_py(py, entry))
            .transpose()
    }

    fn get_definition_config(
        &self,
        py: Python<'_>,
        service_name: &str,
    ) -> PyResult<Option<Py<PyAny>>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_definition_config(service_name))
            .map_err(map_store_err)?;
        config
            .map(|config| serde_value_to_py(py, config))
            .transpose()
    }

    fn get_effective_config(
        &self,
        py: Python<'_>,
        service_name: &str,
        scope: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let scope = py_to_scope_ref(scope)?;
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_effective_config(service_name, &scope))
            .map_err(map_store_err)?;
        config
            .map(|config| serde_value_to_py(py, config))
            .transpose()
    }

    fn list_instances(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let services =
            pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.list_instances());
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

    fn list_tools(&self, py: Python<'_>, instance_id: &str) -> PyResult<Vec<Py<PyAny>>> {
        let instance_id = parse_instance_id(instance_id)?;
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tool_entries_for_instance_with_filter(
                instance_id,
                ToolVisibilityFilter::Available,
            ))
            .map_err(map_store_err)?;
        tools
            .iter()
            .map(|tool| scoped_tool_entry_to_py(py, tool))
            .collect()
    }

    fn call_tool(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
        args: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let args = py_to_serde_value(args, "Tool arguments")?;
        let result = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.call_tool(
                    instance_id,
                    tool_name,
                    args,
                ))
            })
            .map_err(map_store_err)?;
        tool_call_result_to_py(py, &result)
    }

    fn set_tool_transform(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
        transform: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let value = py_to_serde_value(transform, "Tool transform")?;
        let patch: ToolTransformPatch = serde_json::from_value(value).map_err(|err| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "Tool transform conversion failed: {err}"
            ))
        })?;
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.set_tool_transform(instance_id, tool_name, patch))
            .map_err(map_store_err)?;
        tool_transform_rule_to_py(py, &rule)
    }

    #[pyo3(signature = (instance_id, tool_name, friendly_name=None, description=None, hide_technical_params=true, add_safety_policy=true))]
    fn create_llm_friendly_tool_transform(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
        friendly_name: Option<&str>,
        description: Option<&str>,
        hide_technical_params: bool,
        add_safety_policy: bool,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.create_llm_friendly_tool_transform(
                instance_id,
                tool_name,
                friendly_name,
                description,
                hide_technical_params,
                add_safety_policy,
            ))
            .map_err(map_store_err)?;
        tool_transform_rule_to_py(py, &rule)
    }

    #[pyo3(signature = (instance_id, tool_name, parameter_mapping, new_tool_name=None))]
    fn create_parameter_renamed_tool_transform(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
        parameter_mapping: &Bound<'_, PyAny>,
        new_tool_name: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let value = py_to_serde_value(parameter_mapping, "Parameter mapping")?;
        let mapping = value.as_object().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("Parameter mapping must be a dictionary")
        })?;
        let mut pairs = Vec::with_capacity(mapping.len());
        for (original, renamed) in mapping {
            let Some(renamed) = renamed.as_str() else {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "Parameter mapping values must be strings",
                ));
            };
            pairs.push((original.as_str(), renamed));
        }
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.create_parameter_renamed_tool_transform(
                instance_id,
                tool_name,
                new_tool_name,
                &pairs,
            ))
            .map_err(map_store_err)?;
        tool_transform_rule_to_py(py, &rule)
    }

    #[pyo3(signature = (instance_id, tool_name, validation_rules, new_tool_name=None))]
    fn create_validated_tool_transform(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
        validation_rules: &Bound<'_, PyAny>,
        new_tool_name: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let value = py_to_serde_value(validation_rules, "Validation rules")?;
        let rules = value.as_object().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("Validation rules must be a dictionary")
        })?;
        let pairs: Vec<(&str, serde_json::Value)> = rules
            .iter()
            .map(|(param, schema)| (param.as_str(), schema.clone()))
            .collect();
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.create_validated_tool_transform(
                instance_id,
                tool_name,
                new_tool_name,
                &pairs,
            ))
            .map_err(map_store_err)?;
        tool_transform_rule_to_py(py, &rule)
    }

    fn get_tool_transform(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
    ) -> PyResult<Option<Py<PyAny>>> {
        let instance_id = parse_instance_id(instance_id)?;
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_tool_transform(instance_id, tool_name))
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

    fn delete_tool_transform(&self, instance_id: &str, tool_name: &str) -> PyResult<()> {
        let instance_id = parse_instance_id(instance_id)?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.delete_tool_transform(instance_id, tool_name))
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

    #[pyo3(signature = (scope=None, agent_id=None, reason=None))]
    fn close_sessions(
        &self,
        py: Python<'_>,
        scope: Option<String>,
        agent_id: Option<String>,
        reason: Option<String>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let scope = match scope {
            Some(value) => Some(parse_session_scope(Some(value.as_str()))?),
            None => None,
        };
        let statuses = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .close_sessions(scope, agent_id.as_deref(), reason),
            )
            .map_err(map_store_err)?;
        statuses
            .iter()
            .map(|status| session_status_to_py(py, status))
            .collect()
    }

    #[pyo3(signature = (scope=None, agent_id=None))]
    fn cleanup_sessions(
        &self,
        py: Python<'_>,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let scope = match scope {
            Some(value) => Some(parse_session_scope(Some(value.as_str()))?),
            None => None,
        };
        let report = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.cleanup_sessions(scope, agent_id.as_deref()))
            .map_err(map_store_err)?;
        session_cleanup_report_to_py(py, &report)
    }

    #[pyo3(signature = (scope=None, agent_id=None))]
    fn restart_sessions(
        &self,
        py: Python<'_>,
        scope: Option<String>,
        agent_id: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let scope = match scope {
            Some(value) => Some(parse_session_scope(Some(value.as_str()))?),
            None => None,
        };
        let report = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.restart_sessions(scope, agent_id.as_deref()))
            .map_err(map_store_err)?;
        session_restart_report_to_py(py, &report)
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
        instance_id: &str,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let relation = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.bind_service_to_session(session_key, instance_id))
            .map_err(map_store_err)?;
        session_service_relation_to_py(py, &relation)
    }

    #[pyo3(signature = (session_key, instance_id, max_attempts=3, delay_millis=0))]
    fn bind_service_to_session_with_retry(
        &self,
        py: Python<'_>,
        session_key: &str,
        instance_id: &str,
        max_attempts: usize,
        delay_millis: u64,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let policy = SessionRetryPolicy::new(max_attempts).delay_millis(delay_millis);
        let relation = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.bind_service_to_session_with_retry(
                session_key,
                instance_id,
                policy,
            ))
            .map_err(map_store_err)?;
        session_service_relation_to_py(py, &relation)
    }

    fn unbind_service_from_session(
        &self,
        py: Python<'_>,
        session_key: &str,
        instance_id: &str,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let relation = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .unbind_service_from_session(session_key, instance_id),
            )
            .map_err(map_store_err)?;
        session_service_relation_to_py(py, &relation)
    }

    #[pyo3(signature = (session_key, instance_id, max_attempts=3, delay_millis=0))]
    fn unbind_service_from_session_with_retry(
        &self,
        py: Python<'_>,
        session_key: &str,
        instance_id: &str,
        max_attempts: usize,
        delay_millis: u64,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let policy = SessionRetryPolicy::new(max_attempts).delay_millis(delay_millis);
        let relation = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.unbind_service_from_session_with_retry(
                session_key,
                instance_id,
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

    fn get_context_tool_visibility(
        &self,
        py: Python<'_>,
        instance_id: &str,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let visibility = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_context_tool_visibility(instance_id))
            .map_err(map_store_err)?;
        match visibility {
            Some(visibility) => context_tool_visibility_to_py(py, &visibility),
            None => Ok(py.None()),
        }
    }

    fn set_context_tool_visibility(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_names: Vec<String>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let visibility = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .set_context_tool_visibility(instance_id, tool_names),
            )
            .map_err(map_store_err)?;
        context_tool_visibility_to_py(py, &visibility)
    }

    fn clear_context_tool_visibility(&self, instance_id: &str) -> PyResult<()> {
        let instance_id = parse_instance_id(instance_id)?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.clear_context_tool_visibility(instance_id))
            .map_err(map_store_err)
    }

    fn get_tool_preferences(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_tool_preferences(instance_id, tool_name))
            .map_err(map_store_err)?;
        match state {
            Some(state) => tool_preference_state_to_py(py, &state),
            None => Ok(py.None()),
        }
    }

    #[pyo3(signature = (instance_id, tool_name, key, default_value=None))]
    fn get_tool_preference(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
        key: &str,
        default_value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let value = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_tool_preference(instance_id, tool_name, key))
            .map_err(map_store_err)?;
        match value {
            Some(value) => serde_value_to_py(py, value),
            None => match default_value {
                Some(value) => Ok(value.clone().unbind()),
                None => Ok(py.None()),
            },
        }
    }

    #[pyo3(signature = (instance_id, tool_name, key, value))]
    fn set_tool_preference(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
        key: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let value = py_to_serde_value(value, "Tool preference value")?;
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .set_tool_preference(instance_id, tool_name, key, value),
            )
            .map_err(map_store_err)?;
        tool_preference_state_to_py(py, &state)
    }

    #[pyo3(signature = (instance_id, tool_name, key))]
    fn clear_tool_preference(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
        key: &str,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .clear_tool_preference(instance_id, tool_name, key),
            )
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
        instance_id: &str,
        tool_name: &str,
        args: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let args = py_to_serde_value(args, "Tool arguments")?;
        let result = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.call_tool_in_session(
                    session_key,
                    instance_id,
                    tool_name,
                    args,
                ))
            })
            .map_err(map_store_err)?;
        tool_call_result_to_py(py, &result)
    }

    fn list_resources_in_session(
        &self,
        py: Python<'_>,
        session_key: &str,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let resources = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime()
                    .block_on(self.inner.list_resources_in_session(session_key))
            })
            .map_err(map_store_err)?;
        resources
            .into_iter()
            .map(|resource| serde_value_to_py(py, resource))
            .collect()
    }

    fn list_resource_templates_in_session(
        &self,
        py: Python<'_>,
        session_key: &str,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let templates = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime()
                    .block_on(self.inner.list_resource_templates_in_session(session_key))
            })
            .map_err(map_store_err)?;
        templates
            .into_iter()
            .map(|template| serde_value_to_py(py, template))
            .collect()
    }

    fn read_resource_in_session(
        &self,
        py: Python<'_>,
        session_key: &str,
        uri: &str,
        instance_id: &str,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let resource = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner
                        .read_resource_in_session(session_key, uri, instance_id),
                )
            })
            .map_err(map_store_err)?;
        serde_value_to_py(py, resource)
    }

    fn list_prompts_in_session(
        &self,
        py: Python<'_>,
        session_key: &str,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let prompts = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime()
                    .block_on(self.inner.list_prompts_in_session(session_key))
            })
            .map_err(map_store_err)?;
        prompts
            .into_iter()
            .map(|prompt| serde_value_to_py(py, prompt))
            .collect()
    }

    fn get_prompt_in_session(
        &self,
        py: Python<'_>,
        session_key: &str,
        instance_id: &str,
        prompt_name: &str,
        arguments: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let arguments = py_to_serde_value(arguments, "Prompt arguments")?;
        let prompt = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(
                    self.inner.get_prompt_in_session(
                        session_key,
                        prompt_name,
                        arguments,
                        instance_id,
                    ),
                )
            })
            .map_err(map_store_err)?;
        serde_value_to_py(py, prompt)
    }

    fn list_instances_scoped(
        &self,
        py: Python<'_>,
        scope: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let scope = py_to_scope_ref(scope)?;
        let services = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_service_entries_scoped(&scope))
            .map_err(map_store_err)?;
        services
            .iter()
            .map(|service| scoped_service_entry_to_py(py, service))
            .collect()
    }

    fn instance_info(&self, py: Python<'_>, instance_id: &str) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let service = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.service_info_scoped(instance_id))
            .map_err(map_store_err)?;
        serde_value_to_py(py, service)
    }

    #[pyo3(signature = (instance_id, filter="all"))]
    fn list_tool_entries(
        &self,
        py: Python<'_>,
        instance_id: &str,
        filter: Option<&str>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let instance_id = parse_instance_id(instance_id)?;
        let filter = parse_tool_visibility_filter(filter)?;
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .list_tool_entries_for_instance_with_filter(instance_id, filter),
            )
            .map_err(map_store_err)?;
        tools
            .iter()
            .map(|tool| scoped_tool_entry_to_py(py, tool))
            .collect()
    }

    #[pyo3(signature = (instance_id, force_refresh=false))]
    fn list_changed_tools(
        &self,
        py: Python<'_>,
        instance_id: &str,
        force_refresh: bool,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let changes = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_changed_tools(instance_id, force_refresh))
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

    fn check_instances(&self, py: Python<'_>, instance_ids: Vec<String>) -> PyResult<Py<PyAny>> {
        let instance_ids = instance_ids
            .iter()
            .map(|value| parse_instance_id(value))
            .collect::<PyResult<Vec<_>>>()?;
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.check_instances(&instance_ids))
            .map_err(map_store_err)?;
        serializable_to_py(py, &status, "Instance health")
    }

    fn service_state(&self, py: Python<'_>, instance_id: &str) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.service_state_entry(instance_id))
            .map_err(map_store_err)?;
        serializable_to_py(py, &status, "Service state")
    }

    fn list_resources(&self, py: Python<'_>, instance_id: &str) -> PyResult<Vec<Py<PyAny>>> {
        let instance_id = parse_instance_id(instance_id)?;
        let resources = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_resources_for_instance(instance_id))
            .map_err(map_store_err)?;
        resources
            .into_iter()
            .map(|resource| serde_value_to_py(py, resource))
            .collect()
    }

    fn list_resource_templates(
        &self,
        py: Python<'_>,
        instance_id: &str,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let instance_id = parse_instance_id(instance_id)?;
        let templates = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_resource_templates_for_instance(instance_id))
            .map_err(map_store_err)?;
        templates
            .into_iter()
            .map(|template| serde_value_to_py(py, template))
            .collect()
    }

    fn read_resource(&self, py: Python<'_>, instance_id: &str, uri: &str) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let resource = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime()
                    .block_on(self.inner.read_resource_scoped(instance_id, uri))
            })
            .map_err(map_store_err)?;
        serde_value_to_py(py, resource)
    }

    fn list_prompts(&self, py: Python<'_>, instance_id: &str) -> PyResult<Vec<Py<PyAny>>> {
        let instance_id = parse_instance_id(instance_id)?;
        let prompts = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_prompts_for_instance(instance_id))
            .map_err(map_store_err)?;
        prompts
            .into_iter()
            .map(|prompt| serde_value_to_py(py, prompt))
            .collect()
    }

    fn get_prompt(
        &self,
        py: Python<'_>,
        instance_id: &str,
        prompt_name: &str,
        arguments: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let arguments = py_to_serde_value(arguments, "Prompt arguments")?;
        let prompt = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .get_prompt_scoped(instance_id, prompt_name, arguments),
            )
            .map_err(map_store_err)?;
        serde_value_to_py(py, prompt)
    }

    fn show_config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.show_config())
            .map_err(map_store_err)?;
        serde_value_to_py(py, config)
    }

    fn show_scope_config(&self, py: Python<'_>, scope: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let scope = py_to_scope_ref(scope)?;
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.show_scope_config(&scope))
            .map_err(map_store_err)?;
        serde_value_to_py(py, config)
    }

    #[pyo3(signature = (instance_id, format=None))]
    fn export_instance_config(
        &self,
        py: Python<'_>,
        instance_id: &str,
        format: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let format = parse_config_format(format.as_deref())?;
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.export_instance_config(instance_id, format))
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

    fn reset_scope(&self, scope: &Bound<'_, PyAny>) -> PyResult<()> {
        let scope = py_to_scope_ref(scope)?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.reset_scope(&scope))
            .map_err(map_store_err)
    }

    #[pyo3(signature = (instance_id, timeout_secs=10))]
    fn wait_instance_ready(
        &self,
        py: Python<'_>,
        instance_id: &str,
        timeout_secs: u64,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.wait_instance_ready(instance_id, timeout_secs))
            .map_err(map_store_err)?;
        serializable_to_py(py, &status, "Service state")
    }

    fn __repr__(&self) -> String {
        format!(
            "MCPStore(namespace='{}', backend='{}')",
            self.namespace(),
            self.current_backend()
        )
    }
}
