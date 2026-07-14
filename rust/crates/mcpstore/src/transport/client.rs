use crate::auth::{AuthCoordinator, AuthStatus};
use crate::config::ServerConfig;
use crate::identity::InstanceId;
use crate::transport::{http as http_transport, stdio as stdio_transport};
use crate::transport::{Result, TransportError};

pub use crate::transport::pool::ConnectionPool;

use rmcp::service::{RoleClient, RunningService};

pub(super) type McpClient = RunningService<RoleClient, ()>;

enum ActiveClient {
    Stdio(McpClient),
    Http(McpClient),
}

pub struct McpConnection {
    instance_id: InstanceId,
    name: String,
    config: ServerConfig,
    client: Option<ActiveClient>,
    auth_coordinator: AuthCoordinator,
}

impl McpConnection {
    pub fn new(
        instance_id: InstanceId,
        name: String,
        config: ServerConfig,
        auth_coordinator: AuthCoordinator,
    ) -> Self {
        Self {
            instance_id,
            name,
            config,
            client: None,
            auth_coordinator,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    pub async fn connect(&mut self) -> Result<()> {
        let transport_type = self.config.infer_transport();
        tracing::info!(
            "[TRANSPORT] Connecting to service {} (transport={})",
            self.name,
            transport_type
        );

        match transport_type {
            "stdio" => self.connect_stdio().await,
            "streamable-http" | "http" => self.connect_http().await,
            "sse" => self.connect_http().await,
            other => Err(TransportError::ConnectionFailed(format!(
                "Unsupported transport type: {other}"
            ))),
        }
    }

    async fn connect_stdio(&mut self) -> Result<()> {
        let client = stdio_transport::connect(&self.name, &self.config).await?;
        tracing::info!("[TRANSPORT] stdio connected: {}", self.name);
        self.client = Some(ActiveClient::Stdio(client));
        Ok(())
    }

    async fn connect_http(&mut self) -> Result<()> {
        let client = http_transport::connect(
            self.instance_id,
            &self.name,
            &self.config,
            &self.auth_coordinator,
        )
        .await?;
        tracing::info!("[TRANSPORT] HTTP connected: {}", self.name);
        self.client = Some(ActiveClient::Http(client));
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(client) = self.client.take() {
            let inner = match client {
                ActiveClient::Stdio(c) => c,
                ActiveClient::Http(c) => c,
            };
            inner.cancel().await.ok();
            tracing::info!("[TRANSPORT] Disconnected: {}", self.name);
        }
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
