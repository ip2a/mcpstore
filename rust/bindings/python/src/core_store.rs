//! PyO3 wrapper for the MCPStore Rust runtime surface.

use mcpstore::config::ServerConfig;
use mcpstore::core::perspective::ToolResolution;
use mcpstore::core::store::{
    BackendKind, MCPStore, ScopedServiceEntry, ScopedServiceHealth, ScopedToolEntry, SourceMode,
    StoreOptions,
};
use mcpstore::{
    cache::models::{HealthStatus, ServiceStatus, ToolAvailability, ToolStatusItem},
    ConnectionStatus, ContentItem, Event, ServiceEntry, StoreError, ToolCallResult,
    ToolDescription, ToolInfo,
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

fn backend_as_str(backend: &BackendKind) -> &'static str {
    backend.as_str()
}

fn py_to_server_config(value: &Bound<'_, PyAny>, context: &str) -> PyResult<ServerConfig> {
    let value = py_to_serde_value(value, context)?;
    serde_json::from_value(value).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("{context} conversion failed: {err}"))
    })
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
            .block_on(self.inner.event_capability_report());
        serde_value_to_py(py, report)
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

    #[pyo3(signature = (agent_id=None, service_name=None))]
    fn list_tools_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
        service_name: Option<String>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .list_tool_entries_scoped(agent_id.as_deref(), service_name.as_deref()),
            )
            .map_err(map_store_err)?;
        tools
            .iter()
            .map(|tool| scoped_tool_entry_to_py(py, tool))
            .collect()
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
            .block_on(self.inner.show_config())
            .map_err(map_store_err)?;
        serde_value_to_py(py, config)
    }

    fn cache_health_check(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let health = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.cache_health_check())
            .map_err(map_store_err)?;
        serde_value_to_py(py, health)
    }

    fn cache_inspect(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let inspect = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.cache_inspect())
            .map_err(map_store_err)?;
        serde_value_to_py(py, inspect)
    }

    fn reset_config(&self) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.reset_config())
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
