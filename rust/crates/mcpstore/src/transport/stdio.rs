use std::collections::VecDeque;
use std::fmt::Display;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use crate::config::ServerConfig;
use crate::health::supervisor::InstanceSupervisor;
use crate::identity::InstanceId;
use crate::transport::client::McpClient;
use crate::transport::handler::McpStoreClientHandler;
use crate::transport::{Result, TransportError};

use rmcp::transport::async_rw::AsyncRwTransport;
use rmcp::RoleClient;
use tokio::io::{AsyncBufReadExt, BufReader};

type StdioTransport =
    AsyncRwTransport<RoleClient, tokio::process::ChildStdout, tokio::process::ChildStdin>;

/// How many trailing child stderr lines we keep to surface in handshake errors.
const STDERR_TAIL_LINES: usize = 50;

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
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|err| {
        TransportError::ConnectionFailed(format!("Failed to spawn child process: {err}"))
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        TransportError::ConnectionFailed(format!("stdio child for {name} has no stdout"))
    })?;
    let stdin = child.stdin.take().ok_or_else(|| {
        TransportError::ConnectionFailed(format!("stdio child for {name} has no stdin"))
    })?;
    let stderr = child.stderr.take();

    // Capture child stderr so we can surface it when the handshake fails, and
    // keep it visible through tracing while the service runs.
    let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    if let Some(stderr) = stderr {
        let tail = Arc::clone(&stderr_tail);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // EOF — child closed stderr
                    Ok(_) => {
                        let trimmed = line.trim_end();
                        if trimmed.is_empty() {
                            continue;
                        }
                        tracing::debug!(
                            target: "mcpstore::transport::stdio::stderr",
                            "{trimmed}"
                        );
                        let mut guard = match tail.lock() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        if guard.len() >= STDERR_TAIL_LINES {
                            guard.pop_front();
                        }
                        guard.push_back(trimmed.to_string());
                    }
                    Err(error) => {
                        tracing::debug!("stdio stderr read failed: {error}");
                        break;
                    }
                }
            }
        });
    }

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
    let client = match rmcp::service::serve_client_with_lifecycle(
        handler,
        transport,
        crate::transport::client_lifecycle_mode(config.handshake_mode()),
    )
    .await
    {
        Ok(client) => client,
        Err(err) => {
            // Give the child a moment to flush stderr before we snapshot it.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let tail: Vec<String> = match stderr_tail.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            }
            .iter()
            .cloned()
            .collect();
            let child_exited = exited.load(Ordering::Acquire);
            let detail = format_stdio_connect_failure(&err, &tail, child_exited);
            // Keep the full stderr in the log so it is diagnosable even when
            // the user-facing message is trimmed to the cause line.
            if !tail.is_empty() {
                tracing::warn!(
                    target: "mcpstore::transport::stdio::stderr",
                    instance_id = %instance_id,
                    "child stderr on connect failure:\n{}",
                    tail.join("\n")
                );
            }
            return Err(TransportError::ConnectionFailed(detail));
        }
    };

    Ok((
        client,
        StdioProcess {
            exited,
            shutdown_requested,
            shutdown: Some(shutdown_sender),
        },
    ))
}

/// Build a user-facing message for a stdio connect failure, distinguishing a
/// child that crashed during startup (never reached the MCP handshake) from a
/// genuine protocol/handshake error on a live process.
fn format_stdio_connect_failure(
    handshake_error: &impl Display,
    stderr_tail: &[String],
    child_exited: bool,
) -> String {
    if child_exited {
        // The process died before (or while) starting — this is a server-side
        // startup failure, not an MCP protocol issue. Surface the likely cause
        // instead of the misleading "handshake failed" framing.
        return match extract_startup_cause(stderr_tail) {
            Some(cause) => format!(
                "service process exited during startup and never reached the MCP \
                 handshake.\n  cause: {cause}\n  hint: this is usually a \
                 server-side problem — a dependency/import error (e.g. an \
                 incompatible MCP SDK version), a missing binary, or an invalid \
                 command. Full stderr is in the log file."
            ),
            None => format!(
                "service process exited during startup and never reached the MCP \
                 handshake; no stderr output was captured.\n  hint: verify the \
                 command and arguments are correct and that the service can start \
                 on its own."
            ),
        };
    }

    // Process is still alive — a real handshake/protocol failure.
    let mut detail = format!("MCP handshake failed: {handshake_error}");
    if !stderr_tail.is_empty() {
        detail.push_str("\n  child stderr (last lines):\n    ");
        detail.push_str(&stderr_tail.join("\n    "));
    } else {
        detail.push_str("\n  (child process produced no stderr output)");
    }
    detail
}

/// Pull the most likely cause line out of a crashed child's stderr.
///
/// Filters out installer noise (uv/pip progress lines) and Python traceback
/// scaffolding (`Traceback ...`, `File "..."`, ...), returning the final
/// exception line — e.g. `ImportError: cannot import name 'McpError' ...`.
fn extract_startup_cause(lines: &[String]) -> Option<String> {
    let is_noise = |line: &str| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return true;
        }
        matches!(
            trimmed,
            "Traceback (most recent call last):"
                | "The above exception was the direct cause of the following exception:"
                | "During handling of the above exception, another exception occurred:"
        ) || trimmed.starts_with("File \"")
            || trimmed.starts_with("Installed ")
            || trimmed.starts_with("Audited ")
            || trimmed.starts_with("Resolved ")
            || trimmed.starts_with("Downloading")
            || trimmed.starts_with("Using cached")
            || trimmed.starts_with("Preparing ")
            || trimmed.starts_with("Building ")
            || trimmed.starts_with("Collecting ")
    };
    lines.iter().rev().find(|line| !is_noise(line)).cloned()
}
