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
            handler: McpStoreClientHandler::new(instance_id, registry, event_bus),
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
            subscription_task: None,
        }
    }

    pub(crate) async fn connect(
        &mut self,
        supervisor: Option<Arc<InstanceSupervisor>>,
    ) -> Result<()> {
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
}
