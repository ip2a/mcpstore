use crate::config::ServerConfig;
use crate::transport::client::McpClient;
use crate::transport::handler::McpStoreClientHandler;
use crate::transport::{Result, TransportError};

use rmcp::transport::child_process::TokioChildProcess;

pub(super) async fn connect(
    name: &str,
    config: &ServerConfig,
    handler: McpStoreClientHandler,
) -> Result<McpClient> {
    let command = config.command.as_deref().ok_or_else(|| {
        TransportError::ConnectionFailed(format!("Service {name} missing command field"))
    })?;

    let mut cmd = tokio::process::Command::new(command);
    cmd.args(&config.args);
    for (key, value) in &config.env {
        cmd.env(key, value);
    }
    if let Some(working_dir) = &config.working_dir {
        cmd.current_dir(working_dir);
    }

    let transport = TokioChildProcess::new(cmd).map_err(|err| {
        TransportError::ConnectionFailed(format!("Failed to spawn child process: {err}"))
    })?;

    rmcp::service::serve_client(handler, transport)
        .await
        .map_err(|err| TransportError::ConnectionFailed(format!("MCP handshake failed: {err}")))
}
