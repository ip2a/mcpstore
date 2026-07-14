use std::collections::HashMap;
use std::sync::Arc;

use futures::stream::BoxStream;
use http::{HeaderName, HeaderValue};
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::auth::{AuthClient, AuthError as RmcpAuthError, AuthorizationManager};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
};
use sse_stream::{Error as SseError, Sse};
use tokio::sync::Mutex;

use crate::auth::{AuthConfig, AuthCoordinator};
use crate::identity::InstanceId;

/// OAuth-aware HTTP client used by MCPStore's rmcp Streamable HTTP transport.
///
/// rmcp supplies token acquisition and HTTP request behavior. This type owns the
/// MCPStore lifecycle policy around a rejected token: refresh once, replace the
/// rmcp authorization manager, and replay the rejected request at most once.
#[derive(Clone)]
pub(super) struct McpStoreOAuthClient {
    auth_client: AuthClient<reqwest::Client>,
    auth_coordinator: AuthCoordinator,
    instance_id: InstanceId,
    base_url: Arc<str>,
    auth: AuthConfig,
    refresh_lock: Arc<Mutex<()>>,
}

impl McpStoreOAuthClient {
    pub(super) fn new(
        http_client: reqwest::Client,
        authorization_manager: AuthorizationManager,
        auth_coordinator: AuthCoordinator,
        instance_id: InstanceId,
        base_url: impl Into<Arc<str>>,
        auth: AuthConfig,
    ) -> Self {
        Self {
            auth_client: AuthClient::new(http_client, authorization_manager),
            auth_coordinator,
            instance_id,
            base_url: base_url.into(),
            auth,
            refresh_lock: Arc::new(Mutex::new(())),
        }
    }

    async fn access_token(&self) -> Result<String, StreamableHttpError<reqwest::Error>> {
        match self.auth_client.get_access_token().await {
            Ok(token) => Ok(token),
            Err(error) => {
                if matches!(
                    error,
                    RmcpAuthError::AuthorizationRequired | RmcpAuthError::TokenRefreshFailed(_)
                ) {
                    self.auth_coordinator
                        .invalidate_authorization(self.instance_id, &self.base_url, &self.auth)
                        .await;
                }
                Err(StreamableHttpError::Auth(error))
            }
        }
    }

    async fn refresh_rejected_token(&self, rejected_token: &str) -> Option<String> {
        let _refresh_guard = self.refresh_lock.lock().await;

        // Another request may have refreshed while this request waited for the lock.
        if let Ok(current_token) = self.access_token().await {
            if current_token != rejected_token {
                return Some(current_token);
            }
        }

        let manager = self
            .auth_coordinator
            .refresh_http_authorization(self.instance_id, &self.base_url, &self.auth)
            .await
            .ok()?;
        *self.auth_client.auth_manager.lock().await = manager;
        self.access_token().await.ok()
    }

    async fn record_scope_failure(&self, required_scope: Option<&str>) {
        self.auth_coordinator
            .mark_scope_upgrade_required(self.instance_id, required_scope)
            .await;
    }

    async fn invalidate_rejected_token(&self) {
        self.auth_coordinator
            .invalidate_authorization(self.instance_id, &self.base_url, &self.auth)
            .await;
    }
}

impl StreamableHttpClient for McpStoreOAuthClient {
    type Error = reqwest::Error;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        _auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let token = self.access_token().await?;
        let first = self
            .auth_client
            .post_message(
                Arc::clone(&uri),
                message.clone(),
                session_id.clone(),
                Some(token.clone()),
                custom_headers.clone(),
            )
            .await;

        match first {
            Err(first_error @ StreamableHttpError::AuthRequired(_)) => {
                let Some(retry_token) = self.refresh_rejected_token(&token).await else {
                    return Err(first_error);
                };
                let retry = self
                    .auth_client
                    .post_message(uri, message, session_id, Some(retry_token), custom_headers)
                    .await;
                match &retry {
                    Err(StreamableHttpError::AuthRequired(_)) => {
                        self.invalidate_rejected_token().await;
                    }
                    Err(StreamableHttpError::InsufficientScope(error)) => {
                        self.record_scope_failure(error.get_required_scope()).await;
                    }
                    _ => {}
                }
                retry
            }
            Err(error @ StreamableHttpError::InsufficientScope(_)) => {
                if let StreamableHttpError::InsufficientScope(scope_error) = &error {
                    self.record_scope_failure(scope_error.get_required_scope())
                        .await;
                }
                Err(error)
            }
            other => other,
        }
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        _auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        let token = self.access_token().await?;
        let first = self
            .auth_client
            .delete_session(
                Arc::clone(&uri),
                Arc::clone(&session_id),
                Some(token.clone()),
                custom_headers.clone(),
            )
            .await;

        if !is_unauthorized(&first) {
            if is_forbidden(&first) {
                self.record_scope_failure(None).await;
            }
            return first;
        }

        let Some(retry_token) = self.refresh_rejected_token(&token).await else {
            return first;
        };
        let retry = self
            .auth_client
            .delete_session(uri, session_id, Some(retry_token), custom_headers)
            .await;
        if is_unauthorized(&retry) {
            self.invalidate_rejected_token().await;
        } else if is_forbidden(&retry) {
            self.record_scope_failure(None).await;
        }
        retry
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        _auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        let token = self.access_token().await?;
        let first = self
            .auth_client
            .get_stream(
                Arc::clone(&uri),
                Arc::clone(&session_id),
                last_event_id.clone(),
                Some(token.clone()),
                custom_headers.clone(),
            )
            .await;

        if !is_unauthorized(&first) {
            if is_forbidden(&first) {
                self.record_scope_failure(None).await;
            }
            return first;
        }

        let Some(retry_token) = self.refresh_rejected_token(&token).await else {
            return first;
        };
        let retry = self
            .auth_client
            .get_stream(
                uri,
                session_id,
                last_event_id,
                Some(retry_token),
                custom_headers,
            )
            .await;
        if is_unauthorized(&retry) {
            self.invalidate_rejected_token().await;
        } else if is_forbidden(&retry) {
            self.record_scope_failure(None).await;
        }
        retry
    }
}

fn is_unauthorized<T>(result: &Result<T, StreamableHttpError<reqwest::Error>>) -> bool {
    match result {
        Err(StreamableHttpError::AuthRequired(_)) => true,
        Err(StreamableHttpError::Client(error)) => {
            error.status() == Some(reqwest::StatusCode::UNAUTHORIZED)
        }
        _ => false,
    }
}

fn is_forbidden<T>(result: &Result<T, StreamableHttpError<reqwest::Error>>) -> bool {
    match result {
        Err(StreamableHttpError::InsufficientScope(_)) => true,
        Err(StreamableHttpError::Client(error)) => {
            error.status() == Some(reqwest::StatusCode::FORBIDDEN)
        }
        _ => false,
    }
}
