use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// Default Unix socket path for daemon IPC.
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

/// A request sent from the CLI client to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonRequest {
    pub method: String,
    pub params: Value,
}

impl DaemonRequest {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            method: method.into(),
            params,
        }
    }

    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }
}

/// A response sent from the daemon back to the CLI client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DaemonResponse {
    pub fn ok(data: impl Into<Option<Value>>) -> Self {
        Self {
            success: true,
            data: data.into(),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }

    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }
}

/// Check whether the daemon appears to be alive by reading its PID file
/// and verifying the process exists.
pub fn is_daemon_running() -> bool {
    let pid_path = default_pid_path();
    let Ok(pid_str) = std::fs::read_to_string(&pid_path) else {
        return false;
    };
    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        return false;
    };
    // Check if process exists (send signal 0)
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        // On non-Unix, just check the socket exists as a fallback.
        default_socket_path().exists()
    }
}

/// Remove stale PID and socket files if the daemon is not actually running.
pub fn cleanup_stale_files() {
    if !is_daemon_running() {
        let _ = std::fs::remove_file(default_pid_path());
        let _ = std::fs::remove_file(default_socket_path());
    }
}
