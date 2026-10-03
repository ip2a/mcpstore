//! Role components: ControlPanel (self-heal supervision) and DataPanel
//! (on-demand execution), composable from Python code.

use std::sync::Arc;

use mcpstore::{ControlPanel, DataPanel, MCPStore};
use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;

use crate::core_store::{map_store_err, PyMCPStore};

/// 接受 pyclass 或其 Python 门面包装（持有 `_inner`）。
fn unwrap_store(store: &Bound<'_, PyAny>) -> PyResult<Arc<MCPStore>> {
    if let Ok(inner) = store.extract::<PyRef<'_, PyMCPStore>>() {
        return Ok(inner.inner().clone());
    }
    if let Ok(wrapped) = store.getattr("_inner") {
        if let Ok(inner) = wrapped.extract::<PyRef<'_, PyMCPStore>>() {
            return Ok(inner.inner().clone());
        }
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected an MCPStore instance",
    ))
}

#[pyclass(name = "ControlPanel")]
pub struct PyControlPanel {
    store: Arc<MCPStore>,
}

#[pymethods]
impl PyControlPanel {
    /// Decision/scheduling role: attach the self-heal supervisor
    /// (keep_alive reconnect, health state machine). Idempotent.
    #[new]
    fn new(store: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            store: unwrap_store(store)?,
        })
    }

    /// Attach the supervision loop. Idempotent; until attached, no self-heal.
    fn start(&self) -> PyResult<()> {
        ControlPanel::new(self.store.clone())
            .start()
            .map_err(map_store_err)
    }

    /// Detach supervision; established connections keep their current state.
    fn stop<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let panel = ControlPanel::new(self.store.clone());
        future_into_py(py, async move {
            panel.stop().await;
            Ok(())
        })
    }
}

#[pyclass(name = "DataPanel")]
pub struct PyDataPanel {
    store: Arc<MCPStore>,
    panel_id: String,
}

#[pymethods]
impl PyDataPanel {
    /// Execution role: on-demand placement serving, no periodic background work.
    #[new]
    fn new(store: &Bound<'_, PyAny>, panel_id: String) -> PyResult<Self> {
        Ok(Self {
            store: unwrap_store(store)?,
            panel_id,
        })
    }

    fn panel_id(&self) -> &str {
        &self.panel_id
    }

    /// Pull placement-matching services and connect them locally.
    /// Returns the number of services connected.
    fn serve<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let panel = DataPanel::new(self.store.clone(), self.panel_id.clone());
        future_into_py(py, async move {
            panel.serve().await.map_err(map_store_err)
        })
    }

    /// Tear down transports started by this process only.
    fn stop<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let panel = DataPanel::new(self.store.clone(), self.panel_id.clone());
        future_into_py(py, async move {
            panel.stop().await;
            Ok(())
        })
    }
}
