use crate::auth::{AuthCoordinator, AuthError};
use crate::config::ServerConfig;
use crate::identity::InstanceId;
use crate::transport::client::McpClient;
use crate::transport::handler::McpStoreClientHandler;
use crate::transport::oauth::McpStoreOAuthClient;
use crate::transport::{Result, TransportError};

use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;

pub(super) async fn connect(
    instance_id: InstanceId,
    name: &str,
    server_config: &ServerConfig,
    auth_coordinator: &AuthCoordinator,
    handler: McpStoreClientHandler,
) -> Result<McpClient> {
    let config = server_config;
    let url = config.url.as_deref().ok_or_else(|| {
        TransportError::ConnectionFailed(format!("Service {name} missing url field"))
    })?;

    let mut custom_headers = std::collections::HashMap::new();
    for (key, value) in &config.headers {
        let name = ::http::HeaderName::from_bytes(key.as_bytes()).map_err(|err| {
            TransportError::ConnectionFailed(format!("Invalid HTTP header name '{key}': {err}"))
        })?;
        let value = ::http::HeaderValue::from_str(value).map_err(|err| {
            TransportError::ConnectionFailed(format!("Invalid HTTP header value '{value}': {err}"))
        })?;
        custom_headers.insert(name, value);
    }

    let transport_config = StreamableHttpClientTransportConfig::with_uri(url.to_string())
        .custom_headers(custom_headers);

    if server_config.auth.is_none() {
        let transport = StreamableHttpClientTransport::from_config(transport_config);
        return rmcp::service::serve_client_with_lifecycle(
            handler,
            transport,
            rmcp::service::ClientLifecycleMode::Discover {
                preferred_versions: vec![rmcp::model::ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .map_err(|err| {
            TransportError::ConnectionFailed(format!("HTTP MCP handshake failed: {err}"))
        });
    }

    let authorization_manager = auth_coordinator
        .prepare_http_authorization(instance_id, url, &server_config.auth)
        .await
        .map_err(|error| match error {
            AuthError::Required(required) => TransportError::AuthRequired(required),
            other => TransportError::ConnectionFailed(format!(
                "OAuth preparation failed for service {name}: {other}"
            )),
        })?;
    let http_client = reqwest::Client::builder().build().map_err(|error| {
        TransportError::ConnectionFailed(format!("HTTP client initialization failed: {error}"))
    })?;
    let oauth_client = McpStoreOAuthClient::new(
        http_client,
        authorization_manager,
        auth_coordinator.clone(),
        instance_id,
        url,
        server_config.auth.clone(),
    );
    let transport = StreamableHttpClientTransport::with_client(oauth_client, transport_config);

    match rmcp::service::serve_client_with_lifecycle(
        handler,
        transport,
        rmcp::service::ClientLifecycleMode::Discover {
            preferred_versions: vec![rmcp::model::ProtocolVersion::V_2026_07_28],
        },
    )
    .await
    {
        Ok(client) => Ok(client),
        Err(error) => match auth_coordinator.status(instance_id).await {
            crate::auth::AuthStatus::Unauthenticated => Err(TransportError::AuthRequired(
                auth_coordinator.auth_required(instance_id, &server_config.auth),
            )),
            crate::auth::AuthStatus::ScopeUpgradeRequired => {
                Err(TransportError::InsufficientScope {
                    instance_id,
                    required_scope: auth_coordinator.required_scope(instance_id).await,
                })
            }
            _ => Err(TransportError::ConnectionFailed(format!(
                "HTTP MCP handshake failed: {error}"
            ))),
        },
    }
}
