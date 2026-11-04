//! Panel role configuration: passed to setup_store, not constructed after.
//!
//! - ControlPanel(): decision + connection maintenance + self-heal (default)
//! - DataPanel(): routes calls per its local config.toml [placement] — default local, "delegate" proxies to the control panel
//!
//! DataPanel identity defaults to the machine hostname; pass panel_id explicitly for a stable identity.

use mcpstore::PanelRole;
use pyo3::prelude::*;

/// Control panel role: mounts the self-healing supervisor at setup (idempotence is carried by the role).
#[pyclass(name = "ControlPanel")]
#[derive(Clone, Default)]
pub struct PyControlPanel;

#[pymethods]
impl PyControlPanel {
    #[new]
    fn new() -> Self {
        Self
    }
}

/// Data panel role: services per its config.toml `[placement]` execute locally (lazy connect on
/// first call), `"delegate"` entries proxy to the control panel through the shared store.
#[pyclass(name = "DataPanel")]
#[derive(Clone)]
pub struct PyDataPanel {
    panel_id: String,
}

#[pymethods]
impl PyDataPanel {
    #[new]
    #[pyo3(signature = (panel_id = None))]
    fn new(panel_id: Option<String>) -> Self {
        Self {
            panel_id: panel_id.unwrap_or_else(PanelRole::default_panel_id),
        }
    }

    #[getter]
    fn panel_id(&self) -> &str {
        &self.panel_id
    }
}

/// Convert a Python panel object (or None = control panel) to PanelRole.
pub(crate) fn parse_panel_role(panel: &Bound<'_, PyAny>) -> PyResult<PanelRole> {
    if panel.is_none() {
        return Ok(PanelRole::ControlPanel);
    }
    if let Ok(role) = panel.extract::<PyRef<'_, PyControlPanel>>() {
        let _ = role;
        return Ok(PanelRole::ControlPanel);
    }
    if let Ok(role) = panel.extract::<PyRef<'_, PyDataPanel>>() {
        return Ok(PanelRole::DataPanel {
            panel_id: role.panel_id.clone(),
        });
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "panel must be ControlPanel() or DataPanel()",
    ))
}
