//! PyO3 binding for the MCPStore MCP server runner.
//!
//! Exposes `start_mcp_server` so that the `uvx mcpstore` entry point can start
//! the MCP server directly from the Rust core without requiring an external
//! `mcpstore` CLI binary.

use pyo3::prelude::*;

fn parse_scope(scope: &str) -> mcpstore::mcp_server::Scope {
    match scope.to_lowercase().as_str() {
        "agent" => mcpstore::mcp_server::Scope::Agent,
        "project" => mcpstore::mcp_server::Scope::Project,
        _ => mcpstore::mcp_server::Scope::Store,
    }
}

fn parse_transport(transport: &str) -> mcpstore::mcp_server::McpServerTransport {
    match transport {
        "streamable-http" | "http" => mcpstore::mcp_server::McpServerTransport::StreamableHttp,
        _ => mcpstore::mcp_server::McpServerTransport::Stdio,
    }
}

fn parse_source(source: &str) -> mcpstore::SourceMode {
    match source {
        "db" => mcpstore::SourceMode::Db,
        _ => mcpstore::SourceMode::Local,
    }
}

fn parse_backend(backend: Option<&str>) -> PyResult<Option<mcpstore::CacheStorage>> {
    Ok(match backend {
        None => None,
        Some("memory") => Some(mcpstore::CacheStorage::Memory),
        Some("redis") => Some(mcpstore::CacheStorage::Redis),
        Some("openkeyv_memory") => Some(mcpstore::CacheStorage::OpenKeyvMemory),
        Some("openkeyv_redis") => Some(mcpstore::CacheStorage::OpenKeyvRedis),
        Some(other) => {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Unsupported cache backend: {other}"
            )))
        }
    })
}

fn build_options(
    transport: String,
    scope: String,
    agent: Option<String>,
    service: Option<String>,
    host: String,
    port: u16,
    path: String,
    config_path: Option<String>,
    source: String,
    session_key: Option<String>,
    backend: Option<String>,
    redis_url: Option<String>,
    namespace: Option<String>,
    expose_session_state_tools: bool,
    expose_tool_transform_tools: bool,
    expose_openapi_tools: bool,
    expose_service_tools: bool,
    expose_cache_tools: bool,
    expose_event_tools: bool,
) -> PyResult<mcpstore::mcp_server::McpServerOptions> {
    Ok(mcpstore::mcp_server::McpServerOptions {
        config_path,
        source_mode: parse_source(&source),
        backend: parse_backend(backend.as_deref())?,
        redis_url,
        namespace,
        scope: parse_scope(&scope),
        agent,
        service,
        transport: parse_transport(&transport),
        host,
        port,
        path,
        session_key,
        expose_session_state_tools,
        expose_tool_transform_tools,
        expose_openapi_tools,
        expose_service_tools,
        expose_cache_tools,
        expose_event_tools,
    })
}

/// Start the MCPStore MCP server from Python.
///
/// This is the internal implementation used by `uvx mcpstore`. It does not
/// provide the full CLI surface; for the full CLI use the npm/curl native
/// binary installers.
#[pyfunction]
#[pyo3(signature = (
    transport,
    scope,
    agent=None,
    service=None,
    host="127.0.0.1",
    port=18300,
    path="/mcp",
    config_path=None,
    source="local",
    session_key=None,
    backend=None,
    redis_url=None,
    namespace=None,
    expose_session_state_tools=false,
    expose_tool_transform_tools=false,
    expose_openapi_tools=false,
    expose_service_tools=false,
    expose_cache_tools=false,
    expose_event_tools=false
))]
fn start_mcp_server(
    transport: String,
    scope: String,
    agent: Option<String>,
    service: Option<String>,
    host: &str,
    port: u16,
    path: &str,
    config_path: Option<String>,
    source: &str,
    session_key: Option<String>,
    backend: Option<String>,
    redis_url: Option<String>,
    namespace: Option<String>,
    expose_session_state_tools: bool,
    expose_tool_transform_tools: bool,
    expose_openapi_tools: bool,
    expose_service_tools: bool,
    expose_cache_tools: bool,
    expose_event_tools: bool,
) -> PyResult<()> {
    let options = build_options(
        transport,
        scope,
        agent,
        service,
        host.to_string(),
        port,
        path.to_string(),
        config_path,
        source.to_string(),
        session_key,
        backend,
        redis_url,
        namespace,
        expose_session_state_tools,
        expose_tool_transform_tools,
        expose_openapi_tools,
        expose_service_tools,
        expose_cache_tools,
        expose_event_tools,
    )?;

    pyo3_async_runtimes::tokio::get_runtime()
        .block_on(mcpstore::mcp_server::run(options))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
}

pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(start_mcp_server, m)?)?;
    Ok(())
}
