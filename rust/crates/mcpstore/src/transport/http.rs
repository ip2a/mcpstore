use crate::auth::{AuthCoordinator, AuthError};
use crate::config::ServerConfig;
use crate::identity::InstanceId;
use crate::transport::client::McpClient;
use crate::transport::handler::McpStoreClientHandler;
use crate::transport::oauth::McpStoreOAuthClient;
use crate::transport::{Result, TransportError};

use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;

/// Every connection failure funnels through here so the log always carries the
/// service and URL alongside the error.
fn connect_failed(service: &str, url: &str, error: String) -> TransportError {
    tracing::warn!(
        target: "mcpstore::transport::http",
        service,
        url,
        error = %error,
        "http connect failed"
    );
    TransportError::ConnectionFailed(error)
}

pub(super) async fn connect(
    instance_id: InstanceId,
    name: &str,
    server_config: &ServerConfig,
    auth_coordinator: &AuthCoordinator,
    handler: McpStoreClientHandler,
) -> Result<McpClient> {
    let config = server_config;
    let url = config.url.as_deref().ok_or_else(|| {
        connect_failed(name, "unset", format!("Service {name} missing url field"))
    })?;

    let mut custom_headers = std::collections::HashMap::new();
    for (key, value) in &config.headers {
        let header_name = ::http::HeaderName::from_bytes(key.as_bytes()).map_err(|err| {
            connect_failed(
                name,
                url,
                format!("Invalid HTTP header name '{key}': {err}"),
            )
        })?;
        let header_value = ::http::HeaderValue::from_str(value).map_err(|err| {
            connect_failed(
                name,
                url,
                format!("Invalid HTTP header value '{value}': {err}"),
            )
        })?;
        custom_headers.insert(header_name, header_value);
    }

    let transport_config = StreamableHttpClientTransportConfig::with_uri(url.to_string())
        .custom_headers(custom_headers);

    if server_config.auth.is_none() {
        let transport = StreamableHttpClientTransport::from_config(transport_config);
        return rmcp::service::serve_client_with_lifecycle(
            handler,
            transport,
            crate::transport::client_lifecycle_mode(server_config.handshake_mode()),
        )
        .await
        .map_err(|err| connect_failed(name, url, format!("HTTP MCP handshake failed: {err}")));
    }

    let authorization_manager = auth_coordinator
        .prepare_http_authorization(instance_id, url, &server_config.auth)
        .await
        .map_err(|error| match error {
            AuthError::Required(required) => TransportError::AuthRequired(required),
            other => connect_failed(
                name,
                url,
                format!("OAuth preparation failed for service {name}: {other}"),
            ),
        })?;
    let http_client = reqwest::Client::builder().build().map_err(|error| {
        connect_failed(
            name,
            url,
            format!("HTTP client initialization failed: {error}"),
        )
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
        crate::transport::client_lifecycle_mode(server_config.handshake_mode()),
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
            _ => Err(connect_failed(
                name,
                url,
                format!("HTTP MCP handshake failed: {error}"),
            )),
        },
    }
}
