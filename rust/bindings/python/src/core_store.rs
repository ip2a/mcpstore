//! PyO3 wrapper for the MCPStore Rust runtime surface.

use mcpstore::config::ServerConfig;
use mcpstore::core::store::{BackendKind, MCPStore, SourceMode, StoreOptions};
use mcpstore::StoreError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use pyo3::IntoPyObjectExt;

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

fn to_py_object<T: serde::Serialize>(
    py: Python<'_>,
    value: &T,
    context: &str,
) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(value).map_err(|err| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("{context} serialization failed: {err}"))
    })?;
    json_value_to_py(py, value)
}

fn json_value_to_py(py: Python<'_>, value: serde_json::Value) -> PyResult<Py<PyAny>> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(value) => value.into_py_any(py),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                value.into_py_any(py)
            } else if let Some(value) = value.as_u64() {
                value.into_py_any(py)
            } else if let Some(value) = value.as_f64() {
                value.into_py_any(py)
            } else {
                Err(pyo3::exceptions::PyValueError::new_err(
                    "Unsupported JSON number",
                ))
            }
        }
        serde_json::Value::String(value) => value.into_py_any(py),
        serde_json::Value::Array(values) => {
            let list = PyList::empty(py);
            for value in values {
                list.append(json_value_to_py(py, value)?)?;
            }
            Ok(list.into_any().unbind())
        }
        serde_json::Value::Object(values) => {
            let dict = PyDict::new(py);
            for (key, value) in values {
                dict.set_item(key, json_value_to_py(py, value)?)?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

fn py_to_json_value(value: &Bound<'_, PyAny>, context: &str) -> PyResult<serde_json::Value> {
    if value.is_none() {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(dict) = value.downcast::<PyDict>() {
        let mut object = serde_json::Map::new();
        for (key, value) in dict.iter() {
            let key = key.extract::<String>().map_err(|err| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "{context} dict key must be str: {err}"
                ))
            })?;
            object.insert(key, py_to_json_value(&value, context)?);
        }
        return Ok(serde_json::Value::Object(object));
    }
    if let Ok(list) = value.downcast::<PyList>() {
        let mut values = Vec::with_capacity(list.len());
        for value in list.iter() {
            values.push(py_to_json_value(&value, context)?);
        }
        return Ok(serde_json::Value::Array(values));
    }
    if let Ok(tuple) = value.downcast::<PyTuple>() {
        let mut values = Vec::with_capacity(tuple.len());
        for value in tuple.iter() {
            values.push(py_to_json_value(&value, context)?);
        }
        return Ok(serde_json::Value::Array(values));
    }
    if let Ok(value) = value.extract::<bool>() {
        return Ok(serde_json::Value::Bool(value));
    }
    if let Ok(value) = value.extract::<i64>() {
        return Ok(serde_json::Value::Number(value.into()));
    }
    if let Ok(value) = value.extract::<u64>() {
        return Ok(serde_json::Value::Number(value.into()));
    }
    if let Ok(value) = value.extract::<f64>() {
        return serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "{context} float value is not JSON-compatible"
                ))
            });
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(serde_json::Value::String(value));
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{context} contains unsupported Python value: {}",
        value.get_type().name()?
    )))
}

fn py_to_server_config(value: &Bound<'_, PyAny>, context: &str) -> PyResult<ServerConfig> {
    let value = py_to_json_value(value, context)?;
    serde_json::from_value(value).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("{context} conversion failed: {err}"))
    })
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
        let updates = py_to_json_value(updates, "Service config patch")?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.patch_service(name, updates))
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
            .map(|entry| to_py_object(py, &entry, "Service"))
            .transpose()
    }

    fn list_services(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let services =
            pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.list_services());
        services
            .into_iter()
            .map(|entry| to_py_object(py, &entry, "Service"))
            .collect()
    }

    fn list_tools(&self, py: Python<'_>, service_name: &str) -> PyResult<Vec<Py<PyAny>>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tools(service_name))
            .map_err(map_store_err)?;
        tools
            .into_iter()
            .map(|tool| to_py_object(py, &tool, "Tool"))
            .collect()
    }

    fn call_tool(
        &self,
        py: Python<'_>,
        service_name: &str,
        tool_name: &str,
        args: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let args = py_to_json_value(args, "Tool arguments")?;
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

    fn show_config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.show_config())
            .map_err(map_store_err)?;
        to_py_object(py, &config, "Config")
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
