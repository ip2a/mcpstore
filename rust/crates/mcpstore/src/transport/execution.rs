use std::time::Duration;

use rmcp::model::{
    CallToolRequest, CallToolRequestParams, CancelledNotificationParam, ClientRequest,
    ProgressNotificationParam, ServerResult,
};
use rmcp::service::{PeerRequestOptions, RequestHandle, RoleClient};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::auth::{AuthConfig, AuthCoordinator, AuthStatus};
use crate::identity::InstanceId;
use crate::transport::client::McpConnection;
use crate::transport::content::content_item_from_rmcp;
use crate::transport::{McpTask, McpToolExecution, Result, ToolCallResult, TransportError};

const EXECUTION_UPDATE_BUFFER: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct McpExecutionOptions {
    pub idle_timeout: Option<Duration>,
    pub max_total_timeout: Option<Duration>,
}

impl McpExecutionOptions {
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = Some(timeout);
        self
    }

    pub fn with_max_total_timeout(mut self, timeout: Duration) -> Self {
        self.max_total_timeout = Some(timeout);
        self
    }

    fn peer_options(self) -> PeerRequestOptions {
        let mut options = self
            .idle_timeout
            .map(PeerRequestOptions::with_timeout)
            .unwrap_or_else(PeerRequestOptions::no_options);
        if self.idle_timeout.is_some() {
            options = options.reset_timeout_on_progress();
        }
        if let Some(timeout) = self.max_total_timeout {
            options = options.with_max_total_timeout(timeout);
        }
        options
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpExecutionProgress {
    pub instance_id: InstanceId,
    pub progress_token: serde_json::Value,
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug)]
pub enum McpExecutionUpdate {
    Progress(McpExecutionProgress),
    Finished(Result<McpToolExecution>),
}

pub struct McpToolExecutionHandle {
    instance_id: InstanceId,
    request_id: serde_json::Value,
    progress_token: serde_json::Value,
    updates: mpsc::Receiver<McpExecutionUpdate>,
    cancellation: Option<oneshot::Sender<Option<String>>>,
}

impl std::fmt::Debug for McpToolExecutionHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpToolExecutionHandle")
            .field("instance_id", &self.instance_id)
            .field("request_id", &self.request_id)
            .field("progress_token", &self.progress_token)
            .finish_non_exhaustive()
    }
}

impl McpToolExecutionHandle {
    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    pub fn request_id(&self) -> &serde_json::Value {
        &self.request_id
    }

    pub fn progress_token(&self) -> &serde_json::Value {
        &self.progress_token
    }

    pub fn cancel(&mut self, reason: impl Into<String>) -> bool {
        self.cancellation
            .take()
            .is_some_and(|sender| sender.send(Some(reason.into())).is_ok())
    }

    pub async fn next_update(&mut self) -> Option<McpExecutionUpdate> {
        let update = self.updates.recv().await;
        if matches!(update, Some(McpExecutionUpdate::Finished(_))) {
            self.cancellation = None;
        }
        update
    }

    pub async fn wait(mut self) -> Result<McpToolExecution> {
        while let Some(update) = self.next_update().await {
            if let McpExecutionUpdate::Finished(result) = update {
                return result;
            }
        }
        Err(TransportError::Protocol(
            "tool execution ended without a result".to_string(),
        ))
    }
}

impl Drop for McpToolExecutionHandle {
    fn drop(&mut self) {
        if let Some(sender) = self.cancellation.take() {
            let _ = sender.send(Some("execution handle dropped".to_string()));
        }
    }
}

impl McpConnection {
    pub async fn start_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
        options: McpExecutionOptions,
    ) -> Result<McpToolExecutionHandle> {
        self.require_tools()?;
        let params = CallToolRequestParams::new(tool_name.to_string())
            .with_arguments(arguments_object(arguments));
        self.start_tool_request(params, options, false, "tool call")
            .await
    }

    pub(crate) async fn start_tool_request(
        &self,
        params: CallToolRequestParams,
        options: McpExecutionOptions,
        accepts_task: bool,
        operation: &'static str,
    ) -> Result<McpToolExecutionHandle> {
        let mut progress_notifications = self.subscribe_progress();
        let client = self.get_client()?;
        let request = match client
            .send_cancellable_request(
                ClientRequest::CallToolRequest(CallToolRequest::new(params)),
                options.peer_options(),
            )
            .await
        {
            Ok(request) => request,
            Err(error) => {
                let fallback = map_service_error(self.instance_id(), operation, error);
                return Err(self.classify_client_failure(fallback).await);
            }
        };

        let instance_id = self.instance_id();
        let request_id = request.id.clone();
        let progress_token = request.progress_token.clone();
        let request_id_value = request_id.clone().into_json_value();
        let progress_token_value = progress_token.0.clone().into_json_value();
        let (auth_coordinator, auth) = self.execution_auth();
        let (cancellation, cancellation_rx) = oneshot::channel();
        let (updates_tx, updates) = mpsc::channel(EXECUTION_UPDATE_BUFFER);

        tokio::spawn(async move {
            let result = drive_tool_request(
                instance_id,
                request,
                &mut progress_notifications,
                cancellation_rx,
                auth_coordinator,
                auth,
                accepts_task,
                operation,
                &updates_tx,
            )
            .await;
            let _ = updates_tx.send(McpExecutionUpdate::Finished(result)).await;
        });

        Ok(McpToolExecutionHandle {
            instance_id,
            request_id: request_id_value,
            progress_token: progress_token_value,
            updates,
            cancellation: Some(cancellation),
        })
    }
}

fn arguments_object(arguments: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    match arguments {
        serde_json::Value::Object(arguments) => arguments,
        _ => serde_json::Map::new(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn drive_tool_request(
    instance_id: InstanceId,
    request: RequestHandle<RoleClient>,
    progress_notifications: &mut broadcast::Receiver<ProgressNotificationParam>,
    mut cancellation: oneshot::Receiver<Option<String>>,
    auth_coordinator: AuthCoordinator,
    auth: AuthConfig,
    accepts_task: bool,
    operation: &'static str,
    updates: &mpsc::Sender<McpExecutionUpdate>,
) -> Result<McpToolExecution> {
    let request_id = request.id.clone();
    let progress_token = request.progress_token.clone();
    let peer = request.peer.clone();
    let mut response = Box::pin(request.await_response());
    let mut cancellation_open = true;
    let mut progress_open = true;
    let response = loop {
        tokio::select! {
            response = &mut response => break response,
            notification = progress_notifications.recv(), if progress_open => {
                match notification {
                    Ok(notification) if notification.progress_token == progress_token => {
                        let update = McpExecutionUpdate::Progress(McpExecutionProgress {
                            instance_id,
                            progress_token: notification.progress_token.0.into_json_value(),
                            progress: notification.progress,
                            total: notification.total,
                            message: notification.message,
                        });
                        if updates.try_send(update).is_err() {
                            tracing::warn!(%instance_id, "dropping MCP progress update because the execution consumer is lagging");
                        }
                    }
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => progress_open = false,
                }
            }
            reason = &mut cancellation, if cancellation_open => {
                cancellation_open = false;
                let reason = reason.ok().flatten();
                // `await_response` owns the rmcp RequestHandle. Sending the same typed
                // cancellation notification through its cloned Peer lets rmcp resolve the
                // pending response as ServiceError::Cancelled and clean up timeout watchers.
                peer.notify_cancelled(CancelledNotificationParam::new(
                    Some(request_id.clone()),
                    reason,
                ))
                .await
                .map_err(|error| map_service_error(instance_id, operation, error))?;
            }
        }
    };

    let response = match response {
        Ok(response) => response,
        Err(error) => {
            let fallback = map_service_error(instance_id, operation, error);
            return Err(
                classify_auth_failure(instance_id, &auth_coordinator, &auth, fallback).await,
            );
        }
    };
    execution_from_response(response, accepts_task, operation)
}

fn execution_from_response(
    response: ServerResult,
    accepts_task: bool,
    operation: &str,
) -> Result<McpToolExecution> {
    match response {
        ServerResult::CallToolResult(result) => {
            let content = result
                .content
                .into_iter()
                .map(content_item_from_rmcp)
                .collect::<Result<Vec<_>>>()?;
            Ok(McpToolExecution::Immediate {
                result: ToolCallResult {
                    content,
                    is_error: result.is_error.unwrap_or(false),
                },
            })
        }
        ServerResult::CreateTaskResult(result) if accepts_task => Ok(McpToolExecution::Task {
            task: McpTask::from(result.task),
        }),
        _ => Err(TransportError::Protocol(format!(
            "{operation} returned an unexpected response"
        ))),
    }
}

pub(crate) fn map_service_error(
    instance_id: InstanceId,
    operation: &str,
    error: rmcp::ServiceError,
) -> TransportError {
    match error {
        rmcp::ServiceError::Cancelled { reason } => TransportError::RequestCancelled { reason },
        rmcp::ServiceError::Timeout { timeout } => TransportError::RequestTimedOut { timeout },
        rmcp::ServiceError::TransportClosed | rmcp::ServiceError::TransportSend(_) => {
            TransportError::RequestDisconnected { instance_id }
        }
        rmcp::ServiceError::UnexpectedResponse => {
            TransportError::Protocol(format!("{operation} returned an unexpected response"))
        }
        error => TransportError::ToolCallFailed(format!("{operation} failed: {error}")),
    }
}

async fn classify_auth_failure(
    instance_id: InstanceId,
    auth_coordinator: &AuthCoordinator,
    auth: &AuthConfig,
    fallback: TransportError,
) -> TransportError {
    if auth.is_none()
        || matches!(
            fallback,
            TransportError::RequestCancelled { .. } | TransportError::RequestTimedOut { .. }
        )
    {
        return fallback;
    }
    match auth_coordinator.status(instance_id).await {
        AuthStatus::Unauthenticated => {
            TransportError::AuthRequired(auth_coordinator.auth_required(instance_id, auth))
        }
        AuthStatus::ScopeUpgradeRequired => TransportError::InsufficientScope {
            instance_id,
            required_scope: auth_coordinator.required_scope(instance_id).await,
        },
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::model::{
        CallToolRequestParams, CallToolResult, Implementation, ServerCapabilities, ServerInfo,
    };
    use rmcp::service::{RequestContext, RoleServer, RunningService};
    use rmcp::{ServerHandler, ServiceExt};
    use tokio::sync::Notify;

    use super::*;
    use crate::events::EventBus;
    use crate::identity::{ScopeRef, ServiceInstanceKey};
    use crate::registry::ServiceRegistry;
    use crate::transport::handler::McpStoreClientHandler;

    #[derive(Clone)]
    struct ExecutionFixtureState {
        started: Arc<Notify>,
        cancelled: Arc<Notify>,
    }

    impl Default for ExecutionFixtureState {
        fn default() -> Self {
            Self {
                started: Arc::new(Notify::new()),
                cancelled: Arc::new(Notify::new()),
            }
        }
    }

    #[derive(Clone)]
    struct ExecutionServer {
        label: String,
        state: ExecutionFixtureState,
    }

    impl ServerHandler for ExecutionServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
                .with_server_info(Implementation::new("execution-fixture", "1.0.0"))
        }

        async fn call_tool(
            &self,
            request: CallToolRequestParams,
            context: RequestContext<RoleServer>,
        ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
            match request.name.as_ref() {
                "progress" => {
                    let token = context.meta.get_progress_token().ok_or_else(|| {
                        rmcp::ErrorData::invalid_params("progress token is required", None)
                    })?;
                    context
                        .peer
                        .notify_progress(
                            ProgressNotificationParam::new(token, 1.0)
                                .with_total(2.0)
                                .with_message(self.label.clone()),
                        )
                        .await
                        .map_err(|error| {
                            rmcp::ErrorData::internal_error(error.to_string(), None)
                        })?;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    Ok(CallToolResult::success(Vec::new()))
                }
                "steady_progress" => {
                    let token = context.meta.get_progress_token().ok_or_else(|| {
                        rmcp::ErrorData::invalid_params("progress token is required", None)
                    })?;
                    for step in 1..=4 {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        context
                            .peer
                            .notify_progress(
                                ProgressNotificationParam::new(token.clone(), step as f64)
                                    .with_total(4.0),
                            )
                            .await
                            .map_err(|error| {
                                rmcp::ErrorData::internal_error(error.to_string(), None)
                            })?;
                    }
                    Ok(CallToolResult::success(Vec::new()))
                }
                "endless_progress" => {
                    let token = context.meta.get_progress_token().ok_or_else(|| {
                        rmcp::ErrorData::invalid_params("progress token is required", None)
                    })?;
                    self.state.started.notify_one();
                    let mut step = 0.0;
                    loop {
                        tokio::select! {
                            () = context.ct.cancelled() => {
                                self.state.cancelled.notify_one();
                                return Ok(CallToolResult::success(Vec::new()));
                            }
                            () = tokio::time::sleep(Duration::from_millis(10)) => {
                                step += 1.0;
                                context.peer.notify_progress(
                                    ProgressNotificationParam::new(token.clone(), step),
                                )
                                .await
                                .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
                            }
                        }
                    }
                }
                "blocked" => {
                    self.state.started.notify_one();
                    context.ct.cancelled().await;
                    self.state.cancelled.notify_one();
                    Ok(CallToolResult::success(Vec::new()))
                }
                "never" => {
                    self.state.started.notify_one();
                    std::future::pending::<()>().await;
                    unreachable!()
                }
                _ => Ok(CallToolResult::success(Vec::new())),
            }
        }
    }

    async fn connect_fixture(
        name: &str,
        label: &str,
    ) -> (
        McpConnection,
        RunningService<RoleServer, ExecutionServer>,
        ExecutionFixtureState,
    ) {
        let instance_id = ServiceInstanceKey::new(name, ScopeRef::Store).instance_id();
        let state = ExecutionFixtureState::default();
        let server_handler = ExecutionServer {
            label: label.to_string(),
            state: state.clone(),
        };
        let handler =
            McpStoreClientHandler::new(instance_id, ServiceRegistry::new(), EventBus::new());
        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server_start =
            tokio::spawn(async move { server_handler.serve(server_transport).await.unwrap() });
        let client = handler.clone().serve(client_transport).await.unwrap();
        let server = server_start.await.unwrap();
        (
            McpConnection::from_test_client(instance_id, client, handler),
            server,
            state,
        )
    }

    async fn collect_updates(
        mut handle: McpToolExecutionHandle,
    ) -> (Vec<McpExecutionProgress>, Result<McpToolExecution>) {
        let mut progress = Vec::new();
        while let Some(update) = handle.next_update().await {
            match update {
                McpExecutionUpdate::Progress(update) => progress.push(update),
                McpExecutionUpdate::Finished(result) => return (progress, result),
            }
        }
        (
            progress,
            Err(TransportError::Protocol(
                "fixture execution ended without a result".to_string(),
            )),
        )
    }

    #[tokio::test]
    async fn progress_is_correlated_by_instance_and_progress_token() {
        let (mut first, first_server, _) = connect_fixture("execution-first", "first").await;
        let (mut second, second_server, _) = connect_fixture("execution-second", "second").await;

        let first_handle = first
            .start_tool_call(
                "progress",
                serde_json::json!({}),
                McpExecutionOptions::default(),
            )
            .await
            .unwrap();
        let second_handle = second
            .start_tool_call(
                "progress",
                serde_json::json!({}),
                McpExecutionOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            first_handle.progress_token(),
            second_handle.progress_token()
        );

        let ((first_progress, first_result), (second_progress, second_result)) = tokio::join!(
            collect_updates(first_handle),
            collect_updates(second_handle)
        );
        assert!(first_result.is_ok());
        assert!(second_result.is_ok());
        assert_eq!(first_progress.len(), 1);
        assert_eq!(second_progress.len(), 1);
        assert_eq!(first_progress[0].instance_id, first.instance_id());
        assert_eq!(second_progress[0].instance_id, second.instance_id());
        assert_eq!(first_progress[0].message.as_deref(), Some("first"));
        assert_eq!(second_progress[0].message.as_deref(), Some("second"));

        first.disconnect().await.unwrap();
        second.disconnect().await.unwrap();
        first_server.cancel().await.unwrap();
        second_server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn matching_progress_resets_idle_timeout() {
        let (mut connection, server, _) = connect_fixture("execution-idle", "idle").await;
        let result = connection
            .start_tool_call(
                "steady_progress",
                serde_json::json!({}),
                McpExecutionOptions::default()
                    .with_idle_timeout(Duration::from_millis(35))
                    .with_max_total_timeout(Duration::from_millis(250)),
            )
            .await
            .unwrap()
            .wait()
            .await;
        assert!(result.is_ok());

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn idle_timeout_is_typed_and_cancels_the_server_request() {
        let (mut connection, server, state) =
            connect_fixture("execution-idle-timeout", "idle-timeout").await;
        let handle = connection
            .start_tool_call(
                "blocked",
                serde_json::json!({}),
                McpExecutionOptions::default().with_idle_timeout(Duration::from_millis(40)),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), state.started.notified())
            .await
            .unwrap();
        let error = handle.wait().await.unwrap_err();
        assert!(matches!(error, TransportError::RequestTimedOut { .. }));
        tokio::time::timeout(Duration::from_secs(1), state.cancelled.notified())
            .await
            .unwrap();

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn max_total_timeout_is_typed_and_cancels_the_server_request() {
        let (mut connection, server, state) = connect_fixture("execution-timeout", "timeout").await;
        let handle = connection
            .start_tool_call(
                "endless_progress",
                serde_json::json!({}),
                McpExecutionOptions::default()
                    .with_idle_timeout(Duration::from_millis(30))
                    .with_max_total_timeout(Duration::from_millis(80)),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), state.started.notified())
            .await
            .unwrap();
        let error = handle.wait().await.unwrap_err();
        assert!(matches!(error, TransportError::RequestTimedOut { .. }));
        tokio::time::timeout(Duration::from_secs(1), state.cancelled.notified())
            .await
            .unwrap();

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn closed_transport_is_reported_as_disconnected() {
        let (mut connection, server, state) =
            connect_fixture("execution-disconnected", "disconnected").await;
        let handle = connection
            .start_tool_call(
                "never",
                serde_json::json!({}),
                McpExecutionOptions::default(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), state.started.notified())
            .await
            .unwrap();
        server.cancel().await.unwrap();
        let error = tokio::time::timeout(Duration::from_secs(1), handle.wait())
            .await
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            error,
            TransportError::RequestDisconnected { instance_id }
                if instance_id == connection.instance_id()
        ));

        connection.disconnect().await.unwrap();
    }

    #[tokio::test]
    async fn explicit_cancel_uses_typed_rmcp_cancellation() {
        let (mut connection, server, state) = connect_fixture("execution-cancel", "cancel").await;
        let mut handle = connection
            .start_tool_call(
                "blocked",
                serde_json::json!({}),
                McpExecutionOptions::default(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), state.started.notified())
            .await
            .unwrap();
        assert!(handle.cancel("user interrupt"));
        let error = handle.wait().await.unwrap_err();
        assert!(matches!(
            error,
            TransportError::RequestCancelled {
                reason: Some(ref reason)
            } if reason == "user interrupt"
        ));
        tokio::time::timeout(Duration::from_secs(1), state.cancelled.notified())
            .await
            .unwrap();

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }
}
