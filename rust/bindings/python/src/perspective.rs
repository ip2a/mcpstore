use mcpstore::perspective;
use pyo3::prelude::*;

use crate::py_value::{py_to_serde_value, serde_value_to_py};

fn map_err(err: mcpstore::StoreError) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(err.to_string())
}

fn tool_resolution_to_py(
    py: Python<'_>,
    resolution: &perspective::ToolResolution,
) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(resolution).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("tool resolution conversion failed: {err}"))
    })?;
    serde_value_to_py(py, value)
}

#[pyclass(name = "PerspectiveResolver")]
pub struct PyPerspectiveResolver;

#[pymethods]
impl PyPerspectiveResolver {
    #[new]
    fn new() -> Self {
        Self
    }

    #[staticmethod]
    #[pyo3(signature = (user_input, available_tools))]
    fn resolve_tool(
        py: Python<'_>,
        user_input: &str,
        available_tools: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let available_tools = py_to_serde_value(available_tools, "available_tools")?;
        let available_tools: Vec<perspective::AvailableTool> =
            serde_json::from_value(available_tools).map_err(|err| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "available_tools conversion failed: {err}"
                ))
            })?;
        let resolution =
            perspective::resolve_tool(user_input, &available_tools).map_err(map_err)?;
        tool_resolution_to_py(py, &resolution)
    }
}
