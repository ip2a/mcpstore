use mcpstore::perspective;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::py_value::py_to_serde_value;

fn map_err(err: mcpstore::StoreError) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(err.to_string())
}

fn agent_scoped_name_to_py(
    py: Python<'_>,
    parsed: &perspective::AgentScopedName,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("agent_id", parsed.agent_id.as_deref())?;
    dict.set_item("local_name", &parsed.local_name)?;
    Ok(dict.into_any().unbind())
}

fn service_resolution_to_py(
    py: Python<'_>,
    resolution: &perspective::ServiceResolution,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("agent_id", &resolution.agent_id)?;
    dict.set_item("local_name", &resolution.local_name)?;
    dict.set_item("global_name", &resolution.global_name)?;
    dict.set_item("resolution_method", &resolution.resolution_method)?;
    dict.set_item("original_input", &resolution.original_input)?;
    Ok(dict.into_any().unbind())
}

fn tool_resolution_to_py(
    py: Python<'_>,
    resolution: &perspective::ToolResolution,
) -> PyResult<Py<PyAny>> {
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
        agent_scoped_name_to_py(py, &parsed)
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
        service_resolution_to_py(py, &resolution)
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
        tool_resolution_to_py(py, &resolution)
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
