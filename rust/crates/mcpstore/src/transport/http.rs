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
        return rmcp::service::serve_client(handler, transport)
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

    match rmcp::service::serve_client(handler, transport).await {
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use axum::{
        body::Body,
        extract::{Form, Request, State},
        http::{header::WWW_AUTHENTICATE, HeaderValue, StatusCode},
        middleware::{self, Next},
        response::{IntoResponse, Response},
        routing::{get, post},
        Json, Router,
    };
    use rmcp::{
        model::{ServerCapabilities, ServerInfo},
        transport::streamable_http_server::{
            session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
        },
        ServerHandler,
    };

    use super::*;
    use crate::auth::test_support::test_keyring;
    use crate::auth::{
        AuthConfig, AuthStatus, ClientCredentialsAuthMethod, ClientSecret,
        OAuthClientCredentialsConfig,
    };

    #[derive(Clone)]
    struct EmptyServer;

    impl ServerHandler for EmptyServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo::new(ServerCapabilities::default())
        }
    }

    #[derive(Clone)]
    struct HeaderGate {
        accepted_requests: Arc<AtomicUsize>,
    }

    async fn require_static_headers(
        axum::extract::State(gate): axum::extract::State<HeaderGate>,
        request: Request<Body>,
        next: Next,
    ) -> Response {
        let headers = request.headers();
        let valid = headers.get("x-api-key") == Some(&HeaderValue::from_static("api-key-value"))
            && headers.get(::http::header::AUTHORIZATION)
                == Some(&HeaderValue::from_static("Bearer static-token"));
        if !valid {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        gate.accepted_requests.fetch_add(1, Ordering::SeqCst);
        next.run(request).await
    }

    fn test_handler(instance_id: InstanceId) -> McpStoreClientHandler {
        McpStoreClientHandler::new(
            instance_id,
            crate::registry::ServiceRegistry::new(),
            crate::events::EventBus::new(),
        )
    }

    #[tokio::test]
    async fn oauth_disabled_streamable_http_preserves_static_api_key_and_bearer_headers() {
        let accepted_requests = Arc::new(AtomicUsize::new(0));
        let gate = HeaderGate {
            accepted_requests: Arc::clone(&accepted_requests),
        };
        let service: StreamableHttpService<EmptyServer, LocalSessionManager> =
            StreamableHttpService::new(
                || Ok(EmptyServer),
                Default::default(),
                StreamableHttpServerConfig::default().with_sse_keep_alive(None),
            );
        let app = Router::new().nest_service("/mcp", service).route_layer(
            middleware::from_fn_with_state(gate.clone(), require_static_headers),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let config = ServerConfig {
            url: Some(format!("http://{address}/mcp")),
            headers: HashMap::from([
                ("x-api-key".to_string(), "api-key-value".to_string()),
                (
                    "authorization".to_string(),
                    "Bearer static-token".to_string(),
                ),
            ]),
            ..ServerConfig::default()
        };
        assert!(config.auth.is_none());
        let coordinator = AuthCoordinator::for_tests(
            crate::auth::SystemKeyring::new().unwrap(),
            crate::auth::test_state_manager(),
        )
        .unwrap();
        let client = connect(
            "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            "static-header-service",
            &config,
            &coordinator,
            test_handler("00000000-0000-0000-0000-000000000001".parse().unwrap()),
        )
        .await
        .unwrap();

        assert!(accepted_requests.load(Ordering::SeqCst) >= 2);
        client.cancel().await.unwrap();
        server.abort();
    }
    #[derive(Clone, Copy)]
    enum ProtectedResponse {
        AuthorizationCodeRefresh,
        AcceptRefreshedToken,
        AlwaysUnauthorized,
        RejectRefresh,
        InsufficientScope,
    }

    #[derive(Clone)]
    struct ProtectedFixtureState {
        issuer: String,
        response: ProtectedResponse,
        token_requests: Arc<AtomicUsize>,
        mcp_requests: Arc<AtomicUsize>,
        accepted_requests: Arc<AtomicUsize>,
    }

    struct ProtectedFixture {
        base_url: String,
        token_requests: Arc<AtomicUsize>,
        mcp_requests: Arc<AtomicUsize>,
        accepted_requests: Arc<AtomicUsize>,
        task: tokio::task::JoinHandle<()>,
    }

    impl Drop for ProtectedFixture {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    impl ProtectedFixture {
        async fn spawn(response: ProtectedResponse) -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base_url = format!("http://{}", listener.local_addr().unwrap());
            let token_requests = Arc::new(AtomicUsize::new(0));
            let mcp_requests = Arc::new(AtomicUsize::new(0));
            let accepted_requests = Arc::new(AtomicUsize::new(0));
            let state = ProtectedFixtureState {
                issuer: base_url.clone(),
                response,
                token_requests: Arc::clone(&token_requests),
                mcp_requests: Arc::clone(&mcp_requests),
                accepted_requests: Arc::clone(&accepted_requests),
            };
            let service: StreamableHttpService<EmptyServer, LocalSessionManager> =
                StreamableHttpService::new(
                    || Ok(EmptyServer),
                    Default::default(),
                    StreamableHttpServerConfig::default().with_sse_keep_alive(None),
                );
            let protected_mcp = Router::new()
                .nest_service("/mcp", service)
                .route_layer(middleware::from_fn_with_state(state.clone(), protect_mcp));
            let app = Router::new()
                .route(
                    "/.well-known/oauth-authorization-server/mcp",
                    get(protected_oauth_metadata),
                )
                .route("/token", post(protected_oauth_token))
                .merge(protected_mcp)
                .with_state(state);
            let task = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            Self {
                base_url,
                token_requests,
                mcp_requests,
                accepted_requests,
                task,
            }
        }

        fn mcp_url(&self) -> String {
            format!("{}/mcp", self.base_url)
        }
    }

    async fn protected_oauth_metadata(
        State(state): State<ProtectedFixtureState>,
    ) -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "issuer": state.issuer,
            "authorization_endpoint": format!("{}/authorize", state.issuer),
            "token_endpoint": format!("{}/token", state.issuer),
            "response_types_supported": ["code"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none", "client_secret_post"]
        }))
    }

    async fn protected_oauth_token(
        State(state): State<ProtectedFixtureState>,
        Form(form): Form<HashMap<String, String>>,
    ) -> (StatusCode, Json<serde_json::Value>) {
        let request_number = state.token_requests.fetch_add(1, Ordering::SeqCst) + 1;
        if matches!(state.response, ProtectedResponse::AuthorizationCodeRefresh) {
            let grant_type = form.get("grant_type").map(String::as_str);
            let valid = match grant_type {
                Some("authorization_code") => {
                    form.get("code").map(String::as_str) == Some("valid-code")
                        && form
                            .get("code_verifier")
                            .is_some_and(|value| !value.is_empty())
                }
                Some("refresh_token") => {
                    form.get("refresh_token").map(String::as_str) == Some("fixture-refresh-token")
                }
                _ => false,
            };
            if !valid {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid_grant"})),
                );
            }
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "access_token": if grant_type == Some("refresh_token") {
                        "refreshed-token"
                    } else {
                        "initial-token"
                    },
                    "token_type": "Bearer",
                    "expires_in": 3600,
                    "refresh_token": "fixture-refresh-token",
                    "scope": "tools.read"
                })),
            );
        }

        if form.get("grant_type").map(String::as_str) != Some("client_credentials")
            || form.get("client_id").map(String::as_str) != Some("machine-client")
            || form.get("client_secret").map(String::as_str) != Some("machine-secret")
        {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid_client"})),
            );
        }
        if matches!(state.response, ProtectedResponse::RejectRefresh) && request_number > 1 {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid_grant"})),
            );
        }
        let access_token = if request_number == 1 {
            "initial-token"
        } else {
            "refreshed-token"
        };
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "access_token": access_token,
                "token_type": "Bearer",
                "expires_in": 3600,
                "scope": "tools.call"
            })),
        )
    }

    async fn protect_mcp(
        State(state): State<ProtectedFixtureState>,
        request: Request<Body>,
        next: Next,
    ) -> Response {
        let token = request
            .headers()
            .get(::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        if token.is_some() {
            state.mcp_requests.fetch_add(1, Ordering::SeqCst);
        }
        let accepted = match state.response {
            ProtectedResponse::AuthorizationCodeRefresh
            | ProtectedResponse::AcceptRefreshedToken => token == Some("Bearer refreshed-token"),
            ProtectedResponse::AlwaysUnauthorized
            | ProtectedResponse::RejectRefresh
            | ProtectedResponse::InsufficientScope => false,
        };
        if accepted {
            state.accepted_requests.fetch_add(1, Ordering::SeqCst);
            return next.run(request).await;
        }

        let (status, challenge) = match state.response {
            ProtectedResponse::InsufficientScope => (
                StatusCode::FORBIDDEN,
                r#"Bearer error="insufficient_scope", scope="resources.read tools.call""#,
            ),
            _ => (StatusCode::UNAUTHORIZED, r#"Bearer error="invalid_token""#),
        };
        let mut response = status.into_response();
        response
            .headers_mut()
            .insert(WWW_AUTHENTICATE, HeaderValue::from_static(challenge));
        response
    }

    fn client_credentials_config() -> AuthConfig {
        AuthConfig::OAuthClientCredentials(OAuthClientCredentialsConfig {
            client_id: "machine-client".to_string(),
            scopes: vec!["tools.call".to_string()],
            resource: None,
            audience: None,
            credential_profile: None,
            client_auth_method: ClientCredentialsAuthMethod::ClientSecretPost,
            jwt_signing_algorithm: Default::default(),
        })
    }

    async fn protected_coordinator(
        fixture: &ProtectedFixture,
        instance_id: InstanceId,
    ) -> (AuthCoordinator, ServerConfig) {
        let auth = client_credentials_config();
        let state_manager = crate::auth::test_state_manager();
        state_manager
            .create(crate::state::ServiceState::new(
                instance_id,
                "test".to_string(),
                crate::identity::ScopeRef::Store,
                crate::state::DesiredState::Stopped,
                crate::state::AuthState::NotRequired,
                0,
            ))
            .await
            .unwrap();
        let coordinator = AuthCoordinator::for_tests(test_keyring(), state_manager).unwrap();
        coordinator
            .save_client_secret(
                instance_id,
                &fixture.mcp_url(),
                &auth,
                ClientSecret::new("machine-secret"),
            )
            .await
            .unwrap();
        let config = ServerConfig {
            url: Some(fixture.mcp_url()),
            auth,
            ..ServerConfig::default()
        };
        (coordinator, config)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn authorization_code_callback_connects_to_protected_mcp_and_refreshes_once() {
        let fixture = ProtectedFixture::spawn(ProtectedResponse::AuthorizationCodeRefresh).await;
        let instance_id = "00000000-0000-0000-0000-000000000010".parse().unwrap();
        let state_manager = crate::auth::test_state_manager();
        state_manager
            .create(crate::state::ServiceState::new(
                instance_id,
                "test".to_string(),
                crate::identity::ScopeRef::Store,
                crate::state::DesiredState::Stopped,
                crate::state::AuthState::NotRequired,
                0,
            ))
            .await
            .unwrap();
        let coordinator = AuthCoordinator::for_tests(test_keyring(), state_manager).unwrap();
        let auth: AuthConfig = serde_json::from_value(serde_json::json!({
            "type": "oauth_authorization_code",
            "client_id": "browser-client",
            "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
            "scopes": ["tools.read"]
        }))
        .unwrap();
        let authorization = coordinator
            .begin_authorization(instance_id, &fixture.mcp_url(), &auth)
            .await
            .unwrap();
        let state = reqwest::Url::parse(&authorization.authorization_url)
            .unwrap()
            .query_pairs()
            .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
            .unwrap();
        coordinator
            .complete_authorization_callback(
                instance_id,
                "valid-code",
                &state,
                Some(&fixture.base_url),
            )
            .await
            .unwrap();

        let config = ServerConfig {
            url: Some(fixture.mcp_url()),
            auth,
            ..ServerConfig::default()
        };
        let client = connect(
            instance_id,
            "protected-service",
            &config,
            &coordinator,
            test_handler(instance_id),
        )
        .await
        .unwrap();

        assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 2);
        assert_eq!(
            coordinator.status(instance_id).await,
            AuthStatus::Authenticated
        );
        assert!(fixture.mcp_requests.load(Ordering::SeqCst) >= 3);
        assert!(fixture.accepted_requests.load(Ordering::SeqCst) >= 2);
        client.cancel().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn rejected_access_token_is_refreshed_and_mcp_request_is_replayed_once() {
        let fixture = ProtectedFixture::spawn(ProtectedResponse::AcceptRefreshedToken).await;
        let instance_id = "00000000-0000-0000-0000-000000000011".parse().unwrap();
        let (coordinator, config) = protected_coordinator(&fixture, instance_id).await;

        let client = connect(
            instance_id,
            "protected-service",
            &config,
            &coordinator,
            test_handler(instance_id),
        )
        .await
        .unwrap();

        assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 2);
        assert_eq!(
            coordinator.status(instance_id).await,
            AuthStatus::Authenticated
        );
        assert!(fixture.mcp_requests.load(Ordering::SeqCst) >= 3);
        assert!(fixture.accepted_requests.load(Ordering::SeqCst) >= 2);
        client.cancel().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn repeated_unauthorized_response_stops_after_one_replay_and_invalidates_token() {
        let fixture = ProtectedFixture::spawn(ProtectedResponse::AlwaysUnauthorized).await;
        let instance_id = "00000000-0000-0000-0000-000000000012".parse().unwrap();
        let (coordinator, config) = protected_coordinator(&fixture, instance_id).await;

        let error = connect(
            instance_id,
            "protected-service",
            &config,
            &coordinator,
            test_handler(instance_id),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, TransportError::AuthRequired(_)));
        assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.mcp_requests.load(Ordering::SeqCst), 2);
        assert_eq!(
            coordinator.status(instance_id).await,
            AuthStatus::Unauthenticated
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn rejected_refresh_invalidates_authorization_without_replaying_again() {
        let fixture = ProtectedFixture::spawn(ProtectedResponse::RejectRefresh).await;
        let instance_id = "00000000-0000-0000-0000-000000000014".parse().unwrap();
        let (coordinator, config) = protected_coordinator(&fixture, instance_id).await;

        let error = connect(
            instance_id,
            "protected-service",
            &config,
            &coordinator,
            test_handler(instance_id),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, TransportError::AuthRequired(_)));
        assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.mcp_requests.load(Ordering::SeqCst), 1);
        assert_eq!(
            coordinator.status(instance_id).await,
            AuthStatus::Unauthenticated
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn insufficient_scope_enters_explicit_scope_upgrade_state_without_refresh() {
        let fixture = ProtectedFixture::spawn(ProtectedResponse::InsufficientScope).await;
        let instance_id = "00000000-0000-0000-0000-000000000013".parse().unwrap();
        let (coordinator, config) = protected_coordinator(&fixture, instance_id).await;

        let error = connect(
            instance_id,
            "protected-service",
            &config,
            &coordinator,
            test_handler(instance_id),
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            TransportError::InsufficientScope {
                instance_id: error_instance_id,
                required_scope: Some(ref scope),
            } if error_instance_id == instance_id && scope == "resources.read tools.call"
        ));
        assert_eq!(fixture.token_requests.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.mcp_requests.load(Ordering::SeqCst), 1);
        assert_eq!(
            coordinator.status(instance_id).await,
            AuthStatus::ScopeUpgradeRequired
        );
        assert_eq!(
            coordinator.required_scope(instance_id).await.as_deref(),
            Some("resources.read tools.call")
        );
    }
}
