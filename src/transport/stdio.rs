use std::collections::VecDeque;
use std::fmt::Display;
use std::io;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::task::{Context, Poll};
use std::time::Instant;

use crate::config::ServerConfig;
use crate::error::{Error, FailureCode};
use crate::health::supervisor::InstanceSupervisor;
use crate::identity::InstanceId;
use crate::store::CoreStdioStderrOptions;
use crate::telemetry::FrameTelemetry;
use crate::transport::client::McpClient;
use crate::transport::handler::McpStoreClientHandler;
use crate::transport::telemetry_transport::TelemetryTransport;

use rmcp::transport::async_rw::AsyncRwTransport;
use rmcp::RoleClient;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader, ReadBuf};

type StdioTransport = AsyncRwTransport<
    RoleClient,
    ObservedStdout<tokio::process::ChildStdout>,
    tokio::process::ChildStdin,
>;

const MAX_OBSERVED_LINE_BYTES: usize = 16 * 1024 * 1024;

/// Transparent stdout reader that timestamps the first byte of each JSON-RPC line.
struct ObservedStdout<R> {
    inner: R,
    instance_id: String,
    line: Vec<u8>,
    line_started: Option<Instant>,
    discarding: bool,
}

impl<R> ObservedStdout<R> {
    fn new(inner: R, instance_id: InstanceId) -> Self {
        Self {
            inner,
            instance_id: instance_id.to_string(),
            line: Vec::new(),
            line_started: None,
            discarding: false,
        }
    }

    fn observe_bytes(&mut self, bytes: &[u8], arrived: Instant) {
        let mut start = 0;
        for (index, byte) in bytes.iter().enumerate() {
            if !self.discarding && self.line_started.is_none() {
                self.line_started = Some(arrived);
            }
            if *byte != b'\n' {
                continue;
            }
            if !self.discarding {
                self.line.extend_from_slice(&bytes[start..index]);
                if self.line.len() <= MAX_OBSERVED_LINE_BYTES {
                    if let (Ok(envelope), Some(first_byte)) = (
                        serde_json::from_slice::<serde_json::Value>(&self.line),
                        self.line_started,
                    ) {
                        if envelope.get("method").is_none()
                            && (envelope.get("result").is_some() || envelope.get("error").is_some())
                        {
                            if let Some(id) = envelope.get("id").map(|value| match value {
                                serde_json::Value::String(id) => id.clone(),
                                other => other.to_string(),
                            }) {
                                crate::telemetry::mark_stdio_response(
                                    &self.instance_id,
                                    &id,
                                    first_byte,
                                );
                            }
                        }
                    }
                }
            }
            self.line.clear();
            self.line_started = None;
            self.discarding = false;
            start = index + 1;
        }
        if start < bytes.len() && !self.discarding {
            let tail = &bytes[start..];
            if self.line.len().saturating_add(tail.len()) > MAX_OBSERVED_LINE_BYTES {
                self.line.clear();
                self.line_started = None;
                self.discarding = true;
            } else {
                self.line.extend_from_slice(tail);
            }
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for ObservedStdout<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let filled = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let arrived = Instant::now();
                self.observe_bytes(&buf.filled()[filled..], arrived);
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

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
    frame_telemetry: Option<Arc<dyn FrameTelemetry>>,
    capture_frame_payload: bool,
    stderr_options: CoreStdioStderrOptions,
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

    // Always drain the piped stderr, even when capture is disabled; otherwise
    // a noisy child can block on a full OS pipe before completing the handshake.
    let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    let capture_stderr = stderr_options.enabled;
    let tail_lines = stderr_options.tail_lines;
    let forward_to_runtime_log = stderr_options.forward_to_runtime_log;
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
                        if !capture_stderr {
                            continue;
                        }
                        let trimmed = line.trim_end();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if forward_to_runtime_log {
                            tracing::debug!(
                                target: "mcpstore::transport::stdio::stderr",
                                "{trimmed}"
                            );
                        }
                        if tail_lines == 0 {
                            continue;
                        }
                        let mut guard = match tail.lock() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        if guard.len() >= tail_lines {
                            guard.pop_front();
                        }
                        guard.push_back(trimmed.to_string());
                    }
                    Err(error) => {
                        if capture_stderr {
                            tracing::debug!("stdio stderr read failed: {error}");
                        }
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

    let stdout = ObservedStdout::new(stdout, instance_id);
    let transport = StdioTransport::new_client(stdout, stdin);
    let transport = TelemetryTransport::wrap(
        transport,
        frame_telemetry,
        capture_frame_payload,
        instance_id,
        name.to_string(),
        "stdio",
    );
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
            let error_tail: &[String] = if stderr_options.include_in_error {
                &tail
            } else {
                &[]
            };
            let detail = format_stdio_connect_failure(&err, error_tail, child_exited);
            // Keep the full stderr in the log so it is diagnosable even when
            // the user-facing message is trimmed to the cause line.
            if stderr_options.forward_to_runtime_log && !tail.is_empty() {
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
    use super::{extract_startup_cause, ObservedStdout};
    use crate::identity::InstanceId;
    use crate::store::{CoreDiagnosticsOptions, CoreStdioStderrOptions};
    use crate::telemetry::{bind_jsonrpc_trace, CallTrace};
    use crate::{ScopeRef, ServiceInstanceKey};
    use std::time::Instant;

    #[cfg(unix)]
    async fn connect_failure_with_stderr(
        script: &str,
        stderr_options: CoreStdioStderrOptions,
    ) -> crate::Error {
        use crate::config::ServerConfig;
        use crate::events::EventBus;
        use crate::registry::ServiceRegistry;
        use crate::transport::handler::McpStoreClientHandler;

        let instance_id = instance();
        let config = ServerConfig {
            command: Some("/bin/sh".to_string()),
            args: vec!["-c".to_string(), script.to_string()],
            ..ServerConfig::default()
        };
        let handler =
            McpStoreClientHandler::new(instance_id, ServiceRegistry::new(), EventBus::new());
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            super::connect(
                "stdio-stderr-test",
                &config,
                handler,
                instance_id,
                None,
                None,
                true,
                stderr_options,
            ),
        )
        .await
        .expect("child stderr pipe must keep draining");
        match result {
            Ok(_) => panic!("non-MCP child unexpectedly completed the handshake"),
            Err(error) => error,
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn disabled_stderr_capture_still_drains_more_than_the_pipe_capacity() {
        let mut options = CoreDiagnosticsOptions::default().stdio_stderr;
        options.enabled = false;
        let error = connect_failure_with_stderr(
            "i=0; while [ \"$i\" -lt 4096 ]; do printf 'STDERR_SENTINEL-%s-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\\n' \"$i\" >&2; i=$((i+1)); done; exit 1",
            options,
        )
        .await;
        assert!(!error.message().contains("STDERR_SENTINEL"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stderr_tail_and_error_inclusion_are_independently_configurable() {
        let script = "printf 'STDERR_SENTINEL first\\nSTDERR_SENTINEL final\\n' >&2; exit 1";
        let captured =
            connect_failure_with_stderr(script, CoreDiagnosticsOptions::default().stdio_stderr)
                .await;
        assert!(captured.message().contains("STDERR_SENTINEL final"));

        let mut no_tail = CoreDiagnosticsOptions::default().stdio_stderr;
        no_tail.tail_lines = 0;
        let without_tail = connect_failure_with_stderr(script, no_tail).await;
        assert!(!without_tail.message().contains("STDERR_SENTINEL"));

        let mut no_error_text = CoreDiagnosticsOptions::default().stdio_stderr;
        no_error_text.include_in_error = false;
        let without_error_text = connect_failure_with_stderr(script, no_error_text).await;
        assert!(!without_error_text.message().contains("STDERR_SENTINEL"));
        assert!(without_error_text
            .message()
            .contains("exited during startup"));
    }

    #[cfg(unix)]
    #[test]
    fn stderr_runtime_log_forwarding_is_independently_configurable() {
        use std::io::Write;
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;

        #[derive(Clone, Default)]
        struct LogBuffer(Arc<Mutex<Vec<u8>>>);
        struct LogGuard(Arc<Mutex<Vec<u8>>>);

        impl<'a> MakeWriter<'a> for LogBuffer {
            type Writer = LogGuard;

            fn make_writer(&'a self) -> Self::Writer {
                LogGuard(Arc::clone(&self.0))
            }
        }

        impl Write for LogGuard {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let logs = LogBuffer::default();
        let subscriber = || {
            tracing_subscriber::fmt()
                .with_ansi(false)
                .without_time()
                .with_max_level(tracing::Level::DEBUG)
                .with_writer(logs.clone())
                .finish()
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let script = "printf 'FORWARD_SENTINEL\\n' >&2; exit 1";

        let mut disabled = CoreDiagnosticsOptions::default().stdio_stderr;
        disabled.forward_to_runtime_log = false;
        tracing::subscriber::with_default(subscriber(), || {
            runtime.block_on(connect_failure_with_stderr(script, disabled));
        });
        let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
        assert!(!output.contains("FORWARD_SENTINEL"), "got: {output}");

        tracing::subscriber::with_default(subscriber(), || {
            runtime.block_on(connect_failure_with_stderr(
                script,
                CoreDiagnosticsOptions::default().stdio_stderr,
            ));
        });
        let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
        assert!(output.contains("FORWARD_SENTINEL"));
    }

    fn instance() -> InstanceId {
        ServiceInstanceKey::new("stdio-timing-test", ScopeRef::Store).instance_id()
    }

    #[test]
    fn stdout_observer_correlates_split_and_coalesced_jsonrpc_lines() {
        let instance_id = instance();
        let split = CallTrace::register("stdio-split");
        let coalesced = CallTrace::register("stdio-coalesced");
        bind_jsonrpc_trace(&instance_id.to_string(), "41".to_string(), split.clone());
        bind_jsonrpc_trace(
            &instance_id.to_string(),
            "42".to_string(),
            coalesced.clone(),
        );
        let mut stdout = ObservedStdout::new(tokio::io::empty(), instance_id);

        stdout.observe_bytes(
            br#"{"jsonrpc":"2.0","id":41,"method":"elicitation/create","params":{}}
"#,
            Instant::now(),
        );
        assert!(split.timing().first_byte_ms.is_none());
        assert!(split.timing().response_read_ms.is_none());

        let split_start = Instant::now();
        stdout.observe_bytes(br#"{"jsonrpc":"2.0","id":41,"#, split_start);
        stdout.observe_bytes(
            concat!(
                "\"result\":{}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":42,\"result\":{}}\n"
            )
            .as_bytes(),
            Instant::now(),
        );

        assert!(split.timing().first_byte_ms.is_some());
        assert!(split.timing().response_read_ms.is_some());
        assert!(coalesced.timing().first_byte_ms.is_some());
        assert!(coalesced.timing().response_read_ms.is_some());
        split.unregister();
        coalesced.unregister();
    }

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
