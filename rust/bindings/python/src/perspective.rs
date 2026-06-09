use mcpstore::perspective;
use pyo3::prelude::*;

use crate::py_value::{py_to_serde_value, to_py_object};

fn map_err(err: mcpstore::StoreError) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(err.to_string())
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
    fn parse_agent_scoped(py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        let parsed = perspective::parse_agent_scoped(name).map_err(map_err)?;
        to_py_object(py, &parsed, "Agent scoped name")
    }

    #[staticmethod]
    #[pyo3(signature = (agent_id, name, target="global", strict=false))]
    fn normalize_service_name(
        py: Python<'_>,
        agent_id: &str,
        name: &str,
        target: &str,
        strict: bool,
    ) -> PyResult<Py<PyAny>> {
        let resolution =
            perspective::normalize_service_name(agent_id, name, target, strict).map_err(map_err)?;
        to_py_object(py, &resolution, "Service resolution")
    }

    #[staticmethod]
    #[pyo3(signature = (agent_id, user_input, available_tools, target="canonical", strict=false))]
    fn resolve_tool(
        py: Python<'_>,
        agent_id: &str,
        user_input: &str,
        available_tools: &Bound<'_, PyAny>,
        target: &str,
        strict: bool,
    ) -> PyResult<Py<PyAny>> {
        let available_tools = py_to_serde_value(available_tools, "available_tools")?;
        let available_tools: Vec<perspective::AvailableTool> =
            serde_json::from_value(available_tools).map_err(|err| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "available_tools conversion failed: {err}"
                ))
            })?;
        let resolution =
            perspective::resolve_tool(agent_id, user_input, &available_tools, target, strict)
                .map_err(map_err)?;
        to_py_object(py, &resolution, "Tool resolution")
    }

    #[staticmethod]
    fn generate_service_global_name(original_name: &str, agent_id: &str) -> PyResult<String> {
        perspective::generate_service_global_name(original_name, agent_id).map_err(map_err)
    }

    #[staticmethod]
    fn generate_tool_global_name(
        service_global_name: &str,
        tool_original_name: &str,
    ) -> PyResult<String> {
        perspective::generate_tool_global_name(service_global_name, tool_original_name)
            .map_err(map_err)
    }
}
