use std::path::PathBuf;
use std::time::Duration;

use mcpstore::error::{Error, ErrorContext, FailureCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const KERNEL_PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// daemon 握手默认 namespace；v1 单 daemon 假设下 CLI 与 daemon 共用它。
pub const DEFAULT_NAMESPACE: &str = "mcpstore";

/// Default Unix socket path for KernelHost IPC.
#[cfg(unix)]
pub fn default_socket_path() -> PathBuf {
    std::env::var("MCPSTORE_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/mcpstore.sock"))
}

/// Default PID file path.
pub fn default_pid_path() -> PathBuf {
    std::env::var("MCPSTORE_PID")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/mcpstore.pid"))
}

/// Default loopback address used by the KernelHost stream transport.
pub fn default_endpoint_address() -> String {
    std::env::var("MCPSTORE_KERNEL_ENDPOINT").unwrap_or_else(|_| "127.0.0.1:0".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum KernelTransport {
    UnixSocket,
    LoopbackTcp,
}

/// First message on every KernelHost connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub protocol_version: u32,
    pub namespace: String,
    #[serde(default)]
    pub client_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeResponse {
    pub protocol_version: u32,
    pub namespace: String,
    pub kernel_revision: String,
    pub host_capabilities: Vec<String>,
}

/// Typed process-boundary request. `payload` is the DTO for `operation`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelRequest {
    pub request_id: u64,
    pub operation: KernelOperation,
    pub payload: Value,
    #[serde(default = "default_deadline_ms")]
    pub deadline_ms: u64,
}

fn default_deadline_ms() -> u64 {
    DEFAULT_REQUEST_TIMEOUT.as_millis() as u64
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum KernelOperation {
    CallTool,
    StreamToolExecution,
    ListTools,
    ListServices,
    GetService,
    ConnectService,
    DisconnectService,
    RestartService,
    CheckService,
    WaitService,
    AddService,
    DeclareServiceScope,
    RemoveServiceScope,
    ListAgents,
    ShowConfig,
    ResetConfig,
    AuthStatus,
    AuthCallbackUri,
    AuthBegin,
    AuthCallback,
    AuthRefresh,
    AuthLogout,
    AuthScopeUpgrade,
    AuthSaveClientSecret,
    AuthSavePrivateKey,
    SubscribeEvents,
    StopHost,
    StatusHost,
    GetDaemonConfig,
    SetDaemonConfig,
    GetServiceInfo,
    UpdateService,
    ResourcesList,
    ResourcesTemplates,
    ResourcesRead,
    PromptsList,
    PromptGet,
    CompleteArgument,
    TaskList,
    TaskGet,
    TaskCancel,
    SwapStore,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KernelEvent {
    Started {
        request_id: Value,
        instance_id: mcpstore::InstanceId,
        cancellation: bool,
    },
    Progress {
        request_id: Value,
        #[serde(flatten)]
        progress: mcpstore::McpExecutionProgress,
    },
    Finished {
        #[serde(flatten)]
        result: Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelResponse {
    pub request_id: Option<u64>,
    pub event: Option<KernelEvent>,
    pub result: Option<Value>,
    pub error: Option<KernelError>,
    pub kernel_revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelError {
    pub code: FailureCode,
    pub message: String,
    pub context: ErrorContext,
}

impl KernelError {
    pub fn new(code: FailureCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: ErrorContext::None,
        }
    }

    pub fn from_error(error: &Error) -> Self {
        Self {
            code: error.code(),
            message: error.message().to_string(),
            context: error.context().clone(),
        }
    }

    pub fn into_error(self) -> Error {
        Error::new(self.code, self.message).with_context(self.context)
    }
}

impl KernelResponse {
    pub fn ok(request_id: u64, result: impl Into<Option<Value>>) -> Self {
        Self {
            request_id: Some(request_id),
            event: None,
            result: result.into(),
            error: None,
            kernel_revision: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn event(event: KernelEvent) -> Self {
        Self {
            request_id: None,
            event: Some(event),
            result: None,
            error: None,
            kernel_revision: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn error(request_id: Option<u64>, error: impl Into<KernelError>) -> Self {
        Self {
            request_id,
            event: None,
            result: None,
            error: Some(error.into()),
            kernel_revision: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }
}

pub fn validate_handshake(
    request: &HandshakeRequest,
    namespace: &str,
) -> Result<HandshakeResponse, KernelError> {
    if request.protocol_version != KERNEL_PROTOCOL_VERSION {
        return Err(KernelError::new(
            FailureCode::HandshakeIncompatible,
            format!(
                "KernelHost protocol {} is incompatible with client protocol {}",
                KERNEL_PROTOCOL_VERSION, request.protocol_version
            ),
        ));
    }
    if request.namespace != namespace {
        return Err(KernelError::new(
            FailureCode::ConnectionScope,
            format!(
                "KernelHost namespace mismatch: host={namespace} client={}",
                request.namespace
            ),
        ));
    }
    Ok(HandshakeResponse {
        protocol_version: KERNEL_PROTOCOL_VERSION,
        namespace: namespace.to_string(),
        kernel_revision: env!("CARGO_PKG_VERSION").to_string(),
        host_capabilities: vec!["requests".into(), "streams".into(), "events".into()],
    })
}

pub fn deadline(deadline_ms: u64) -> Duration {
    Duration::from_millis(deadline_ms.min(DEFAULT_REQUEST_TIMEOUT.as_millis() as u64))
}

/// Check whether the host appears to be alive by reading its PID file
/// and verifying the process exists.
pub fn is_daemon_running() -> bool {
    is_host_running()
}

pub fn is_host_running() -> bool {
    let pid_path = default_pid_path();
    let Ok(pid_str) = std::fs::read_to_string(&pid_path) else {
        return false;
    };
    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        return false;
    };
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string()])
            .output()
            .is_ok()
    }
}

/// Remove stale PID and endpoint files if the host is not actually running.
pub fn cleanup_stale_files() {
    if !is_host_running() {
        let _ = std::fs::remove_file(default_pid_path());
        #[cfg(unix)]
        let _ = std::fs::remove_file(default_socket_path());
    }
}

#[cfg(test)]
mod host_tests {
    use super::*;

    #[test]
    fn is_host_running_ignores_empty_and_malformed_pid_files() {
        let old_socket = std::env::var("MCPSTORE_SOCKET").ok();
        let old_pid = std::env::var("MCPSTORE_PID").ok();
        let dir = std::env::temp_dir().join(format!(
            "mcpstore-host-pid-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("MCPSTORE_SOCKET", dir.join("kernel.sock"));
        std::env::set_var("MCPSTORE_PID", dir.join("kernel.pid"));

        let pid_path = default_pid_path();
        std::fs::write(&pid_path, "").unwrap();
        assert!(!is_host_running());
        std::fs::write(&pid_path, "not-a-pid").unwrap();
        assert!(!is_host_running());
        std::fs::remove_file(&pid_path).unwrap();
        assert!(!is_host_running());

        if let Some(value) = old_socket {
            std::env::set_var("MCPSTORE_SOCKET", value);
        } else {
            std::env::remove_var("MCPSTORE_SOCKET");
        }
        if let Some(value) = old_pid {
            std::env::set_var("MCPSTORE_PID", value);
        } else {
            std::env::remove_var("MCPSTORE_PID");
        }
        let _ = std::fs::remove_dir(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_error_serializes_as_code_message_context_object() {
        let error =
            mcpstore::Error::new(FailureCode::ServiceNotFound, "service instance not found")
                .with_context(ErrorContext::Service {
                    instance_id: "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap(),
                    service_name: "demo".to_string(),
                });
        let wire = serde_json::to_value(KernelError::from_error(&error)).unwrap();
        assert_eq!(wire["code"], "service_not_found");
        assert_eq!(wire["message"], "service instance not found");
        assert_eq!(wire["context"]["kind"], "service");
        assert_eq!(wire["context"]["service_name"], "demo");

        let round_trip: KernelError = serde_json::from_value(wire).unwrap();
        let restored = round_trip.into_error();
        assert_eq!(restored.code(), FailureCode::ServiceNotFound);
        assert!(restored.message().contains("service instance"));
    }

    #[test]
    fn handshake_rejects_wrong_protocol_and_namespace() {
        let request = HandshakeRequest {
            protocol_version: KERNEL_PROTOCOL_VERSION + 1,
            namespace: "demo".to_string(),
            client_capabilities: Vec::new(),
        };
        assert!(validate_handshake(&request, "demo").is_err());

        let request = HandshakeRequest {
            protocol_version: KERNEL_PROTOCOL_VERSION,
            namespace: "other".to_string(),
            client_capabilities: Vec::new(),
        };
        assert!(validate_handshake(&request, "demo").is_err());
    }

    #[test]
    fn deadline_is_bounded() {
        assert_eq!(deadline(250), Duration::from_millis(250));
        assert_eq!(
            deadline(DEFAULT_REQUEST_TIMEOUT.as_millis() as u64 + 1),
            DEFAULT_REQUEST_TIMEOUT
        );
    }
}
