//! PyO3 wrapper for the MCPStore Rust runtime surface.

use mcpstore_config::ServerConfig;
use mcpstore_core::store::{BackendKind, MCPStore, SourceMode, StoreOptions};
use mcpstore_core::StoreError;
use pyo3::prelude::*;

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

fn to_json_string<T: serde::Serialize>(value: &T, context: &str) -> PyResult<String> {
    serde_json::to_string(value).map_err(|err| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("{context} serialization failed: {err}"))
    })
}

fn parse_json_value(value_json: &str, context: &str) -> PyResult<serde_json::Value> {
    serde_json::from_str(value_json).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("{context} JSON parse failed: {err}"))
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

    fn add_service_json(&self, name: &str, config_json: &str) -> PyResult<()> {
        let config: ServerConfig = serde_json::from_str(config_json).map_err(|err| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "Service config JSON parse failed: {err}"
            ))
        })?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.add_service(name, config))
            .map_err(map_store_err)
    }

    fn add_service_for_agent_json(
        &self,
        agent_id: &str,
        local_name: &str,
        config_json: &str,
    ) -> PyResult<String> {
        let config: ServerConfig = serde_json::from_str(config_json).map_err(|err| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "Service config JSON parse failed: {err}"
            ))
        })?;
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .add_service_for_agent(agent_id, local_name, config),
            )
            .map_err(map_store_err)
    }

    fn patch_service_json(&self, name: &str, updates_json: &str) -> PyResult<()> {
        let updates = parse_json_value(updates_json, "Service config patch")?;
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

    fn find_service_json(&self, name: &str) -> PyResult<Option<String>> {
        let service =
            pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.find_service(name));
        service
            .map(|entry| to_json_string(&entry, "Service"))
            .transpose()
    }

    fn list_services_json(&self) -> PyResult<Vec<String>> {
        let services =
            pyo3_async_runtimes::tokio::get_runtime().block_on(self.inner.list_services());
        services
            .into_iter()
            .map(|entry| to_json_string(&entry, "Service"))
            .collect()
    }

    fn list_tools_json(&self, service_name: &str) -> PyResult<Vec<String>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_tools(service_name))
            .map_err(map_store_err)?;
        tools
            .into_iter()
            .map(|tool| to_json_string(&tool, "Tool"))
            .collect()
    }

    fn call_tool_json(
        &self,
        service_name: &str,
        tool_name: &str,
        args_json: &str,
    ) -> PyResult<String> {
        let args = parse_json_value(args_json, "Tool arguments")?;
        let result = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.call_tool(service_name, tool_name, args))
            .map_err(map_store_err)?;
        to_json_string(&result, "Tool call result")
    }

    fn resolve_tool_for_agent_json(&self, agent_id: &str, user_input: &str) -> PyResult<String> {
        let resolution = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.resolve_tool_for_agent(agent_id, user_input))
            .map_err(map_store_err)?;
        to_json_string(&resolution, "Tool resolution")
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
    fn list_services_scoped_json(&self, agent_id: Option<String>) -> PyResult<Vec<String>> {
        let services = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.list_services_scoped(agent_id.as_deref()))
            .map_err(map_store_err)?;
        services
            .into_iter()
            .map(|service| to_json_string(&service, "Scoped service"))
            .collect()
    }

    #[pyo3(signature = (agent_id=None, service_name=None))]
    fn list_tools_scoped_json(
        &self,
        agent_id: Option<String>,
        service_name: Option<String>,
    ) -> PyResult<Vec<String>> {
        let tools = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(
                self.inner
                    .list_tools_scoped(agent_id.as_deref(), service_name.as_deref()),
            )
            .map_err(map_store_err)?;
        tools
            .into_iter()
            .map(|tool| to_json_string(&tool, "Scoped tool"))
            .collect()
    }

    fn show_config_json(&self) -> PyResult<String> {
        let config = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.show_config())
            .map_err(map_store_err)?;
        to_json_string(&config, "Config")
    }

    fn reset_config(&self) -> PyResult<()> {
        pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.reset_config())
            .map_err(map_store_err)
    }

    #[pyo3(signature = (name, timeout_secs=10))]
    fn wait_service_ready_json(&self, name: &str, timeout_secs: u64) -> PyResult<String> {
        let status = pyo3_async_runtimes::tokio::get_runtime()
            .block_on(self.inner.wait_service_ready(name, timeout_secs))
            .map_err(map_store_err)?;
        to_json_string(&status, "Service ready status")
    }

    fn __repr__(&self) -> String {
        format!(
            "MCPStore(namespace='{}', backend='{}')",
            self.namespace(),
            self.current_backend()
        )
    }
}
