use crate::auth::{AuthCoordinator, AuthStatus};
use crate::config::ServerConfig;
use crate::events::EventBus;
use crate::health::supervisor::InstanceSupervisor;
use crate::identity::InstanceId;
use crate::registry::ServiceRegistry;
use crate::transport::handler::McpStoreClientHandler;
use crate::transport::stdio::StdioProcess;
use crate::transport::{http as http_transport, stdio as stdio_transport};
use crate::transport::{Result, TransportError};

pub use crate::transport::pool::ConnectionPool;

use rmcp::model::{ClientRequest, InitializeResult, PingRequest};
use rmcp::service::{RoleClient, RunningService};
use std::sync::Arc;

pub(super) type McpClient = RunningService<RoleClient, McpStoreClientHandler>;

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
    handler: McpStoreClientHandler,
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
            auth_coordinator: AuthCoordinator::new().expect("test auth coordinator"),
            handler,
        }
    }

    pub(crate) async fn connect(
        &mut self,
        supervisor: Option<Arc<InstanceSupervisor>>,
    ) -> Result<()> {
        let transport_type = self.config.infer_transport();
        tracing::info!(
            "[TRANSPORT] Connecting to service {} (transport={})",
            self.name,
            transport_type
        );

        match transport_type {
            "stdio" => self.connect_stdio(supervisor).await,
            "streamable-http" | "http" => self.connect_http().await,
            "sse" => self.connect_http().await,
            other => Err(TransportError::ConnectionFailed(format!(
                "Unsupported transport type: {other}"
            ))),
        }
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
        tracing::info!("[TRANSPORT] stdio connected: {}", self.name);
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
        tracing::info!("[TRANSPORT] HTTP connected: {}", self.name);
        self.client = Some(ActiveClient::Http(client));
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
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
            .map_err(|()| TransportError::ElicitationSessionActive {
                instance_id: self.instance_id,
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

    pub(in crate::transport) fn peer_info(&self) -> Result<Arc<InitializeResult>> {
        self.get_client()?.peer_info().ok_or_else(|| {
            TransportError::Protocol(format!(
                "MCP handshake metadata unavailable for {}",
                self.name
            ))
        })
    }

    pub async fn ping(&self, timeout: std::time::Duration) -> Result<()> {
        let client = self.get_client()?;
        tokio::time::timeout(
            timeout,
            client.send_request(ClientRequest::PingRequest(PingRequest::default())),
        )
        .await
        .map_err(|_| TransportError::RequestTimedOut { timeout })?
        .map_err(|error| TransportError::Protocol(format!("MCP ping failed: {error}")))?;
        Ok(())
    }

    pub(in crate::transport) fn get_client(&self) -> Result<&McpClient> {
        match &self.client {
            Some(ActiveClient::Stdio(c)) => Ok(c),
            Some(ActiveClient::Http(c)) => Ok(c),
            None => Err(TransportError::NotConnected(self.name.clone())),
        }
    }

    pub(in crate::transport) async fn classify_client_failure(
        &self,
        fallback: TransportError,
    ) -> TransportError {
        if self.config.auth.is_none() {
            return fallback;
        }
        match self.auth_coordinator.status(self.instance_id).await {
            AuthStatus::Unauthenticated => TransportError::AuthRequired(
                self.auth_coordinator
                    .auth_required(self.instance_id, &self.config.auth),
            ),
            AuthStatus::ScopeUpgradeRequired => TransportError::InsufficientScope {
                instance_id: self.instance_id,
                required_scope: self.auth_coordinator.required_scope(self.instance_id).await,
            },
            _ => fallback,
        }
    }
}
