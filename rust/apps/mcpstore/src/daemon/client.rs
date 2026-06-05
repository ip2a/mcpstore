use serde_json::Value;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::daemon::protocol::{default_socket_path, DaemonRequest, DaemonResponse};

/// Check whether the daemon socket exists and is connectable.
pub fn daemon_socket_exists() -> bool {
    default_socket_path().exists()
}

/// Send a single request to the daemon and return the parsed response.
pub async fn call_daemon(method: impl Into<String>, params: Value) -> Result<Value, String> {
    let socket_path = default_socket_path();
    if !socket_path.exists() {
        return Err("Daemon not running. Run `mcpstore start` first.".to_string());
    }

    let mut stream = UnixStream::connect(&socket_path)
        .await
        .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

    let request = DaemonRequest::new(method, params);
    let line = request
        .to_json_line()
        .map_err(|e| format!("Failed to serialize request: {}", e))?;

    stream
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("Failed to write to daemon: {}", e))?;

    // Shutdown write to signal end of request.
    let _ = stream.shutdown().await;

    let (reader, _) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    tokio::time::timeout(Duration::from_secs(60), buf_reader.read_line(&mut line))
        .await
        .map_err(|_| "Daemon response timed out".to_string())?
        .map_err(|e| format!("Failed to read daemon response: {}", e))?;

    let response: DaemonResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Failed to parse daemon response: {}", e))?;

    if response.success {
        Ok(response.data.unwrap_or(Value::Null))
    } else {
        Err(response
            .error
            .unwrap_or_else(|| "Unknown daemon error".to_string()))
    }
}
