//! Async PyO3 wrappers for the MCPStore Rust runtime surface.
//!
//! Mirrors the sync classes in `core_store.rs` but returns Python awaitables
//! instead of blocking on the Tokio runtime. Every method that performs I/O
//! is exposed via `pyo3_async_runtimes::tokio::future_into_py`.

use mcpstore::core::store::{MCPStore, StoreOptions};
use mcpstore::{CreateSessionRequest, ScopeContext, Service, Tool};

use pyo3::prelude::*;
use std::sync::Arc;

use crate::core_store::{
    duration_from_seconds, facade_service_target, map_store_err, parse_backend,
    parse_session_scope, parse_source_mode, py_to_add_service_config, py_to_server_config,
    serializable_to_py,
};
use pyo3_async_runtimes::tokio::future_into_py;

#[pyclass(name = "AsyncMCPStore")]
pub struct PyAsyncMCPStore {
    inner: Arc<MCPStore>,
}

#[pyclass(name = "AsyncScopeContext")]
pub struct PyAsyncScopeContext {
    inner: ScopeContext,
}

#[pyclass(name = "AsyncService")]
pub struct PyAsyncService {
    inner: Service,
}

#[pyclass(name = "AsyncTool")]
pub struct PyAsyncTool {
    inner: Tool,
}

impl PyAsyncMCPStore {
    pub(crate) fn from_store(store: Arc<MCPStore>) -> Self {
        Self { inner: store }
    }
}

fn parse_instance_id(value: &str) -> PyResult<mcpstore::InstanceId> {
    value
        .parse()
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e:?}")))
}

#[pymethods]
impl PyAsyncMCPStore {
    #[staticmethod]
    #[pyo3(signature = (config_path=None))]
    fn setup(config_path: Option<String>) -> PyResult<Self> {
        let inner = MCPStore::setup(config_path.as_deref()).map_err(map_store_err)?;
        Ok(Self::from_store(inner))
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
        Ok(Self::from_store(inner))
    }

    fn for_store(&self) -> PyAsyncScopeContext {
        PyAsyncScopeContext {
            inner: self.inner.for_store(),
        }
    }

    fn for_agent(&self, agent_id: &str) -> PyAsyncScopeContext {
        PyAsyncScopeContext {
            inner: self.inner.for_agent(agent_id),
        }
    }

    fn add_service<'a>(
        &self,
        py: Python<'a>,
        service_name: &str,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let config = py_to_server_config(config, "Service config")?;
        let inner = self.inner.clone();
        let service_name = service_name.to_string();
        future_into_py(py, async move {
            inner
                .add_service(&service_name, config)
                .await
                .map_err(map_store_err)?;
            Ok(Python::with_gil(|py| py.None()))
        })
    }

    fn remove_service<'a>(&self, py: Python<'a>, service_name: &str) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        let service_name = service_name.to_string();
        future_into_py(py, async move {
            inner
                .remove_service(&service_name)
                .await
                .map_err(map_store_err)?;
            Ok(Python::with_gil(|py| py.None()))
        })
    }

    fn restart_service<'a>(&self, py: Python<'a>, instance_id: &str) -> PyResult<Bound<'a, PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let inner = self.inner.clone();
        future_into_py(py, async move {
            inner
                .restart_service(instance_id)
                .await
                .map_err(map_store_err)?;
            Ok(Python::with_gil(|py| py.None()))
        })
    }

    fn show_config<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let config = inner.show_config().await.map_err(map_store_err)?;
            Python::with_gil(|py| crate::py_value::serde_value_to_py(py, config))
        })
    }

    fn reset_config<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            inner.reset_config().await.map_err(map_store_err)?;
            Ok(Python::with_gil(|py| py.None()))
        })
    }

    #[pyo3(signature = (session_id, scope=None, agent_id=None, lease_seconds=None, metadata=None))]
    fn create_session<'a>(
        &self,
        py: Python<'a>,
        session_id: &str,
        scope: Option<String>,
        agent_id: Option<String>,
        lease_seconds: Option<i64>,
        metadata: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let request = CreateSessionRequest {
            session_id: session_id.to_string(),
            scope: parse_session_scope(scope.as_deref())?,
            agent_id,
            lease_seconds,
            metadata: match metadata {
                Some(value) => crate::py_value::py_to_serde_value(value, "Session metadata")?,
                None => serde_json::json!({}),
            },
        };
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let session = inner.create_session(request).await.map_err(map_store_err)?;
            Python::with_gil(|py| serializable_to_py(py, &session, "session_entity"))
        })
    }

    fn get_session<'a>(&self, py: Python<'a>, session_key: &str) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        let session_key = session_key.to_string();
        future_into_py(py, async move {
            let session = inner
                .get_session(&session_key)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| match session {
                Some(session) => serializable_to_py(py, &session, "session_entity"),
                None => Ok(py.None()),
            })
        })
    }

    #[pyo3(signature = (session_key, reason=None))]
    fn close_session<'a>(
        &self,
        py: Python<'a>,
        session_key: &str,
        reason: Option<String>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        let session_key = session_key.to_string();
        future_into_py(py, async move {
            let status = inner
                .close_session(&session_key, reason)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| serializable_to_py(py, &status, "session_status"))
        })
    }

    fn bind_service_to_session<'a>(
        &self,
        py: Python<'a>,
        session_key: &str,
        instance_id: &str,
    ) -> PyResult<Bound<'a, PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let inner = self.inner.clone();
        let session_key = session_key.to_string();
        future_into_py(py, async move {
            inner
                .bind_service_to_session(&session_key, instance_id)
                .await
                .map_err(map_store_err)?;
            Ok(Python::with_gil(|py| py.None()))
        })
    }

    fn unbind_service_from_session<'a>(
        &self,
        py: Python<'a>,
        session_key: &str,
        instance_id: &str,
    ) -> PyResult<Bound<'a, PyAny>> {
        let instance_id = parse_instance_id(instance_id)?;
        let inner = self.inner.clone();
        let session_key = session_key.to_string();
        future_into_py(py, async move {
            inner
                .unbind_service_from_session(&session_key, instance_id)
                .await
                .map_err(map_store_err)?;
            Ok(Python::with_gil(|py| py.None()))
        })
    }

    fn list_session_services<'a>(
        &self,
        py: Python<'a>,
        session_key: &str,
    ) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        let session_key = session_key.to_string();
        future_into_py(py, async move {
            let services = inner
                .list_session_services(&session_key)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| serde_list(py, &services))
        })
    }

    fn list_tools_in_session<'a>(
        &self,
        py: Python<'a>,
        session_key: &str,
    ) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        let session_key = session_key.to_string();
        future_into_py(py, async move {
            let tools = inner
                .list_tools_in_session(&session_key)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| serde_list(py, &tools))
        })
    }

    #[pyo3(signature = (session_key, instance_id, tool_name, args=None))]
    fn call_tool_in_session<'a>(
        &self,
        py: Python<'a>,
        session_key: &str,
        instance_id: &str,
        tool_name: &str,
        args: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let args = args
            .map(|value| crate::py_value::py_to_serde_value(value, "Tool arguments"))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));
        let instance_id = parse_instance_id(instance_id)?;
        let inner = self.inner.clone();
        let session_key = session_key.to_string();
        let tool_name = tool_name.to_string();
        future_into_py(py, async move {
            let result = inner
                .call_tool_in_session(&session_key, instance_id, &tool_name, args)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| serializable_to_py(py, &result, "tool_call_result"))
        })
    }
}

fn serde_list<T: serde::Serialize>(py: Python<'_>, items: &[T]) -> PyResult<Py<PyAny>> {
    let list = pyo3::types::PyList::new(
        py,
        items.iter().map(|item| {
            serde_json::to_value(item)
                .ok()
                .and_then(|v| crate::py_value::serde_value_to_py(py, v).ok())
                .unwrap_or_else(|| py.None())
        }),
    )?;
    Ok(list.into_any().unbind())
}

#[pymethods]
impl PyAsyncScopeContext {
    fn scope<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        crate::py_value::serde_value_to_py(
            py,
            serde_json::to_value(self.inner.scope())
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?,
        )
        .map(|v| v.into_bound(py))
    }

    fn show_config<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let config = inner.show_config().await.map_err(map_store_err)?;
            Python::with_gil(|py| crate::py_value::serde_value_to_py(py, config))
        })
    }

    fn reset_config<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let result = inner.reset_config().await.map_err(map_store_err)?;
            Ok::<bool, PyErr>(result)
        })
    }

    fn add_service_config<'a>(
        &self,
        py: Python<'a>,
        service_name: &str,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let config = py_to_server_config(config, "Service config")?;
        let inner = self.inner.clone();
        let service_name = service_name.to_string();
        future_into_py(py, async move {
            let new_inner = inner
                .add_service_config(&service_name, config)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncScopeContext { inner: new_inner }))
        })
    }

    fn add_service<'a>(
        &self,
        py: Python<'a>,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let config = py_to_add_service_config(config)?;
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let new_inner = inner.add_service(config).await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncScopeContext { inner: new_inner }))
        })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None, timeout=10.0))]
    fn wait_service<'a>(
        &self,
        py: Python<'a>,
        service_name: Option<&str>,
        instance_id: Option<&str>,
        timeout: f64,
    ) -> PyResult<Bound<'a, PyAny>> {
        let service_name = service_name.map(str::to_string);
        let instance_id = instance_id.map(str::to_string);
        let duration = duration_from_seconds(timeout)?;
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let target = facade_service_target(service_name.as_deref(), instance_id.as_deref())?;
            let new_inner = inner
                .wait_service(target, duration)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncScopeContext { inner: new_inner }))
        })
    }

    fn list_services<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let services = inner.list_services().await.map_err(map_store_err)?;
            Python::with_gil(|py| {
                let list = pyo3::types::PyList::new(
                    py,
                    services
                        .into_iter()
                        .map(|s| Py::new(py, PyAsyncService { inner: s }).unwrap().into_any()),
                )?;
                Ok::<Py<PyAny>, PyErr>(list.into_any().unbind())
            })
        })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None))]
    fn find_service<'a>(
        &self,
        py: Python<'a>,
        service_name: Option<&str>,
        instance_id: Option<&str>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let service_name = service_name.map(str::to_string);
        let instance_id = instance_id.map(str::to_string);
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let target = facade_service_target(service_name.as_deref(), instance_id.as_deref())?;
            let service = inner.find_service(target).await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncService { inner: service }))
        })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None, updates))]
    fn patch_service<'a>(
        &self,
        py: Python<'a>,
        service_name: Option<&str>,
        instance_id: Option<&str>,
        updates: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let updates = crate::py_value::py_to_serde_value(updates, "Service base config patch")?;
        let service_name = service_name.map(str::to_string);
        let instance_id = instance_id.map(str::to_string);
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let target = facade_service_target(service_name.as_deref(), instance_id.as_deref())?;
            let new_inner = inner
                .patch_service(target, updates)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncScopeContext { inner: new_inner }))
        })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None, config))]
    fn update_service<'a>(
        &self,
        py: Python<'a>,
        service_name: Option<&str>,
        instance_id: Option<&str>,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let config = py_to_server_config(config, "Service base config update")?;
        let service_name = service_name.map(str::to_string);
        let instance_id = instance_id.map(str::to_string);
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let target = facade_service_target(service_name.as_deref(), instance_id.as_deref())?;
            let new_inner = inner
                .update_service(target, config)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncScopeContext { inner: new_inner }))
        })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None))]
    fn remove_service<'a>(
        &self,
        py: Python<'a>,
        service_name: Option<&str>,
        instance_id: Option<&str>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let service_name = service_name.map(str::to_string);
        let instance_id = instance_id.map(str::to_string);
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let target = facade_service_target(service_name.as_deref(), instance_id.as_deref())?;
            let result = inner.remove_service(target).await.map_err(map_store_err)?;
            Ok::<bool, PyErr>(result)
        })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None))]
    fn disconnect_service<'a>(
        &self,
        py: Python<'a>,
        service_name: Option<&str>,
        instance_id: Option<&str>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let service_name = service_name.map(str::to_string);
        let instance_id = instance_id.map(str::to_string);
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let target = facade_service_target(service_name.as_deref(), instance_id.as_deref())?;
            let new_inner = inner
                .disconnect_service(target)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncScopeContext { inner: new_inner }))
        })
    }

    #[pyo3(signature = (*, service_name=None, instance_id=None))]
    fn restart_service<'a>(
        &self,
        py: Python<'a>,
        service_name: Option<&str>,
        instance_id: Option<&str>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let service_name = service_name.map(str::to_string);
        let instance_id = instance_id.map(str::to_string);
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let target = facade_service_target(service_name.as_deref(), instance_id.as_deref())?;
            let new_inner = inner.restart_service(target).await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncScopeContext { inner: new_inner }))
        })
    }

    fn list_tools<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let tools = inner.list_tools().await.map_err(map_store_err)?;
            Python::with_gil(|py| {
                let list = pyo3::types::PyList::new(
                    py,
                    tools
                        .into_iter()
                        .map(|t| Py::new(py, PyAsyncTool { inner: t }).unwrap().into_any()),
                )?;
                Ok::<Py<PyAny>, PyErr>(list.into_any().unbind())
            })
        })
    }

    fn find_tool<'a>(&self, py: Python<'a>, tool_name: &str) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        let tool_name = tool_name.to_string();
        future_into_py(py, async move {
            let tool = inner.find_tool(&tool_name).await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncTool { inner: tool }))
        })
    }

    #[pyo3(signature = (tool_name, args=None))]
    fn call_tool<'a>(
        &self,
        py: Python<'a>,
        tool_name: &str,
        args: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let args = args
            .map(|value| crate::py_value::py_to_serde_value(value, "Tool arguments"))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));
        let inner = self.inner.clone();
        let tool_name = tool_name.to_string();
        future_into_py(py, async move {
            let result = inner
                .call_tool(&tool_name, args)
                .await
                .map_err(map_store_err)?;
            Python::with_gil(|py| serializable_to_py(py, &result, "tool_call_result"))
        })
    }
}

#[pymethods]
impl PyAsyncService {
    fn info<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let info = inner.info().await.map_err(map_store_err)?;
            Python::with_gil(|py| {
                let value = serde_json::to_value(&info).unwrap_or_default();
                crate::py_value::serde_value_to_py(py, value)
            })
        })
    }

    fn state<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let state = inner.state().await.map_err(map_store_err)?;
            Python::with_gil(|py| {
                let value = serde_json::to_value(&state).unwrap_or_default();
                crate::py_value::serde_value_to_py(py, value)
            })
        })
    }

    fn config<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let config = inner.config().await.map_err(map_store_err)?;
            Python::with_gil(|py| crate::py_value::serde_value_to_py(py, config))
        })
    }

    #[pyo3(signature = (timeout=10.0))]
    fn wait_service<'a>(&self, py: Python<'a>, timeout: f64) -> PyResult<Bound<'a, PyAny>> {
        let duration = duration_from_seconds(timeout)?;
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let new_inner = inner.wait_service(duration).await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncService { inner: new_inner }))
        })
    }

    fn disconnect_service<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let new_inner = inner.disconnect_service().await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncService { inner: new_inner }))
        })
    }

    fn restart_service<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let new_inner = inner.restart_service().await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncService { inner: new_inner }))
        })
    }

    fn patch_service<'a>(
        &self,
        py: Python<'a>,
        updates: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let updates = crate::py_value::py_to_serde_value(updates, "Service base config patch")?;
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let new_inner = inner.patch_service(updates).await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncService { inner: new_inner }))
        })
    }

    fn update_service<'a>(
        &self,
        py: Python<'a>,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let config = py_to_server_config(config, "Service base config update")?;
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let new_inner = inner.update_service(config).await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncService { inner: new_inner }))
        })
    }

    fn remove_service<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let result = inner.remove_service().await.map_err(map_store_err)?;
            Ok::<bool, PyErr>(result)
        })
    }

    fn list_tools<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let tools = inner.list_tools().await.map_err(map_store_err)?;
            Python::with_gil(|py| {
                let list = pyo3::types::PyList::new(
                    py,
                    tools
                        .into_iter()
                        .map(|t| Py::new(py, PyAsyncTool { inner: t }).unwrap().into_any()),
                )?;
                Ok::<Py<PyAny>, PyErr>(list.into_any().unbind())
            })
        })
    }

    fn find_tool<'a>(&self, py: Python<'a>, tool_name: &str) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        let tool_name = tool_name.to_string();
        future_into_py(py, async move {
            let tool = inner.find_tool(&tool_name).await.map_err(map_store_err)?;
            Python::with_gil(|py| Py::new(py, PyAsyncTool { inner: tool }))
        })
    }
}

#[pymethods]
impl PyAsyncTool {
    fn info<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let info = inner.info().await.map_err(map_store_err)?;
            Python::with_gil(|py| {
                let value = serde_json::to_value(&info).unwrap_or_default();
                crate::py_value::serde_value_to_py(py, value)
            })
        })
    }

    #[pyo3(signature = (args=None))]
    fn call<'a>(
        &self,
        py: Python<'a>,
        args: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'a, PyAny>> {
        let args = args
            .map(|value| crate::py_value::py_to_serde_value(value, "Tool arguments"))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));
        let inner = self.inner.clone();
        future_into_py(py, async move {
            let result = inner.call(args).await.map_err(map_store_err)?;
            Python::with_gil(|py| serializable_to_py(py, &result, "tool_call_result"))
        })
    }
}
