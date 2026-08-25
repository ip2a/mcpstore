use std::collections::VecDeque;
use std::fmt::Display;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use crate::config::ServerConfig;
use crate::error::{Error, FailureCode};
use crate::health::supervisor::InstanceSupervisor;
use crate::identity::InstanceId;
use crate::transport::client::McpClient;
use crate::transport::handler::McpStoreClientHandler;

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
) -> crate::error::Result<(McpClient, StdioProcess)> {
    let command = config.command.as_deref().ok_or_else(|| {
        Error::new(
            FailureCode::ConnectionUnsupported,
            format!("Service {name} missing command field"),
        )
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
        Error::new(
            FailureCode::ConnectionSpawnFailed,
            format!("Failed to spawn child process: {err}"),
        )
        .with_source(err)
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        Error::new(
            FailureCode::ConnectionSpawnFailed,
            format!("stdio child for {name} has no stdout"),
        )
    })?;
    let stdin = child.stdin.take().ok_or_else(|| {
        Error::new(
            FailureCode::ConnectionSpawnFailed,
            format!("stdio child for {name} has no stdin"),
        )
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
            // Preserve rmcp's structured classification (fallback-eligible
            // handshake codes survive); swap only the message for the
            // user-facing detail with the child's stderr tail.
            let classified = crate::transport::handshake_error(config.handshake_mode(), err);
            return Err(
                Error::new(classified.code(), detail).with_context(classified.context().clone())
            );
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
                | "^"
                | "{"
                | "}"
        ) || trimmed.starts_with("File \"")
            // Python: uv/pip installer output.
            || trimmed.starts_with("Installed ")
            || trimmed.starts_with("Audited ")
            || trimmed.starts_with("Resolved ")
            || trimmed.starts_with("Downloading")
            || trimmed.starts_with("Using cached")
            || trimmed.starts_with("Preparing ")
            || trimmed.starts_with("Building ")
            || trimmed.starts_with("Collecting ")
            // Node.js / JS: version banner, stack frames, crash scaffolding.
            || trimmed.starts_with("Node.js v")
            || trimmed.starts_with("node:")
            || trimmed.starts_with("throw ")
            || trimmed.starts_with("at ")
            || trimmed.starts_with("code:")
    };

    // Prefer a headline line that carries the error type/code and its message
    // (e.g. "Error: ...", "Error [ERR_FOO]: ...", "ImportError: ..."). This
    // matters for runtimes whose stderr does not end on the error line —
    // Node.js prints a stack and a "Node.js vX" banner after it.
    let is_error_headline = |line: &str| {
        let trimmed = line.trim();
        trimmed.contains("Error:")
            || trimmed.contains("Exception:")
            || trimmed.contains(" ERR_")
            || trimmed.contains("[ERR_")
    };

    lines
        .iter()
        .rev()
        .find(|line| !is_noise(line) && is_error_headline(line))
        .cloned()
        .or_else(|| lines.iter().rev().find(|line| !is_noise(line)).cloned())
}

#[cfg(test)]
mod tests {
    use super::extract_startup_cause;

    fn lines(raw: &str) -> Vec<String> {
        raw.lines().map(String::from).collect()
    }

    #[test]
    fn cause_extracts_node_js_error_code_line() {
        let raw = "\
node:internal/modules/esm/resolve:262
    throw new ERR_UNSUPPORTED_DIR_IMPORT(path, basePath, String(resolved));
          ^
Error [ERR_UNSUPPORTED_DIR_IMPORT]: Directory import '.../zod/v3' is not supported resolving ES modules imported from .../zod-compat.js
    at finalizeResolution (node:internal/modules/esm/resolve:262:11)
    at moduleResolve (node:internal/modules/esm/resolve:864:10) {
  code: 'ERR_UNSUPPORTED_DIR_IMPORT',
}
Node.js v24.13.0
";
        let cause = extract_startup_cause(&lines(raw)).expect("a cause");
        assert!(
            cause.contains("ERR_UNSUPPORTED_DIR_IMPORT") && cause.contains("Directory import"),
            "got: {cause}"
        );
    }

    #[test]
    fn cause_extracts_python_import_error() {
        let raw = "\
Installed 31 packages in 29ms
Traceback (most recent call last):
  File \"mcp-server-time\", line 6, in <module>
    from mcp_server_time import main
  File \"server.py\", line 12, in <module>
    from mcp.shared.exceptions import McpError
ImportError: cannot import name 'McpError' from 'mcp.shared.exceptions'. Did you mean: 'MCPError'?
";
        let cause = extract_startup_cause(&lines(raw)).expect("a cause");
        assert!(
            cause.starts_with("ImportError:") && cause.contains("McpError"),
            "got: {cause}"
        );
    }

    #[test]
    fn cause_falls_back_to_last_meaningful_line_without_an_error_headline() {
        let raw = "\
some server banner line
another descriptive line
";
        let cause = extract_startup_cause(&lines(raw)).expect("a cause");
        assert_eq!(cause, "another descriptive line");
    }

    #[test]
    fn cause_returns_none_for_pure_noise() {
        let raw = "\
Installed 3 packages
Node.js v24.13.0
    at someFrame (file:1:1)
";
        assert!(extract_startup_cause(&lines(raw)).is_none());
    }
}
