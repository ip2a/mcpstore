//! PyO3 wrapper for the MCPStore Rust runtime surface.

use mcpstore::config::ServerConfig;
use mcpstore::core::store::{BackendKind, MCPStore, SourceMode, StoreOptions};
use mcpstore::{ConnectionStatus, ServiceEntry, StoreError, ToolDescription, ToolInfo};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use crate::py_value::{py_to_serde_value, serde_value_to_py, to_py_object};

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

fn service_entry_to_py(py: Python<'_>, service: &ServiceEntry) -> PyResult<Py<PyAny>> {
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
        events
            .into_iter()
            .map(|event| to_py_object(py, &event, "Event"))
            .collect()
    }

    fn event_capability_report(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let report = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.event_capability_report());
        to_py_object(py, &report, "Event capability report")
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
            .map(|config| to_py_object(py, &config, "Service config"))
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
            .map(|agent| to_py_object(py, &agent, "Agent"))
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
        to_py_object(py, &result, "Tool call result")
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
        to_py_object(py, &resolution, "Tool resolution")
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
            .block_on(self.inner.list_services_scoped(agent_id.as_deref()))
            .map_err(map_store_err)?;
        services
            .into_iter()
            .map(|service| to_py_object(py, &service, "Scoped service"))
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
                    .list_tools_scoped(agent_id.as_deref(), service_name.as_deref()),
            )
            .map_err(map_store_err)?;
        tools
            .into_iter()
            .map(|tool| to_py_object(py, &tool, "Scoped tool"))
            .collect()
    }

    #[pyo3(signature = (agent_id=None))]
    fn check_services_scoped(
        &self,
        py: Python<'_>,
        agent_id: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.check_services_scoped(agent_id.as_deref()))
            .map_err(map_store_err)?;
        to_py_object(py, &status, "Scoped service health")
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
                    .service_status_scoped(agent_id.as_deref(), service_name),
            )
            .map_err(map_store_err)?;
        to_py_object(py, &status, "Scoped service status")
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
            .map(|resource| to_py_object(py, &resource, "Scoped resource"))
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
            .map(|template| to_py_object(py, &template, "Scoped resource template"))
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
        to_py_object(py, &resource, "Scoped resource read")
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
            .map(|prompt| to_py_object(py, &prompt, "Scoped prompt"))
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
        to_py_object(py, &prompt, "Scoped prompt result")
    }

    fn show_config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.show_config())
            .map_err(map_store_err)?;
        to_py_object(py, &config, "Config")
    }

    fn cache_health_check(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let health = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.cache_health_check())
            .map_err(map_store_err)?;
        to_py_object(py, &health, "Cache health")
    }

    fn cache_inspect(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let inspect = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.cache_inspect())
            .map_err(map_store_err)?;
        to_py_object(py, &inspect, "Cache inspect")
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
        to_py_object(py, &status, "Service ready status")
    }

    fn __repr__(&self) -> String {
        format!(
            "MCPStore(namespace='{}', backend='{}')",
            self.namespace(),
            self.current_backend()
        )
    }
}
