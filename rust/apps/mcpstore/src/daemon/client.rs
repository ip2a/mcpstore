use mcpstore::error::{Error, FailureCode};
use serde_json::Value;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::daemon::protocol::{default_socket_path, DaemonError, DaemonRequest, DaemonResponse};

/// Check whether the daemon socket exists and is connectable.
pub fn daemon_socket_exists() -> bool {
    default_socket_path().exists()
}

/// Send a single request to the daemon and return the parsed response.
pub async fn call_daemon(method: impl Into<String>, params: Value) -> Result<Value, Error> {
    let socket_path = default_socket_path();
    if !socket_path.exists() {
        return Err(Error::new(
            FailureCode::ServiceUnavailable,
            "daemon is not running; run `mcpstore start` first",
        ));
    }

    let mut stream = UnixStream::connect(&socket_path).await.map_err(|e| {
        Error::new(
            FailureCode::ConnectionRefused,
            format!("failed to connect to daemon socket: {e}"),
        )
    })?;

    let request = DaemonRequest::new(method, params);
    let line = request.to_json_line().map_err(|e| {
        Error::new(
            FailureCode::ConnectionClosed,
            format!("failed to serialize daemon request: {e}"),
        )
    })?;

    stream.write_all(line.as_bytes()).await.map_err(|e| {
        Error::new(
            FailureCode::ConnectionClosed,
            format!("daemon write failed: {e}"),
        )
    })?;

    // Shutdown write to signal end of request.
    let _ = stream.shutdown().await;

    let (reader, _) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    tokio::time::timeout(Duration::from_secs(60), buf_reader.read_line(&mut line))
        .await
        .map_err(|_| Error::new(FailureCode::ConnectionTimedOut, "daemon response timed out"))?
        .map_err(|e| {
            Error::new(
                FailureCode::ConnectionClosed,
                format!("failed to read daemon response: {e}"),
            )
        })?;

    let response: DaemonResponse = serde_json::from_str(&line).map_err(|e| {
        Error::new(
            FailureCode::ConnectionClosed,
            format!("failed to parse daemon response: {e}"),
        )
    })?;

    if response.success {
        Ok(response.data.unwrap_or(Value::Null))
    } else {
        Err(response
            .error
            .unwrap_or_else(|| DaemonError::new(FailureCode::Internal, "unknown daemon error"))
            .into_error())
    }
}
