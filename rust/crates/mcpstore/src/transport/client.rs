use crate::auth::{AuthCoordinator, AuthStatus};
use crate::config::ServerConfig;
use crate::error::{Error, ErrorContext, FailureCode, Result};
use crate::events::EventBus;
use crate::health::supervisor::InstanceSupervisor;
use crate::identity::InstanceId;
use crate::registry::ServiceRegistry;
use crate::transport::handler::McpStoreClientHandler;
use crate::transport::stdio::StdioProcess;
use crate::transport::{http as http_transport, stdio as stdio_transport};

pub use crate::transport::pool::ConnectionPool;

use rmcp::model::{ClientRequest, PingRequest, ServerPeerInfo};
use rmcp::service::{RoleClient, RunningService};
use std::sync::Arc;

pub(super) type McpClient = RunningService<RoleClient, McpStoreClientHandler>;

fn ping_method_not_found(error: &rmcp::service::ServiceError) -> bool {
    matches!(
        error,
        rmcp::service::ServiceError::McpError(rmcp::model::ErrorData {
            code: rmcp::model::ErrorCode::METHOD_NOT_FOUND,
            ..
        })
    )
}

enum ActiveClient {
    Stdio(McpClient),
    Http(McpClient),
}

pub struct McpConnection {
    instance_id: InstanceId,
    name: String,
    config: ServerConfig,
    client: Option<ActiveClient>,
    stdio_process: Option<stdio_transport::StdioProcess>,
    auth_coordinator: AuthCoordinator,
    pub(in crate::transport) handler: McpStoreClientHandler,
    pub(in crate::transport) subscription_task: Option<tokio::task::JoinHandle<()>>,
    event_bus: crate::events::EventBus,
}

impl McpConnection {
    pub fn new(
        instance_id: InstanceId,
        name: String,
        config: ServerConfig,
        auth_coordinator: AuthCoordinator,
        registry: ServiceRegistry,
        event_bus: EventBus,
    ) -> Self {
        Self {
            instance_id,
            name,
            config,
            client: None,
            stdio_process: None,
            auth_coordinator,
            handler: McpStoreClientHandler::new(instance_id, registry, event_bus.clone()),
            event_bus,
            subscription_task: None,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.client.is_some()
            && self
                .stdio_process
                .as_ref()
                .map(StdioProcess::is_running)
                .unwrap_or(true)
    }

    #[cfg(test)]
    pub(super) fn from_test_client(
        instance_id: InstanceId,
        client: McpClient,
        handler: McpStoreClientHandler,
    ) -> Self {
        Self {
            instance_id,
            name: "protocol-test".to_string(),
            config: ServerConfig::default(),
            client: Some(ActiveClient::Stdio(client)),
            stdio_process: None,
            auth_coordinator: AuthCoordinator::for_tests(
                crate::auth::SystemKeyring::new().expect("test keyring"),
                crate::auth::test_state_manager(),
            )
            .expect("test auth coordinator"),
            handler,
            event_bus: crate::events::EventBus::new(),
            subscription_task: None,
        }
    }

    pub(crate) async fn connect(
        &mut self,
        supervisor: Option<Arc<InstanceSupervisor>>,
    ) -> Result<()> {
        let mode = self.config.handshake_mode();
        let result = self.connect_once(supervisor.clone()).await;
        let error = match result {
            Err(error) if needs_fallback(&error, mode) => error,
            other => return other,
        };
        tracing::warn!(
            service = %self.name,
            from_mode = ?mode,
            code = %error.code(),
            "handshake fallback to initialize"
        );
        self.event_bus
            .publish(
                crate::events::Event::new(
                    crate::events::EventKind::HandshakeFallback.as_str(),
                    serde_json::json!({
                        "instance_id": self.instance_id,
                        "service_name": self.name,
                        "from_mode": mode.as_str(),
                        "code": error.code().as_str(),
                    }),
                ),
                true,
            )
            .await;
        // At most one fallback per connect: the second attempt runs with the
        // override below and its failure is returned as-is.
        self.config
            .mcpstore
            .get_or_insert_with(Default::default)
            .handshake_mode = Some(crate::config::HandshakeMode::Initialize);
        self.connect_once(supervisor).await
    }

    async fn connect_once(&mut self, supervisor: Option<Arc<InstanceSupervisor>>) -> Result<()> {
        let transport_type = self.config.infer_transport().to_owned();
        tracing::info!(
            "Connecting to service {} (transport={})",
            self.name,
            transport_type
        );

        let result = match transport_type.as_str() {
            "stdio" => self.connect_stdio(supervisor).await,
            "streamable-http" | "http" => self.connect_http().await,
            other => Err(Error::new(
                FailureCode::ConnectionUnsupported,
                format!("Unsupported transport type: {other}"),
            )),
        };
        if let Err(error) = &result {
            // The transport layer logs the failure detail; this line exists so
            // failures can be found by service or instance. Auth outcomes are
            // a normal login flow, not a connection failure, so they stay out.
            if matches!(
                error.code().category(),
                crate::error::FailureCategory::Connection
                    | crate::error::FailureCategory::Handshake
            ) {
                tracing::warn!(
                    service = %self.name,
                    transport = transport_type,
                    instance_id = %self.instance_id,
                    "connect failed"
                );
            }
        }
        result
    }

    async fn connect_stdio(&mut self, supervisor: Option<Arc<InstanceSupervisor>>) -> Result<()> {
        let (client, process) = stdio_transport::connect(
            &self.name,
            &self.config,
            self.handler.clone(),
            self.instance_id,
            supervisor,
        )
        .await?;
        tracing::info!("stdio connected: {}", self.name);
        self.client = Some(ActiveClient::Stdio(client));
        self.stdio_process = Some(process);
        Ok(())
    }

    async fn connect_http(&mut self) -> Result<()> {
        let client = http_transport::connect(
            self.instance_id,
            &self.name,
            &self.config,
            &self.auth_coordinator,
            self.handler.clone(),
        )
        .await?;
        tracing::info!("HTTP connected: {}", self.name);
        self.client = Some(ActiveClient::Http(client));
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(task) = self.subscription_task.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(process) = self.stdio_process.take() {
            process.shutdown().await;
        }
        if let Some(client) = self.client.take() {
            let inner = match client {
                ActiveClient::Stdio(c) => c,
                ActiveClient::Http(c) => c,
            };
            inner.cancel().await.ok();
            self.handler.shutdown().await;
            tracing::info!("[TRANSPORT] Disconnected: {}", self.name);
        }
        Ok(())
    }

    pub(in crate::transport) fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    pub(in crate::transport) fn open_elicitation_session(
        &self,
        options: crate::transport::McpElicitationSessionOptions,
    ) -> Result<crate::transport::McpElicitationSession> {
        self.handler
            .open_elicitation_session(options)
            .map_err(|()| {
                Error::new(
                    FailureCode::ElicitationInvalidResponse,
                    format!(
                        "an elicitation session is already active for service instance {}",
                        self.instance_id
                    ),
                )
                .with_context(ErrorContext::Service {
                    instance_id: self.instance_id,
                    service_name: self.name.clone(),
                })
            })
    }

    pub(in crate::transport) fn subscribe_progress(
        &self,
    ) -> tokio::sync::broadcast::Receiver<rmcp::model::ProgressNotificationParam> {
        self.handler.subscribe_progress()
    }

    pub(in crate::transport) fn execution_auth(
        &self,
    ) -> (AuthCoordinator, crate::auth::AuthConfig) {
        (self.auth_coordinator.clone(), self.config.auth.clone())
    }

    pub(in crate::transport) fn peer_info(&self) -> Result<Arc<ServerPeerInfo>> {
        self.get_client()?.peer_info().ok_or_else(|| {
            Error::new(
                FailureCode::HandshakeFailed,
                format!("MCP handshake metadata unavailable for {}", self.name),
            )
        })
    }

    pub async fn ping(&self, timeout: std::time::Duration) -> Result<()> {
        let client = self.get_client()?;
        let result = tokio::time::timeout(
            timeout,
            client.send_request(ClientRequest::PingRequest(PingRequest::default())),
        )
        .await
        .map_err(|_| {
            Error::new(
                FailureCode::CallTimedOut,
                format!("MCP request timed out after {timeout:?}"),
            )
        })?;
        match result {
            Ok(_) => Ok(()),
            // A correlated JSON-RPC error proves the transport and server are
            // alive. Some deployed servers do not implement optional ping.
            Err(error) if ping_method_not_found(&error) => Ok(()),
            Err(error) => Err(Error::new(
                FailureCode::ToolFailed,
                format!("MCP ping failed: {error}"),
            )),
        }
    }

    pub(in crate::transport) fn get_client(&self) -> Result<&McpClient> {
        match &self.client {
            Some(ActiveClient::Stdio(c)) => Ok(c),
            Some(ActiveClient::Http(c)) => Ok(c),
            None => Err(Error::new(
                FailureCode::NotConnected,
                format!("Not connected: {}", self.name),
            )),
        }
    }

    pub(in crate::transport) async fn classify_client_failure(&self, fallback: Error) -> Error {
        if self.config.auth.is_none() {
            return fallback;
        }
        match self.auth_coordinator.status(self.instance_id).await {
            AuthStatus::Unauthenticated => {
                let required = self
                    .auth_coordinator
                    .auth_required(self.instance_id, &self.config.auth);
                Error::new(FailureCode::ConnectionAuthRequired, required.to_string())
                    .with_context(ErrorContext::Auth { required })
            }
            AuthStatus::ScopeUpgradeRequired => {
                let required_scope = self.auth_coordinator.required_scope(self.instance_id).await;
                Error::new(
                    FailureCode::ConnectionScope,
                    format!(
                        "insufficient OAuth scope for service instance {}, required: {required_scope:?}",
                        self.instance_id
                    ),
                )
                .with_context(ErrorContext::Scope {
                    instance_id: self.instance_id,
                    required_scope,
                })
            }
            _ => fallback,
        }
    }
}

fn needs_fallback(error: &Error, mode: crate::config::HandshakeMode) -> bool {
    // The policy table is the single source of which codes are fallback-eligible;
    // do not re-list handshake codes here or the two lists can drift apart.
    matches!(
        error.code().policy(),
        crate::error::RecoveryPolicy::HandshakeFallback
    ) && matches!(
        mode,
        crate::config::HandshakeMode::Auto | crate::config::HandshakeMode::Discover
    )
}

/// Raw streamable-HTTP MCP fixture: answers `server/discover` with a
/// configurable JSON-RPC error (or an id-mismatched response for the
/// uncorrelated case) and `initialize` either properly or with -32601.
/// One request per connection (`Connection: close`); counts the probes it
/// served so tests can assert exactly-one fallback.
#[cfg(test)]
struct RawHandshakeServer {
    discover_reply: DiscoverReply,
    reject_initialize: bool,
    discover_count: Arc<std::sync::atomic::AtomicUsize>,
    initialize_count: Arc<std::sync::atomic::AtomicUsize>,
}

#[derive(Clone)]
#[cfg(test)]
enum DiscoverReply {
    RpcError(i32),
    MismatchedId,
}

#[cfg(test)]
impl RawHandshakeServer {
    async fn spawn(self) -> String {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let discover_reply = self.discover_reply;
        let reject_initialize = self.reject_initialize;
        let discover_count = self.discover_count;
        let initialize_count = self.initialize_count;
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let discover_reply = discover_reply.clone();
                let discover_count = discover_count.clone();
                let initialize_count = initialize_count.clone();
                tokio::spawn(async move {
                    let mut head = Vec::new();
                    let mut buffer = vec![0u8; 16 * 1024];
                    // Read headers.
                    let head_end = loop {
                        let n = match socket.read(&mut buffer).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => n,
                        };
                        head.extend_from_slice(&buffer[..n]);
                        if let Some(i) = head.windows(4).position(|w| w == b"\r\n\r\n") {
                            break i + 4;
                        }
                    };
                    let head_text = String::from_utf8_lossy(&head[..head_end]).to_string();
                    let content_length = head_text
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())?
                        })
                        .unwrap_or(0);
                    let mut body = head[head_end..].to_vec();
                    while body.len() < content_length {
                        let n = match socket.read(&mut buffer).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => n,
                        };
                        body.extend_from_slice(&buffer[..n]);
                    }
                    serve_request(
                        &mut socket,
                        &head_text,
                        &body,
                        &discover_reply,
                        reject_initialize,
                        &discover_count,
                        &initialize_count,
                    )
                    .await;
                });
            }
        });
        format!("http://{addr}/mcp")
    }
}

#[cfg(test)]
async fn serve_request(
    socket: &mut tokio::net::TcpStream,
    head: &str,
    body: &[u8],
    discover_reply: &DiscoverReply,
    reject_initialize: bool,
    discover_count: &std::sync::atomic::AtomicUsize,
    initialize_count: &std::sync::atomic::AtomicUsize,
) {
    use std::sync::atomic::Ordering;
    use tokio::io::AsyncWriteExt;
    let is_get = head.starts_with("GET ");
    async fn write(socket: &mut tokio::net::TcpStream, response: String) {
        let _ = socket.write_all(response.as_bytes()).await;
    }
    if is_get {
        write(
            socket,
            "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
        )
        .await;
        return;
    }
    let request: serde_json::Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return,
    };
    let id = request.get("id").cloned();
    let rpc_method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let payload: serde_json::Value = match rpc_method {
        "server/discover" => {
            discover_count.fetch_add(1, Ordering::SeqCst);
            match discover_reply {
                DiscoverReply::RpcError(code) => serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": code, "message": "discover rejected", "data": null},
                }),
                DiscoverReply::MismatchedId => serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "mismatched-session-id",
                    "result": {"status": "ok"},
                }),
            }
        }
        "initialize" => {
            initialize_count.fetch_add(1, Ordering::SeqCst);
            if reject_initialize {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32601, "message": "method not found: initialize", "data": null},
                })
            } else {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2025-06-18",
                        "capabilities": {},
                        "serverInfo": {"name": "raw-fixture", "version": "1.0"},
                    },
                })
            }
        }
        _ if id.is_some() => serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": "method not found", "data": null},
        }),
        // Notifications (e.g. notifications/initialized) get an empty 202.
        _ => serde_json::Value::Null,
    };
    if payload.is_null() {
        write(
            socket,
            "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
        )
        .await;
        return;
    }
    let body = payload.to_string();
    write(
        socket,
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        ),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_method_not_found_means_server_responded() {
        let error = rmcp::service::ServiceError::McpError(rmcp::model::ErrorData::new(
            rmcp::model::ErrorCode::METHOD_NOT_FOUND,
            "method not found: ping",
            None,
        ));
        assert!(ping_method_not_found(&error));
    }
    use crate::events::EventBus;
    use crate::identity::{ScopeRef, ServiceInstanceKey};
    use crate::registry::ServiceRegistry;

    #[test]
    fn fallback_is_eligible_only_for_handshake_codes_in_probe_modes() {
        use crate::config::HandshakeMode;
        let code = |code| Error::new(code, "handshake");
        for error in [
            code(FailureCode::HandshakeIncompatible),
            code(FailureCode::HandshakeRejected),
            code(FailureCode::HandshakeUncorrelated),
        ] {
            assert!(needs_fallback(&error, HandshakeMode::Auto));
            assert!(needs_fallback(&error, HandshakeMode::Discover));
            assert!(!needs_fallback(&error, HandshakeMode::Initialize));
        }
        assert!(!needs_fallback(
            &Error::new(FailureCode::HandshakeFailed, "handshake"),
            HandshakeMode::Auto
        ));
        assert!(!needs_fallback(
            &Error::new(FailureCode::ConnectionRefused, "refused"),
            HandshakeMode::Auto
        ));
    }

    #[tokio::test]
    async fn sse_transport_is_rejected_before_connecting() {
        let instance_id = ServiceInstanceKey::new("sse-service", ScopeRef::Store).instance_id();
        let config = ServerConfig {
            url: Some("http://127.0.0.1:9/sse".to_string()),
            transport: Some("sse".to_string()),
            ..ServerConfig::default()
        };
        let mut connection = McpConnection::new(
            instance_id,
            "sse-service".to_string(),
            config,
            AuthCoordinator::for_tests(
                crate::auth::SystemKeyring::new().expect("test keyring"),
                crate::auth::test_state_manager(),
            )
            .expect("test auth coordinator"),
            ServiceRegistry::new(),
            EventBus::new(),
        );

        let error = connection.connect(None).await.unwrap_err();

        assert_eq!(error.code(), FailureCode::ConnectionUnsupported);
        assert_eq!(error.message(), "Unsupported transport type: sse");
        assert!(!connection.is_connected());
    }

    fn fallback_test_connection(url: String, mode: crate::config::HandshakeMode) -> McpConnection {
        let instance_id =
            ServiceInstanceKey::new("fallback-service", ScopeRef::Store).instance_id();
        let config = ServerConfig {
            url: Some(url),
            transport: Some("http".to_string()),
            mcpstore: Some(crate::config::McpStoreExtension {
                handshake_mode: Some(mode),
                ..Default::default()
            }),
            ..ServerConfig::default()
        };
        McpConnection::new(
            instance_id,
            "fallback-service".to_string(),
            config,
            AuthCoordinator::for_tests(
                crate::auth::SystemKeyring::new().expect("test keyring"),
                crate::auth::test_state_manager(),
            )
            .expect("test auth coordinator"),
            ServiceRegistry::new(),
            EventBus::new(),
        )
    }

    use std::sync::atomic::{AtomicUsize, Ordering};

    async fn fallback_case(
        reply: DiscoverReply,
        mode: crate::config::HandshakeMode,
    ) -> (crate::error::Result<()>, usize, usize) {
        let discover_count = Arc::new(AtomicUsize::new(0));
        let initialize_count = Arc::new(AtomicUsize::new(0));
        let server = RawHandshakeServer {
            discover_reply: reply,
            reject_initialize: false,
            discover_count: discover_count.clone(),
            initialize_count: initialize_count.clone(),
        };
        let url = server.spawn().await;
        let mut connection = fallback_test_connection(url, mode);
        let result = connection.connect(None).await;
        // Give the fixture a beat to finish counting the last request.
        for _ in 0..50 {
            let served =
                discover_count.load(Ordering::SeqCst) + initialize_count.load(Ordering::SeqCst);
            if served >= 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        (
            result,
            discover_count.load(Ordering::SeqCst),
            initialize_count.load(Ordering::SeqCst),
        )
    }

    #[tokio::test]
    async fn discover_rejected_with_method_not_found_falls_back_exactly_once() {
        let (result, discover, initialize) = fallback_case(
            DiscoverReply::RpcError(-32601),
            crate::config::HandshakeMode::Discover,
        )
        .await;
        result.expect("fallback to initialize connects");
        assert_eq!(discover, 1, "exactly one discover probe");
        assert_eq!(initialize, 1, "exactly one initialize after fallback");
    }

    #[tokio::test]
    async fn discover_rejected_with_invalid_params_falls_back_exactly_once() {
        let (result, discover, initialize) = fallback_case(
            DiscoverReply::RpcError(-32602),
            crate::config::HandshakeMode::Auto,
        )
        .await;
        result.expect("fallback to initialize connects");
        assert_eq!(discover, 1);
        assert_eq!(initialize, 1);
    }

    #[tokio::test]
    async fn uncorrelated_discover_response_falls_back_exactly_once() {
        let (result, discover, initialize) = fallback_case(
            DiscoverReply::MismatchedId,
            crate::config::HandshakeMode::Auto,
        )
        .await;
        result.expect("fallback to initialize connects");
        assert_eq!(discover, 1);
        assert_eq!(initialize, 1);
    }

    #[tokio::test]
    async fn initialize_mode_never_falls_back() {
        let initialize_count = Arc::new(AtomicUsize::new(0));
        let server = RawHandshakeServer {
            discover_reply: DiscoverReply::RpcError(-32601),
            reject_initialize: true,
            discover_count: Arc::new(AtomicUsize::new(0)),
            initialize_count: initialize_count.clone(),
        };
        let url = server.spawn().await;
        let mut connection =
            fallback_test_connection(url, crate::config::HandshakeMode::Initialize);
        let error = connection.connect(None).await.unwrap_err();
        assert_eq!(error.code(), FailureCode::HandshakeIncompatible);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(initialize_count.load(Ordering::SeqCst), 1, "no retry");
    }
}
