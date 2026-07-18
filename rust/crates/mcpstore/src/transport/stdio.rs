use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::config::ServerConfig;
use crate::health::supervisor::InstanceSupervisor;
use crate::identity::InstanceId;
use crate::transport::client::McpClient;
use crate::transport::handler::McpStoreClientHandler;
use crate::transport::{Result, TransportError};

use rmcp::transport::async_rw::AsyncRwTransport;
use rmcp::RoleClient;

type StdioTransport =
    AsyncRwTransport<RoleClient, tokio::process::ChildStdout, tokio::process::ChildStdin>;

pub(super) struct StdioProcess {
    exited: Arc<AtomicBool>,
    shutdown_requested: Arc<AtomicBool>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl StdioProcess {
    pub(super) fn is_running(&self) -> bool {
        !self.exited.load(Ordering::Acquire)
    }

    pub(super) async fn shutdown(mut self) {
        self.shutdown_requested.store(true, Ordering::Release);
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }
    }
}

pub(super) async fn connect(
    name: &str,
    config: &ServerConfig,
    handler: McpStoreClientHandler,
    instance_id: InstanceId,
    supervisor: Option<Arc<InstanceSupervisor>>,
) -> Result<(McpClient, StdioProcess)> {
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
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit());

    let mut child = cmd.spawn().map_err(|err| {
        TransportError::ConnectionFailed(format!("Failed to spawn child process: {err}"))
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        TransportError::ConnectionFailed(format!("stdio child for {name} has no stdout"))
    })?;
    let stdin = child.stdin.take().ok_or_else(|| {
        TransportError::ConnectionFailed(format!("stdio child for {name} has no stdin"))
    })?;

    let exited = Arc::new(AtomicBool::new(false));
    let exited_signal = Arc::clone(&exited);
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let shutdown_requested_signal = Arc::clone(&shutdown_requested);
    let (shutdown_sender, mut shutdown_receiver) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::select! {
            result = child.wait() => {
                if let Err(error) = result {
                    tracing::warn!("stdio child wait failed: {error}");
                }
            }
            _ = &mut shutdown_receiver => {
                if let Err(error) = child.kill().await {
                    tracing::debug!("stdio child kill failed during shutdown: {error}");
                }
                let _ = child.wait().await;
            }
        }
        exited_signal.store(true, Ordering::Release);
        if !shutdown_requested_signal.load(Ordering::Acquire) {
            if let Some(supervisor) = supervisor {
                let observed_at = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
                let observation = crate::health::state_machine::HealthObservation {
                    observed_at,
                    kind: crate::health::state_machine::ObservationKind::ProcessExit,
                    succeeded: false,
                    latency_ms: None,
                };
                let _ = supervisor
                    .observe_and_commit(instance_id, observation)
                    .await;
                // Transition persistence and recovery actions are handled by the
                // supervisor's single observation path.
            }
        }
    });

    let transport = StdioTransport::new_client(stdout, stdin);
    let client = rmcp::service::serve_client(handler, transport)
        .await
        .map_err(|err| TransportError::ConnectionFailed(format!("MCP handshake failed: {err}")))?;

    Ok((
        client,
        StdioProcess {
            exited,
            shutdown_requested,
            shutdown: Some(shutdown_sender),
        },
    ))
}
