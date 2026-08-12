//! PyO3 wrapper for the MCPStore Rust runtime surface.

use mcpstore::config::ScopeDescriptor;
use mcpstore::config::ServerConfig;
use mcpstore::config_formats::ConfigFormat;
use mcpstore::core::store::{MCPStore, NodeMode, SourceMode, StoreOptions};
use mcpstore::{
    cache::models::SessionScope, InstanceId, McpConfig, ScopeContext, ScopeRef, Service,
    ServiceTarget, StoreError, Tool,
};
use mcpstore::{
    CreateSessionRequest, OpenApiBundleOptions, OpenApiImportOptions, PromptOverridePatch,
    ResourceOverridePatch, ResourceTemplateOverridePatch, SessionRetryPolicy, SessionToolSelection,
    ToolOverridePatch, ToolVisibilityFilter,
};
use pyo3::prelude::*;
use std::str::FromStr;
use std::time::Duration;

use crate::py_value::{py_to_serde_value, serde_value_to_py};

#[pyclass(name = "MCPStore")]
pub struct PyMCPStore {
    inner: std::sync::Arc<MCPStore>,
}

#[pyclass(name = "ScopeContext")]
pub struct PyScopeContext {
    inner: ScopeContext,
}

#[pyclass(name = "Service")]
pub struct PyService {
    inner: Service,
}

#[pyclass(name = "Tool")]
pub struct PyTool {
    inner: Tool,
}

#[pyclass(name = "Prompt")]
pub struct PyPrompt {
    inner: mcpstore::Prompt,
}

#[pyclass(name = "Resource")]
pub struct PyResource {
    inner: mcpstore::Resource,
}

#[pyclass(name = "ResourceTemplate")]
pub struct PyResourceTemplate {
    inner: mcpstore::ResourceTemplate,
}

pub(crate) fn map_store_err(err: StoreError) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(err.to_string())
}

pub(crate) fn parse_openapi_import_options(
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

pub(crate) fn parse_openapi_bundle_options(
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

pub(crate) fn parse_source_mode(source_mode: Option<&str>) -> PyResult<SourceMode> {
    match source_mode {
        Some("db") => Ok(SourceMode::Db),
        Some("local") | None => Ok(SourceMode::Local),
        Some(other) => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported source_mode: {other}"
        ))),
    }
}

pub(crate) fn parse_node_mode(node_mode: Option<&str>) -> PyResult<NodeMode> {
    match node_mode {
        Some("control_plane") | None => Ok(NodeMode::ControlPlane),
        Some("data_plane") => Ok(NodeMode::DataPlane),
        Some(other) => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported node_mode: {other}, expected `control_plane` or `data_plane`"
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

pub(crate) fn parse_session_scope(scope: Option<&str>) -> PyResult<SessionScope> {
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

pub(crate) fn py_to_server_config(
    value: &Bound<'_, PyAny>,
    context: &str,
) -> PyResult<ServerConfig> {
    let value = py_to_serde_value(value, context)?;
    serde_json::from_value(value).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("{context} conversion failed: {err}"))
    })
}

pub(crate) fn py_to_add_service_config(value: &Bound<'_, PyAny>) -> PyResult<McpConfig> {
    if let Ok(value) = value.extract::<String>() {
        let trimmed = value.trim_start();
        let config = if trimmed.starts_with('{') || trimmed.starts_with('[') {
            McpConfig::from_json_str(&value)
        } else {
            McpConfig::from_file(&value)
        };
        return config.map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()));
    }
    if value.hasattr("__fspath__")? {
        let path = value.call_method0("__fspath__")?.extract::<String>()?;
        return McpConfig::from_file(path)
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()));
    }
    let value = py_to_serde_value(value, "service config")?;
    McpConfig::from_input_value(value)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))
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

pub(crate) fn facade_service_target<'a>(
    service_name: Option<&'a str>,
    instance_id: Option<&str>,
) -> PyResult<ServiceTarget<'a>> {
    match (service_name, instance_id) {
        (Some(service_name), None) => Ok(ServiceTarget::ServiceName(service_name)),
        (None, Some(instance_id)) => Ok(ServiceTarget::InstanceId(parse_instance_id(instance_id)?)),
        (None, None) => Err(pyo3::exceptions::PyTypeError::new_err(
            "Specify exactly one of service_name or instance_id",
        )),
        (Some(_), Some(_)) => Err(pyo3::exceptions::PyTypeError::new_err(
            "Specify exactly one of service_name or instance_id",
        )),
    }
}

pub(crate) fn duration_from_seconds(timeout: f64) -> PyResult<Duration> {
    if !timeout.is_finite() || timeout < 0.0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "timeout must be a finite non-negative number of seconds",
        ));
    }
    Ok(Duration::from_secs_f64(timeout))
}

pub(crate) fn serializable_to_py<T: serde::Serialize>(
    py: Python<'_>,
    value: &T,
    context: &str,
) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(value).map_err(|err| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("{context} conversion failed: {err}"))
    })?;
    serde_value_to_py(py, value)
}

#[pymethods]
impl PyScopeContext {
    fn scope(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        serializable_to_py(py, self.inner.scope(), "Scope")
    }

    fn show_config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.show_config())
            .map_err(map_store_err)?;
        serde_value_to_py(py, config)
    }

    fn reset_config(&self) -> PyResult<bool> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.reset_config())
            .map_err(map_store_err)
    }

    fn add_service_config(&self, service_name: &str, config: &Bound<'_, PyAny>) -> PyResult<Self> {
        let config = py_to_server_config(config, "Service config")?;
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.add_service_config(service_name, config))
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn add_service(&self, config: &Bound<'_, PyAny>) -> PyResult<Self> {
        let config = py_to_add_service_config(config)?;
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.add_service(config))
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None, timeout=10.0))]
    fn wait_service(
        &self,
        service_name: Option<&str>,
        instance_id: Option<&str>,
        timeout: f64,
    ) -> PyResult<Self> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.wait_service(
                facade_service_target(service_name, instance_id)?,
                duration_from_seconds(timeout)?,
            ))
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn list_services(&self) -> PyResult<Vec<PyService>> {
        let services = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_services())
            .map_err(map_store_err)?;
        Ok(services
            .into_iter()
            .map(|inner| PyService { inner })
            .collect())
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None))]
    fn find_service(
        &self,
        service_name: Option<&str>,
        instance_id: Option<&str>,
    ) -> PyResult<PyService> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .find_service(facade_service_target(service_name, instance_id)?),
            )
            .map_err(map_store_err)?;
        Ok(PyService { inner })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None, updates))]
    fn patch_service(
        &self,
        service_name: Option<&str>,
        instance_id: Option<&str>,
        updates: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let updates = py_to_serde_value(updates, "Service base config patch")?;
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .patch_service(facade_service_target(service_name, instance_id)?, updates),
            )
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None, config))]
    fn update_service(
        &self,
        service_name: Option<&str>,
        instance_id: Option<&str>,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let config = py_to_server_config(config, "Service base config update")?;
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .update_service(facade_service_target(service_name, instance_id)?, config),
            )
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None))]
    fn remove_service(
        &self,
        service_name: Option<&str>,
        instance_id: Option<&str>,
    ) -> PyResult<bool> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .remove_service(facade_service_target(service_name, instance_id)?),
            )
            .map_err(map_store_err)
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None))]
    fn disconnect_service(
        &self,
        service_name: Option<&str>,
        instance_id: Option<&str>,
    ) -> PyResult<Self> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .disconnect_service(facade_service_target(service_name, instance_id)?),
            )
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None))]
    fn restart_service(
        &self,
        service_name: Option<&str>,
        instance_id: Option<&str>,
    ) -> PyResult<Self> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .restart_service(facade_service_target(service_name, instance_id)?),
            )
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn list_resources(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let resources = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_resources())
            .map_err(map_store_err)?;
        serializable_to_py(py, &resources, "Resources")
    }

    fn list_resource_templates(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let templates = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_resource_templates())
            .map_err(map_store_err)?;
        serializable_to_py(py, &templates, "Resource templates")
    }

    fn list_prompts(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let prompts = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_prompts())
            .map_err(map_store_err)?;
        serializable_to_py(py, &prompts, "Prompts")
    }

    fn list_tools(&self) -> PyResult<Vec<PyTool>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tools())
            .map_err(map_store_err)?;
        Ok(tools.into_iter().map(|inner| PyTool { inner }).collect())
    }

    fn find_tool(&self, tool_name: &str) -> PyResult<PyTool> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.find_tool(tool_name))
            .map_err(map_store_err)?;
        Ok(PyTool { inner })
    }

    #[pyo3(signature = (tool_name, args=None))]
    fn call_tool(
        &self,
        py: Python<'_>,
        tool_name: &str,
        args: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let args = args
            .map(|value| py_to_serde_value(value, "Tool arguments"))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));
        let result = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime()
                    .block_on(self.inner.call_tool(tool_name, args))
            })
            .map_err(map_store_err)?;
        serializable_to_py(py, &result, "tool_call_result")
    }
}

#[pymethods]
impl PyService {
    fn info(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let info = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.info())
            .map_err(map_store_err)?;
        serde_value_to_py(py, info)
    }

    fn state(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.state())
            .map_err(map_store_err)?;
        serializable_to_py(py, &state, "Service state")
    }

    fn config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.config())
            .map_err(map_store_err)?;
        serde_value_to_py(py, config)
    }

    #[pyo3(signature = (timeout=10.0))]
    fn wait_service(&self, timeout: f64) -> PyResult<Self> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.wait_service(duration_from_seconds(timeout)?))
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn disconnect_service(&self) -> PyResult<Self> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.disconnect_service())
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn restart_service(&self) -> PyResult<Self> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.restart_service())
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn patch_service(&self, updates: &Bound<'_, PyAny>) -> PyResult<Self> {
        let updates = py_to_serde_value(updates, "Service base config patch")?;
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.patch_service(updates))
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn update_service(&self, config: &Bound<'_, PyAny>) -> PyResult<Self> {
        let config = py_to_server_config(config, "Service base config update")?;
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.update_service(config))
            .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn remove_service(&self) -> PyResult<bool> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.remove_service())
            .map_err(map_store_err)
    }

    fn list_resources(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let resources = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_resources())
            .map_err(map_store_err)?;
        serializable_to_py(py, &resources, "Resources")
    }

    fn list_resource_templates(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let templates = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_resource_templates())
            .map_err(map_store_err)?;
        serializable_to_py(py, &templates, "Resource templates")
    }

    fn read_resource(&self, py: Python<'_>, uri: &str) -> PyResult<Py<PyAny>> {
        let resource = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.read_resource(uri))
            })
            .map_err(map_store_err)?;
        serde_value_to_py(py, resource)
    }

    fn list_prompts(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let prompts = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_prompts())
            .map_err(map_store_err)?;
        serializable_to_py(py, &prompts, "Prompts")
    }

    #[pyo3(signature = (prompt_name, arguments=None))]
    fn get_prompt(
        &self,
        py: Python<'_>,
        prompt_name: &str,
        arguments: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let arguments = arguments
            .map(|value| py_to_serde_value(value, "Prompt arguments"))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));
        let prompt = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_prompt(prompt_name, arguments))
            .map_err(map_store_err)?;
        serde_value_to_py(py, prompt)
    }

    fn list_tools(&self) -> PyResult<Vec<PyTool>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tools())
            .map_err(map_store_err)?;
        Ok(tools.into_iter().map(|inner| PyTool { inner }).collect())
    }

    fn find_tool(&self, tool_name: &str) -> PyResult<PyTool> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.find_tool(tool_name))
            .map_err(map_store_err)?;
        Ok(PyTool { inner })
    }

    fn find_prompt(&self, prompt_name: &str) -> PyResult<PyPrompt> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.find_prompt(prompt_name))
            .map_err(map_store_err)?;
        Ok(PyPrompt { inner })
    }

    fn find_resource(&self, uri: &str) -> PyResult<PyResource> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.find_resource(uri))
            .map_err(map_store_err)?;
        Ok(PyResource { inner })
    }

    fn find_resource_template(&self, uri_template: &str) -> PyResult<PyResourceTemplate> {
        let inner = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.find_resource_template(uri_template))
            .map_err(map_store_err)?;
        Ok(PyResourceTemplate { inner })
    }

    fn list_tool_overrides(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let values = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tool_overrides())
            .map_err(map_store_err)?;
        serializable_to_py(py, &values, "Tool overrides")
    }

    fn list_prompt_overrides(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let values = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_prompt_overrides())
            .map_err(map_store_err)?;
        serializable_to_py(py, &values, "Prompt overrides")
    }

    fn list_resource_overrides(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let values = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_resource_overrides())
            .map_err(map_store_err)?;
        serializable_to_py(py, &values, "Resource overrides")
    }

    fn list_resource_template_overrides(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let values = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_resource_template_overrides())
            .map_err(map_store_err)?;
        serializable_to_py(py, &values, "Resource template overrides")
    }
}

#[pymethods]
impl PyTool {
    fn info(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let info = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.info())
            .map_err(map_store_err)?;
        serializable_to_py(py, &info, "scoped_tool_entry")
    }

    #[pyo3(signature = (args=None))]
    fn call(&self, py: Python<'_>, args: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        let args = args
            .map(|value| py_to_serde_value(value, "Tool arguments"))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));
        let result = py
            .allow_threads(|| {
                pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.call(args))
            })
            .map_err(map_store_err)?;
        serializable_to_py(py, &result, "tool_call_result")
    }

    fn set_override(&self, py: Python<'_>, patch: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let patch: ToolOverridePatch =
            serde_json::from_value(py_to_serde_value(patch, "Tool override patch")?).map_err(
                |error| pyo3::exceptions::PyValueError::new_err(format!("invalid patch: {error}")),
            )?;
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.set_override(patch))
            .map_err(map_store_err)?;
        serializable_to_py(py, &rule, "Tool override rule")
    }

    fn get_override(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_override())
            .map_err(map_store_err)?;
        serializable_to_py(py, &rule, "Tool override rule")
    }

    fn delete_override(&self) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.delete_override())
            .map_err(map_store_err)
    }

    fn enable(&self) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.enable())
            .map_err(map_store_err)
    }

    fn disable(&self) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.disable())
            .map_err(map_store_err)
    }
}

macro_rules! sync_component_override_methods {
    ($ty:ident, $patch:ty, $label:literal, $extra:item) => {
        #[pymethods]
        impl $ty {
            fn info(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                let value = pyo3_async_runtimes::tokio::get_runtime()
                    .block_on(self.inner.info())
                    .map_err(map_store_err)?;
                serde_value_to_py(py, value)
            }
            fn set_override(&self, py: Python<'_>, patch: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                let patch: $patch = serde_json::from_value(py_to_serde_value(patch, concat!($label, " override patch"))?)
                    .map_err(|error| pyo3::exceptions::PyValueError::new_err(format!("invalid patch: {error}")))?;
                let rule = pyo3_async_runtimes::tokio::get_runtime()
                    .block_on(self.inner.set_override(patch))
                    .map_err(map_store_err)?;
                serializable_to_py(py, &rule, concat!($label, " override rule"))
            }
            fn get_override(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                let rule = pyo3_async_runtimes::tokio::get_runtime()
                    .block_on(self.inner.get_override())
                    .map_err(map_store_err)?;
                serializable_to_py(py, &rule, concat!($label, " override rule"))
            }
            fn delete_override(&self) -> PyResult<()> {
                pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.delete_override()).map_err(map_store_err)
            }
            fn enable(&self) -> PyResult<()> {
                pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.enable()).map_err(map_store_err)
            }
            fn disable(&self) -> PyResult<()> {
                pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.disable()).map_err(map_store_err)
            }
            $extra
        }
    };
}

sync_component_override_methods!(
    PyPrompt,
    PromptOverridePatch,
    "Prompt",
    fn get(&self, py: Python<'_>, args: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let args = py_to_serde_value(args, "Prompt arguments")?;
        let value = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get(args))
            .map_err(map_store_err)?;
        serde_value_to_py(py, value)
    }
);
sync_component_override_methods!(
    PyResource,
    ResourceOverridePatch,
    "Resource",
    fn read(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.read())
            .map_err(map_store_err)?;
        serde_value_to_py(py, value)
    }
);
sync_component_override_methods!(
    PyResourceTemplate,
    ResourceTemplateOverridePatch,
    "Resource template",
    fn no_extra(&self) -> PyResult<()> {
        Ok(())
    }
);

#[pymethods]
impl PyMCPStore {
    #[staticmethod]
    #[pyo3(signature = (config_path=None))]
    fn setup(config_path: Option<String>) -> PyResult<Self> {
        let inner = MCPStore::setup(config_path.as_deref()).map_err(map_store_err)?;
        Ok(Self { inner })
    }

    #[staticmethod]
    #[pyo3(signature = (config_path=None, source_mode=None, store=None, store_config=None, namespace=None, node_mode=None))]
    fn setup_with_options(
        config_path: Option<String>,
        source_mode: Option<String>,
        store: Option<String>,
        store_config: Option<String>,
        namespace: Option<String>,
        node_mode: Option<String>,
    ) -> PyResult<Self> {
        let inner = MCPStore::setup_with_options(StoreOptions {
            config_path,
            source_mode: parse_source_mode(source_mode.as_deref())?,
            node_mode: parse_node_mode(node_mode.as_deref())?,
            store: store
                .map(|name| {
                    let config = store_config
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()
                        .map_err(|error| {
                            pyo3::exceptions::PyValueError::new_err(format!(
                                "Store configuration: {error}"
                            ))
                        })?
                        .unwrap_or_else(|| serde_json::json!({}));
                    Ok::<mcpstore::JsonStoreConfig, PyErr>(mcpstore::JsonStoreConfig::new(
                        name, config,
                    ))
                })
                .transpose()?,
            namespace,
        })
        .map_err(map_store_err)?;
        Ok(Self { inner })
    }

    fn namespace(&self) -> String {
        self.inner.namespace()
    }

    fn current_store(&self) -> String {
        pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.current_store_name())
    }

    fn for_store(&self) -> PyScopeContext {
        PyScopeContext {
            inner: self.inner.for_store(),
        }
    }

    fn for_agent(&self, agent_id: &str) -> PyScopeContext {
        PyScopeContext {
            inner: self.inner.for_agent(agent_id),
        }
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
        events
            .iter()
            .map(|event| serializable_to_py(py, event, "event"))
            .collect()
    }

    fn event_capability_report(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let report = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.event_capability_report_entry());
        serializable_to_py(py, &report, "event_capability_report")
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
            .map(|entry| serializable_to_py(py, entry, "service_entry"))
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
        let services = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_services())
            .map_err(map_store_err)?;
        services
            .iter()
            .map(|service| serializable_to_py(py, service, "service"))
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
            .map(|tool| serializable_to_py(py, tool, "scoped_tool_entry"))
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
        serializable_to_py(py, &result, "tool_call_result")
    }

    fn set_tool_override(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
        transform: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let value = py_to_serde_value(transform, "Tool transform")?;
        let patch: ToolOverridePatch = serde_json::from_value(value).map_err(|err| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "Tool transform conversion failed: {err}"
            ))
        })?;
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.set_tool_override(instance_id, tool_name, patch))
            .map_err(map_store_err)?;
        serializable_to_py(py, &rule, "tool_override_rule")
    }

    #[pyo3(signature = (instance_id, tool_name, friendly_name=None, description=None, hide_technical_params=true, add_safety_policy=true))]
    fn create_llm_friendly_tool_override(
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
            .block_on(self.inner.create_llm_friendly_tool_override(
                instance_id,
                tool_name,
                friendly_name,
                description,
                hide_technical_params,
                add_safety_policy,
            ))
            .map_err(map_store_err)?;
        serializable_to_py(py, &rule, "tool_override_rule")
    }

    #[pyo3(signature = (instance_id, tool_name, parameter_mapping, new_tool_name=None))]
    fn create_parameter_renamed_tool_override(
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
            .block_on(self.inner.create_parameter_renamed_tool_override(
                instance_id,
                tool_name,
                new_tool_name,
                &pairs,
            ))
            .map_err(map_store_err)?;
        serializable_to_py(py, &rule, "tool_override_rule")
    }

    #[pyo3(signature = (instance_id, tool_name, validation_rules, new_tool_name=None))]
    fn create_validated_tool_override(
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
            .block_on(self.inner.create_validated_tool_override(
                instance_id,
                tool_name,
                new_tool_name,
                &pairs,
            ))
            .map_err(map_store_err)?;
        serializable_to_py(py, &rule, "tool_override_rule")
    }

    fn get_tool_override(
        &self,
        py: Python<'_>,
        instance_id: &str,
        tool_name: &str,
    ) -> PyResult<Option<Py<PyAny>>> {
        let instance_id = parse_instance_id(instance_id)?;
        let rule = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_tool_override(instance_id, tool_name))
            .map_err(map_store_err)?;
        rule.as_ref()
            .map(|rule| serializable_to_py(py, rule, "tool_override_rule"))
            .transpose()
    }

    fn list_tool_overrides(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let rules = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tool_overrides())
            .map_err(map_store_err)?;
        rules
            .iter()
            .map(|rule| serializable_to_py(py, rule, "tool_override_rule"))
            .collect()
    }

    fn delete_tool_override(&self, instance_id: &str, tool_name: &str) -> PyResult<()> {
        let instance_id = parse_instance_id(instance_id)?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.delete_tool_override(instance_id, tool_name))
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
        serializable_to_py(py, &session, "session_entity")
    }

    fn get_session(&self, py: Python<'_>, session_key: &str) -> PyResult<Option<Py<PyAny>>> {
        let session = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_session(session_key))
            .map_err(map_store_err)?;
        session
            .as_ref()
            .map(|session| serializable_to_py(py, session, "session_entity"))
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
            .map(|session| serializable_to_py(py, session, "session_entity"))
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
            .map(|session| serializable_to_py(py, session, "session_entity"))
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
            .map(|session| serializable_to_py(py, session, "session_entity"))
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
        serializable_to_py(py, &session, "session_entity")
    }

    fn get_session_status(&self, py: Python<'_>, session_key: &str) -> PyResult<Option<Py<PyAny>>> {
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.get_session_status(session_key))
            .map_err(map_store_err)?;
        status
            .as_ref()
            .map(|status| serializable_to_py(py, status, "session_status"))
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
        serializable_to_py(py, &status, "session_status")
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
            .map(|status| serializable_to_py(py, status, "session_status"))
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
        serializable_to_py(py, &report, "session_cleanup_report")
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
        serializable_to_py(py, &report, "session_restart_report")
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
        serializable_to_py(py, &session, "session_entity")
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
        serializable_to_py(py, &session, "session_entity")
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
        serializable_to_py(py, &relation, "session_service_relation")
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
        serializable_to_py(py, &relation, "session_service_relation")
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
        serializable_to_py(py, &relation, "session_service_relation")
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
        serializable_to_py(py, &relation, "session_service_relation")
    }

    fn list_session_services(&self, py: Python<'_>, session_key: &str) -> PyResult<Vec<Py<PyAny>>> {
        let services = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_session_services(session_key))
            .map_err(map_store_err)?;
        services
            .iter()
            .map(|service| serializable_to_py(py, service, "session_service_item"))
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
        serializable_to_py(py, &visibility, "session_tool_visibility")
    }

    fn list_session_tools(&self, py: Python<'_>, session_key: &str) -> PyResult<Vec<Py<PyAny>>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_session_tools(session_key))
            .map_err(map_store_err)?;
        tools
            .iter()
            .map(|tool| serializable_to_py(py, tool, "session_tool_item"))
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
            Some(visibility) => serializable_to_py(py, &visibility, "context_tool_visibility"),
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
        serializable_to_py(py, &visibility, "context_tool_visibility")
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
            Some(state) => serializable_to_py(py, &state, "tool_preference_state"),
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
        serializable_to_py(py, &state, "tool_preference_state")
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
            Some(state) => serializable_to_py(py, &state, "tool_preference_state"),
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
        serializable_to_py(py, &state, "session_state")
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
        serializable_to_py(py, &state, "session_state")
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
        serializable_to_py(py, &state, "session_state")
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
        serializable_to_py(py, &state, "session_state")
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
        serializable_to_py(py, &state, "session_state")
    }

    fn clear_session_state(&self, py: Python<'_>, session_key: &str) -> PyResult<Py<PyAny>> {
        let state = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.clear_session_state(session_key))
            .map_err(map_store_err)?;
        serializable_to_py(py, &state, "session_state")
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
            .map(|state| serializable_to_py(py, state, "session_context_state"))
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
        serializable_to_py(py, &state, "session_context_state")
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
            .map(|session| serializable_to_py(py, session, "session_entity"))
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
        serializable_to_py(py, &state, "session_context_state")
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
        serializable_to_py(py, &state, "session_context_state")
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
            .map(|tool| serializable_to_py(py, tool, "scoped_tool_entry"))
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
        serializable_to_py(py, &result, "tool_call_result")
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
            .block_on(self.inner.list_services_scoped(&scope))
            .map_err(map_store_err)?;
        services
            .iter()
            .map(|service| serializable_to_py(py, service, "service"))
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
            .map(|tool| serializable_to_py(py, tool, "scoped_tool_entry"))
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

    fn show_session_config(&self, py: Python<'_>, session_key: &str) -> PyResult<Py<PyAny>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.show_session_config(session_key))
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
        serializable_to_py(py, &health, "cache_health_report")
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

    #[pyo3(signature = (store, config=None))]
    fn swap_store(
        &self,
        py: Python<'_>,
        store: &str,
        config: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let config_value = match config {
            Some(value) if !value.is_none() => py_to_serde_value(value, "Store configuration")?,
            _ => serde_json::json!({}),
        };
        let config = mcpstore::JsonStoreConfig::new(store, config_value);
        let snapshot = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.swap_store(&config))
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
            .block_on(
                self.inner
                    .wait_instance_ready(instance_id, Duration::from_secs(timeout_secs)),
            )
            .map_err(map_store_err)?;
        serializable_to_py(py, &status, "Service state")
    }

    fn __repr__(&self) -> String {
        format!(
            "MCPStore(namespace='{}', store='{}')",
            self.namespace(),
            self.current_store()
        )
    }
}
