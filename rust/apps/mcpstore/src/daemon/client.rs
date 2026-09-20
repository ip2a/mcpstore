use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use mcpstore::error::{Error, FailureCode};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::daemon::transport::{HostStreamReadHalf, HostStreamWriteHalf};

use crate::daemon::protocol::{
    HandshakeRequest, HandshakeResponse, KernelEvent, KernelOperation, KernelRequest,
    KernelResponse, KERNEL_PROTOCOL_VERSION,
};

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct DaemonEndpoint {
    pub address: String,
    pub namespace: String,
    pub token: Option<String>,
}

impl DaemonEndpoint {
    pub fn from_args(
        endpoint: Option<String>,
        namespace: Option<String>,
        token: Option<String>,
    ) -> Option<Self> {
        endpoint.map(|address| Self {
            address,
            namespace: namespace
                .unwrap_or_else(|| crate::daemon::protocol::DEFAULT_NAMESPACE.to_string()),
            token,
        })
    }
}

/// 连接 daemon 管理面；无 remote endpoint 时连本机默认 namespace。
pub async fn connect_admin(endpoint: Option<&DaemonEndpoint>) -> Result<KernelClient, Error> {
    match endpoint {
        Some(endpoint) => KernelClient::connect_remote(&endpoint.namespace, endpoint).await,
        None => KernelClient::connect(crate::daemon::protocol::DEFAULT_NAMESPACE).await,
    }
}

#[derive(Debug)]
pub struct KernelClient {
    writer: HostStreamWriteHalf,
    reader: BufReader<HostStreamReadHalf>,
}

impl KernelClient {
    pub async fn connect(namespace: &str) -> Result<Self, Error> {
        let stream = crate::daemon::transport::connect_endpoint().await?;
        Self::finish_connect(namespace, None, stream).await
    }

    pub async fn connect_remote(namespace: &str, endpoint: &DaemonEndpoint) -> Result<Self, Error> {
        let stream = crate::daemon::transport::connect_tcp_endpoint(&endpoint.address).await?;
        Self::finish_connect(namespace, endpoint.token.as_deref(), stream).await
    }

    async fn finish_connect(
        namespace: &str,
        token: Option<&str>,
        stream: crate::daemon::transport::HostStream,
    ) -> Result<Self, Error> {
        let (reader, writer) = stream.into_split();
        let mut client = Self {
            writer,
            reader: BufReader::new(reader),
        };

        let handshake = HandshakeRequest {
            protocol_version: KERNEL_PROTOCOL_VERSION,
            namespace: namespace.to_string(),
            client_capabilities: vec!["requests".into()],
            token: token.map(str::to_string),
        };
        let line = serde_json::to_string(&handshake).map_err(wire_error)?;
        client.write_line(&line).await?;
        let response = client.read_response(None).await?;
        if let Some(error) = response.error {
            return Err(error.into_error());
        }
        let _: HandshakeResponse = serde_json::from_value(response.result.unwrap_or(Value::Null))
            .map_err(|error| {
            Error::new(
                FailureCode::HandshakeFailed,
                format!("KernelHost returned a malformed handshake: {error}"),
            )
        })?;
        Ok(client)
    }

    /// Send one typed request and wait for its response. Stream events emitted
    /// before the terminal response are returned in arrival order.
    pub async fn request(
        &mut self,
        operation: KernelOperation,
        payload: Value,
        timeout: Duration,
    ) -> Result<(Value, Vec<KernelEvent>), Error> {
        self.call(operation, payload, timeout).await?;
        let mut events = Vec::new();
        loop {
            let response = self.read_response(None).await?;
            if let Some(event) = response.event {
                events.push(event);
                continue;
            }
            if response.request_id.is_none() {
                return Err(Error::new(
                    FailureCode::ConnectionClosed,
                    "KernelHost returned a response without request_id",
                ));
            }
            if let Some(error) = response.error {
                return Err(error.into_error());
            }
            return Ok((response.result.unwrap_or(Value::Null), events));
        }
    }

    /// 流式请求（StreamToolExecution 专用）：事件即时回调，`Finished` 事件即终态，
    /// 返回其 result 载荷。放弃连接（drop）即停止接收事件。
    pub async fn request_stream<F>(
        &mut self,
        operation: KernelOperation,
        payload: Value,
        timeout: Duration,
        mut on_event: F,
    ) -> Result<Value, Error>
    where
        F: FnMut(KernelEvent),
    {
        self.call(operation, payload, timeout).await?;
        loop {
            let response = self.read_response(None).await?;
            if let Some(event) = response.event {
                match event {
                    KernelEvent::Finished { result } => return Ok(result),
                    other => on_event(other),
                }
                continue;
            }
            if response.request_id.is_none() {
                return Err(Error::new(
                    FailureCode::ConnectionClosed,
                    "KernelHost returned a response without request_id",
                ));
            }
            if let Some(error) = response.error {
                return Err(error.into_error());
            }
            return Ok(response.result.unwrap_or(Value::Null));
        }
    }

    async fn call(
        &mut self,
        operation: KernelOperation,
        payload: Value,
        timeout: Duration,
    ) -> Result<(), Error> {
        let request = KernelRequest {
            request_id: NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
            operation,
            payload,
            deadline_ms: timeout.as_millis() as u64,
        };
        let line = serde_json::to_string(&request).map_err(wire_error)?;
        self.write_line(&line).await
    }

    async fn write_line(&mut self, line: &str) -> Result<(), Error> {
        self.writer
            .write_all(format!("{line}\n").as_bytes())
            .await
            .map_err(io_error)
    }

    async fn read_response(&mut self, timeout: Option<Duration>) -> Result<KernelResponse, Error> {
        let mut line = String::new();
        let read = self.reader.read_line(&mut line);
        let read = if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, read).await.map_err(|_| {
                Error::new(
                    FailureCode::ConnectionTimedOut,
                    "KernelHost response timed out",
                )
            })?
        } else {
            Ok(read.await.map_err(io_error)?)
        };
        let read = read.map_err(io_error)?;
        if read == 0 {
            return Err(Error::new(
                FailureCode::ConnectionClosed,
                "KernelHost connection closed",
            ));
        }
        serde_json::from_str(&line).map_err(|error| {
            Error::new(
                FailureCode::ConnectionClosed,
                format!("failed to parse KernelHost response: {error}"),
            )
        })
    }

    pub async fn stop_host(&mut self) -> Result<(), Error> {
        self.request(
            KernelOperation::StopHost,
            Value::Null,
            Duration::from_secs(5),
        )
        .await
        .map(|_| ())
    }

    pub async fn status_host(&mut self) -> Result<Value, Error> {
        self.request(
            KernelOperation::StatusHost,
            Value::Null,
            Duration::from_secs(10),
        )
        .await
        .map(|(result, _)| result)
    }

    pub async fn get_daemon_config(&mut self) -> Result<Value, Error> {
        self.request(
            KernelOperation::GetDaemonConfig,
            Value::Null,
            Duration::from_secs(10),
        )
        .await
        .map(|(result, _)| result)
    }

    pub async fn set_daemon_config(&mut self, key: &str, value: Value) -> Result<Value, Error> {
        self.request(
            KernelOperation::SetDaemonConfig,
            serde_json::json!({"key": key, "value": value}),
            Duration::from_secs(30),
        )
        .await
        .map(|(result, _)| result)
    }
}

/// 连接本机 daemon（默认 namespace）并请求优雅停机。
pub async fn stop_daemon() -> Result<Value, Error> {
    let mut client = KernelClient::connect(crate::daemon::protocol::DEFAULT_NAMESPACE).await?;
    client
        .request(
            KernelOperation::StopHost,
            Value::Null,
            Duration::from_secs(5),
        )
        .await
        .map(|(result, _)| result)
}

fn wire_error(error: serde_json::Error) -> Error {
    Error::new(
        FailureCode::ConnectionClosed,
        format!("failed to serialize KernelHost request: {error}"),
    )
}

fn io_error(error: std::io::Error) -> Error {
    Error::new(
        FailureCode::ConnectionClosed,
        format!("KernelHost write failed: {error}"),
    )
}
