use mcpstore::perspective;
use pyo3::prelude::*;

fn to_json<T: serde::Serialize>(value: &T, context: &str) -> PyResult<String> {
    serde_json::to_string(value).map_err(|err| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("{context} serialization failed: {err}"))
    })
}

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
    fn parse_agent_scoped_json(name: &str) -> PyResult<String> {
        let parsed = perspective::parse_agent_scoped(name).map_err(map_err)?;
        to_json(&parsed, "Agent scoped name")
    }

    #[staticmethod]
    #[pyo3(signature = (agent_id, name, target="global", strict=false))]
    fn normalize_service_name_json(
        agent_id: &str,
        name: &str,
        target: &str,
        strict: bool,
    ) -> PyResult<String> {
        let resolution =
            perspective::normalize_service_name(agent_id, name, target, strict).map_err(map_err)?;
        to_json(&resolution, "Service resolution")
    }

    #[staticmethod]
    #[pyo3(signature = (agent_id, user_input, available_tools_json, target="canonical", strict=false))]
    fn resolve_tool_json(
        agent_id: &str,
        user_input: &str,
        available_tools_json: &str,
        target: &str,
        strict: bool,
    ) -> PyResult<String> {
        let available_tools: Vec<perspective::AvailableTool> =
            serde_json::from_str(available_tools_json).map_err(|err| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "available_tools JSON parse failed: {err}"
                ))
            })?;
        let resolution =
            perspective::resolve_tool(agent_id, user_input, &available_tools, target, strict)
                .map_err(map_err)?;
        to_json(&resolution, "Tool resolution")
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
