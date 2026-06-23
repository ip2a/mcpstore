use crate::config::ServerConfig;
use crate::transport::client::McpClient;
use crate::transport::{Result, TransportError};

use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;

pub(super) async fn connect(name: &str, config: &ServerConfig) -> Result<McpClient> {
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

    let config = StreamableHttpClientTransportConfig::with_uri(url.to_string())
        .custom_headers(custom_headers);
    let transport = StreamableHttpClientTransport::from_config(config);

    rmcp::service::serve_client((), transport)
        .await
        .map_err(|err| {
            TransportError::ConnectionFailed(format!("HTTP MCP handshake failed: {err}"))
        })
}
