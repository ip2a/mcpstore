use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use rmcp::model::{
    ArgumentInfo, ClientRequest, CompleteRequest, CompleteRequestParams, CompletionContext,
    CompletionInfo, InitializeResult, Reference, ServerResult, SubscribeRequest,
    SubscribeRequestParams, UnsubscribeRequest, UnsubscribeRequestParams,
};
#[allow(deprecated)]
use rmcp::model::{LoggingLevel, SetLevelRequest, SetLevelRequestParams};
use rmcp::service::{Peer, PeerRequestOptions, RoleClient};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::identity::InstanceId;
use crate::transport::client::McpConnection;
use crate::transport::execution::map_service_error;
use crate::transport::{Result, TransportError};

#[cfg(not(test))]
const PROTOCOL_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
const PROTOCOL_REQUEST_TIMEOUT: Duration = Duration::from_millis(100);

pub(crate) async fn send_protocol_request(
    peer: &Peer<RoleClient>,
    instance_id: InstanceId,
    request: ClientRequest,
    operation: &str,
) -> Result<ServerResult> {
    let handle = peer
        .send_cancellable_request(
            request,
            PeerRequestOptions::with_timeout(PROTOCOL_REQUEST_TIMEOUT),
        )
        .await
        .map_err(|error| map_service_error(instance_id, operation, error))?;
    handle
        .await_response()
        .await
        .map_err(|error| map_service_error(instance_id, operation, error))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpCompletionReference {
    Prompt { name: String },
    Resource { uri_template: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpCompletionRequest {
    pub reference: McpCompletionReference,
    pub argument_name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpCompletion {
    pub values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

impl From<CompletionInfo> for McpCompletion {
    fn from(value: CompletionInfo) -> Self {
        Self {
            values: value.values,
            total: value.total,
            has_more: value.has_more,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpLoggingLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerImplementation {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerCapabilities {
    pub tools: bool,
    pub tools_list_changed: bool,
    pub resources: bool,
    pub resources_subscribe: bool,
    pub resources_list_changed: bool,
    pub prompts: bool,
    pub prompts_list_changed: bool,
    pub completions: bool,
    pub logging: bool,
    pub tasks: bool,
    pub task_list: bool,
    pub task_cancel: bool,
    pub task_tool_calls: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub experimental: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerMetadata {
    pub protocol_version: String,
    pub server_info: McpServerImplementation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub capabilities: McpServerCapabilities,
}

impl From<&InitializeResult> for McpServerMetadata {
    fn from(info: &InitializeResult) -> Self {
        let capabilities = &info.capabilities;
        let task_capabilities = capabilities.tasks.as_ref();
        let task_tool_calls = task_capabilities
            .and_then(|tasks| tasks.requests.as_ref())
            .and_then(|requests| requests.tools.as_ref())
            .is_some();
        Self {
            protocol_version: info.protocol_version.to_string(),
            server_info: McpServerImplementation {
                name: info.server_info.name.clone(),
                title: info.server_info.title.clone(),
                version: info.server_info.version.clone(),
                description: info.server_info.description.clone(),
                website_url: info.server_info.website_url.clone(),
            },
            instructions: info.instructions.clone(),
            capabilities: McpServerCapabilities {
                tools: capabilities.tools.is_some(),
                tools_list_changed: capabilities
                    .tools
                    .as_ref()
                    .and_then(|value| value.list_changed)
                    == Some(true),
                resources: capabilities.resources.is_some(),
                resources_subscribe: capabilities
                    .resources
                    .as_ref()
                    .and_then(|value| value.subscribe)
                    == Some(true),
                resources_list_changed: capabilities
                    .resources
                    .as_ref()
                    .and_then(|value| value.list_changed)
                    == Some(true),
                prompts: capabilities.prompts.is_some(),
                prompts_list_changed: capabilities
                    .prompts
                    .as_ref()
                    .and_then(|value| value.list_changed)
                    == Some(true),
                completions: capabilities.completions.is_some(),
                logging: capabilities.logging.is_some(),
                tasks: task_capabilities.is_some(),
                task_list: task_capabilities
                    .and_then(|tasks| tasks.list.as_ref())
                    .is_some(),
                task_cancel: task_capabilities
                    .and_then(|tasks| tasks.cancel.as_ref())
                    .is_some(),
                task_tool_calls,
                extensions: capabilities
                    .extensions
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, value)| (name, Value::Object(value)))
                    .collect(),
                experimental: capabilities
                    .experimental
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, value)| (name, Value::Object(value)))
                    .collect(),
            },
        }
    }
}

impl McpConnection {
    pub fn server_metadata(&self) -> Result<McpServerMetadata> {
        let info = self.peer_info()?;
        Ok(McpServerMetadata::from(info.as_ref()))
    }

    pub async fn complete(&self, request: McpCompletionRequest) -> Result<McpCompletion> {
        self.require_capability("completions", |info| {
            info.capabilities.completions.is_some()
        })?;
        let context = (!request.context.is_empty())
            .then(|| CompletionContext::with_arguments(request.context));
        let reference = match request.reference {
            McpCompletionReference::Prompt { name } => Reference::for_prompt(name),
            McpCompletionReference::Resource { uri_template } => {
                Reference::for_resource(uri_template)
            }
        };
        let mut params = CompleteRequestParams::new(
            reference,
            ArgumentInfo::new(request.argument_name, request.value),
        );
        if let Some(context) = context {
            params = params.with_context(context);
        }
        match send_protocol_request(
            self.get_client()?,
            self.instance_id(),
            ClientRequest::CompleteRequest(CompleteRequest::new(params)),
            "completion",
        )
        .await
        {
            Ok(ServerResult::CompleteResult(result)) => Ok(result.completion.into()),
            Ok(_) => Err(TransportError::Protocol(
                "completion returned an unexpected response".to_string(),
            )),
            Err(error) => Err(self.classify_client_failure(error).await),
        }
    }

    pub async fn complete_prompt_argument(
        &self,
        prompt_name: impl Into<String>,
        argument_name: impl Into<String>,
        value: impl Into<String>,
        context: HashMap<String, String>,
    ) -> Result<McpCompletion> {
        self.complete(McpCompletionRequest {
            reference: McpCompletionReference::Prompt {
                name: prompt_name.into(),
            },
            argument_name: argument_name.into(),
            value: value.into(),
            context,
        })
        .await
    }

    pub async fn complete_resource_argument(
        &self,
        uri_template: impl Into<String>,
        argument_name: impl Into<String>,
        value: impl Into<String>,
        context: HashMap<String, String>,
    ) -> Result<McpCompletion> {
        self.complete(McpCompletionRequest {
            reference: McpCompletionReference::Resource {
                uri_template: uri_template.into(),
            },
            argument_name: argument_name.into(),
            value: value.into(),
            context,
        })
        .await
    }

    pub async fn subscribe_resource(&self, uri: &str) -> Result<()> {
        self.require_capability("resources.subscribe", |info| {
            info.capabilities
                .resources
                .as_ref()
                .and_then(|value| value.subscribe)
                == Some(true)
        })?;
        match send_protocol_request(
            self.get_client()?,
            self.instance_id(),
            ClientRequest::SubscribeRequest(SubscribeRequest::new(SubscribeRequestParams::new(
                uri,
            ))),
            "resource subscribe",
        )
        .await
        {
            Ok(ServerResult::EmptyResult(_)) => Ok(()),
            Ok(_) => Err(TransportError::Protocol(
                "resource subscribe returned an unexpected response".to_string(),
            )),
            Err(error) => Err(self.classify_client_failure(error).await),
        }
    }

    pub async fn unsubscribe_resource(&self, uri: &str) -> Result<()> {
        self.require_capability("resources.subscribe", |info| {
            info.capabilities
                .resources
                .as_ref()
                .and_then(|value| value.subscribe)
                == Some(true)
        })?;
        match send_protocol_request(
            self.get_client()?,
            self.instance_id(),
            ClientRequest::UnsubscribeRequest(UnsubscribeRequest::new(
                UnsubscribeRequestParams::new(uri),
            )),
            "resource unsubscribe",
        )
        .await
        {
            Ok(ServerResult::EmptyResult(_)) => Ok(()),
            Ok(_) => Err(TransportError::Protocol(
                "resource unsubscribe returned an unexpected response".to_string(),
            )),
            Err(error) => Err(self.classify_client_failure(error).await),
        }
    }

    #[allow(deprecated)]
    pub async fn set_logging_level(&self, level: McpLoggingLevel) -> Result<()> {
        self.require_capability("logging", |info| info.capabilities.logging.is_some())?;
        let level = match level {
            McpLoggingLevel::Debug => LoggingLevel::Debug,
            McpLoggingLevel::Info => LoggingLevel::Info,
            McpLoggingLevel::Notice => LoggingLevel::Notice,
            McpLoggingLevel::Warning => LoggingLevel::Warning,
            McpLoggingLevel::Error => LoggingLevel::Error,
            McpLoggingLevel::Critical => LoggingLevel::Critical,
            McpLoggingLevel::Alert => LoggingLevel::Alert,
            McpLoggingLevel::Emergency => LoggingLevel::Emergency,
        };
        match send_protocol_request(
            self.get_client()?,
            self.instance_id(),
            ClientRequest::SetLevelRequest(SetLevelRequest::new(SetLevelRequestParams::new(level))),
            "set logging level",
        )
        .await
        {
            Ok(ServerResult::EmptyResult(_)) => Ok(()),
            Ok(_) => Err(TransportError::Protocol(
                "set logging level returned an unexpected response".to_string(),
            )),
            Err(error) => Err(self.classify_client_failure(error).await),
        }
    }

    pub(in crate::transport) fn require_tools(&self) -> Result<()> {
        self.require_capability("tools", |info| info.capabilities.tools.is_some())
    }

    pub(in crate::transport) fn require_resources(&self) -> Result<()> {
        self.require_capability("resources", |info| info.capabilities.resources.is_some())
    }

    pub(in crate::transport) fn require_prompts(&self) -> Result<()> {
        self.require_capability("prompts", |info| info.capabilities.prompts.is_some())
    }

    pub(in crate::transport) fn require_capability(
        &self,
        capability: &'static str,
        supported: impl FnOnce(&InitializeResult) -> bool,
    ) -> Result<()> {
        let info = self.peer_info()?;
        if supported(info.as_ref()) {
            Ok(())
        } else {
            Err(TransportError::CapabilityUnsupported {
                instance_id: self.instance_id(),
                capability,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    #[allow(deprecated)]
    use rmcp::model::SetLevelRequestParams;
    use rmcp::model::{
        CallToolRequestParams, CallToolResult, CompleteRequestParams, CompleteResult,
        CreateTaskResult, GetPromptRequestParams, GetPromptResult, GetTaskParams,
        GetTaskPayloadParams, GetTaskPayloadResult, GetTaskResult, Implementation,
        ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult, ListTasksResult,
        ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult,
        ServerCapabilities, ServerInfo, Task, TaskStatus, TasksCapability, Tool,
    };
    use rmcp::service::{RequestContext, RoleServer, RunningService};
    use rmcp::{ServerHandler, ServiceExt};
    use tokio::sync::Mutex;

    use super::*;
    use crate::events::EventBus;
    use crate::identity::{ScopeRef, ServiceInstanceKey};
    use crate::registry::ServiceRegistry;
    use crate::transport::handler::McpStoreClientHandler;
    use crate::transport::{McpTask, McpTaskStatus, McpToolExecution, ToolCallResult};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ProtocolCall {
        Complete {
            reference: String,
            argument_name: String,
            value: String,
            context: HashMap<String, String>,
        },
        Subscribe(String),
        Unsubscribe(String),
        SetLevel(String),
        ListTools(Option<String>),
        CallTool,
        ListResources,
        ListResourceTemplates,
        ReadResource,
        ListPrompts,
        GetPrompt,
        TaskToolCall {
            name: String,
            arguments: Value,
            ttl: Option<u64>,
        },
        ListTasks(Option<String>),
        GetTask(String),
        GetTaskResult(String),
        CancelTask(String),
    }

    #[derive(Clone)]
    struct ProtocolServer {
        calls: Arc<Mutex<Vec<ProtocolCall>>>,
        capabilities: ServerCapabilities,
        complete_delay: Option<Duration>,
    }

    impl ProtocolServer {
        async fn record(&self, call: ProtocolCall) {
            self.calls.lock().await.push(call);
        }
    }

    impl ServerHandler for ProtocolServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo::new(self.capabilities.clone())
                .with_server_info(Implementation::new("protocol-fixture", "1.0.0"))
                .with_instructions("Use negotiated capabilities only")
        }

        async fn complete(
            &self,
            request: CompleteRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<CompleteResult, rmcp::ErrorData> {
            if let Some(delay) = self.complete_delay {
                tokio::time::sleep(delay).await;
            }
            let reference = request
                .r#ref
                .as_prompt_name()
                .map(|name| format!("prompt:{name}"))
                .or_else(|| {
                    request
                        .r#ref
                        .as_resource_uri()
                        .map(|uri| format!("resource:{uri}"))
                })
                .unwrap_or_else(|| "unknown".to_string());
            self.record(ProtocolCall::Complete {
                reference: reference.clone(),
                argument_name: request.argument.name,
                value: request.argument.value,
                context: request
                    .context
                    .and_then(|context| context.arguments)
                    .unwrap_or_default(),
            })
            .await;
            Ok(CompleteResult::new(
                CompletionInfo::with_all_values(vec![reference]).unwrap(),
            ))
        }

        async fn subscribe(
            &self,
            request: SubscribeRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<(), rmcp::ErrorData> {
            self.record(ProtocolCall::Subscribe(request.uri)).await;
            Ok(())
        }

        async fn unsubscribe(
            &self,
            request: UnsubscribeRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<(), rmcp::ErrorData> {
            self.record(ProtocolCall::Unsubscribe(request.uri)).await;
            Ok(())
        }

        #[allow(deprecated)]
        async fn set_level(
            &self,
            request: SetLevelRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<(), rmcp::ErrorData> {
            self.record(ProtocolCall::SetLevel(
                serde_json::to_value(request.level)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            ))
            .await;
            Ok(())
        }

        async fn list_tools(
            &self,
            request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<ListToolsResult, rmcp::ErrorData> {
            let cursor = request.and_then(|request| request.cursor);
            self.record(ProtocolCall::ListTools(cursor.clone())).await;
            let mut tool = Tool::default();
            tool.name = if cursor.is_some() {
                "second".into()
            } else {
                "first".into()
            };
            Ok(ListToolsResult {
                tools: vec![tool],
                next_cursor: cursor.is_none().then(|| "page-2".to_string()),
                ..Default::default()
            })
        }

        async fn call_tool(
            &self,
            _request: CallToolRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
            self.record(ProtocolCall::CallTool).await;
            Ok(CallToolResult::success(Vec::new()))
        }

        fn enqueue_task(
            &self,
            request: CallToolRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> impl std::future::Future<Output = std::result::Result<CreateTaskResult, rmcp::ErrorData>>
               + Send
               + '_ {
            let call = ProtocolCall::TaskToolCall {
                name: request.name.to_string(),
                arguments: Value::Object(request.arguments.unwrap_or_default()),
                ttl: request.task.and_then(|task| task.ttl),
            };
            let calls = self.calls.clone();
            async move {
                calls.lock().await.push(call);
                Ok(CreateTaskResult::new(fixture_task(
                    "task-created",
                    TaskStatus::Working,
                )))
            }
        }

        fn list_tasks(
            &self,
            request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> impl std::future::Future<Output = std::result::Result<ListTasksResult, rmcp::ErrorData>>
               + Send
               + '_ {
            let cursor = request.and_then(|request| request.cursor);
            let next_cursor = cursor.is_none().then(|| "page-2".to_string());
            let task = if cursor.is_none() {
                fixture_task("task-page-1", TaskStatus::Working)
            } else {
                fixture_task("task-page-2", TaskStatus::Completed)
            };
            let calls = self.calls.clone();
            async move {
                calls.lock().await.push(ProtocolCall::ListTasks(cursor));
                let mut result = ListTasksResult::new(vec![task]);
                result.next_cursor = next_cursor;
                Ok(result)
            }
        }

        fn get_task_info(
            &self,
            request: GetTaskParams,
            _context: RequestContext<RoleServer>,
        ) -> impl std::future::Future<Output = std::result::Result<GetTaskResult, rmcp::ErrorData>>
               + Send
               + '_ {
            let task_id = request.task_id;
            let calls = self.calls.clone();
            async move {
                calls
                    .lock()
                    .await
                    .push(ProtocolCall::GetTask(task_id.clone()));
                if task_id == "missing" {
                    return Err(rmcp::ErrorData::resource_not_found("task not found", None));
                }
                Ok(GetTaskResult::new(fixture_task(
                    &task_id,
                    TaskStatus::Working,
                )))
            }
        }

        fn get_task_result(
            &self,
            request: GetTaskPayloadParams,
            _context: RequestContext<RoleServer>,
        ) -> impl std::future::Future<
            Output = std::result::Result<GetTaskPayloadResult, rmcp::ErrorData>,
        > + Send
               + '_ {
            let task_id = request.task_id;
            let calls = self.calls.clone();
            async move {
                calls
                    .lock()
                    .await
                    .push(ProtocolCall::GetTaskResult(task_id.clone()));
                if task_id == "missing" {
                    return Err(rmcp::ErrorData::resource_not_found("task not found", None));
                }
                Ok(GetTaskPayloadResult::new(serde_json::json!({
                    "content": [{"type": "text", "text": "task result"}],
                    "isError": false
                })))
            }
        }

        fn cancel_task(
            &self,
            request: rmcp::model::CancelTaskParams,
            _context: RequestContext<RoleServer>,
        ) -> impl std::future::Future<
            Output = std::result::Result<rmcp::model::CancelTaskResult, rmcp::ErrorData>,
        > + Send
               + '_ {
            let task_id = request.task_id;
            let calls = self.calls.clone();
            async move {
                calls
                    .lock()
                    .await
                    .push(ProtocolCall::CancelTask(task_id.clone()));
                if task_id == "missing" {
                    return Err(rmcp::ErrorData::resource_not_found("task not found", None));
                }
                Ok(rmcp::model::CancelTaskResult::new(fixture_task(
                    &task_id,
                    TaskStatus::Cancelled,
                )))
            }
        }

        async fn list_resources(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<ListResourcesResult, rmcp::ErrorData> {
            self.record(ProtocolCall::ListResources).await;
            Ok(ListResourcesResult::default())
        }

        async fn list_resource_templates(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<ListResourceTemplatesResult, rmcp::ErrorData> {
            self.record(ProtocolCall::ListResourceTemplates).await;
            Ok(ListResourceTemplatesResult::default())
        }

        async fn read_resource(
            &self,
            _request: ReadResourceRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<ReadResourceResult, rmcp::ErrorData> {
            self.record(ProtocolCall::ReadResource).await;
            Ok(ReadResourceResult::new(Vec::new()))
        }

        async fn list_prompts(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<ListPromptsResult, rmcp::ErrorData> {
            self.record(ProtocolCall::ListPrompts).await;
            Ok(ListPromptsResult::default())
        }

        async fn get_prompt(
            &self,
            _request: GetPromptRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> std::result::Result<GetPromptResult, rmcp::ErrorData> {
            self.record(ProtocolCall::GetPrompt).await;
            Ok(GetPromptResult::new(Vec::new()))
        }
    }

    fn fixture_task(task_id: &str, status: TaskStatus) -> Task {
        Task::new(
            task_id.to_string(),
            status,
            "2026-07-15T00:00:00Z".to_string(),
            "2026-07-15T00:00:01Z".to_string(),
        )
        .with_status_message("fixture task")
        .with_ttl(60_000)
        .with_poll_interval(250)
    }

    async fn connect_fixture(
        capabilities: ServerCapabilities,
    ) -> (
        McpConnection,
        RunningService<RoleServer, ProtocolServer>,
        Arc<Mutex<Vec<ProtocolCall>>>,
    ) {
        let instance_id =
            ServiceInstanceKey::new("protocol-fixture", ScopeRef::Store).instance_id();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let server_handler = ProtocolServer {
            calls: calls.clone(),
            capabilities,
            complete_delay: None,
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
            calls,
        )
    }

    async fn connect_slow_completion_fixture(
    ) -> (McpConnection, RunningService<RoleServer, ProtocolServer>) {
        let instance_id =
            ServiceInstanceKey::new("slow-protocol-fixture", ScopeRef::Store).instance_id();
        let server_handler = ProtocolServer {
            calls: Arc::new(Mutex::new(Vec::new())),
            capabilities: ServerCapabilities::builder().enable_completions().build(),
            complete_delay: Some(Duration::from_secs(1)),
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
        )
    }

    #[tokio::test]
    async fn ordinary_protocol_requests_timeout_through_rmcp_request_handle() {
        let (mut connection, server) = connect_slow_completion_fixture().await;
        let error = connection
            .complete_prompt_argument("review", "language", "r", HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            TransportError::RequestTimedOut { timeout }
                if timeout == PROTOCOL_REQUEST_TIMEOUT
        ));
        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    fn assert_unsupported<T>(result: Result<T>, expected: &'static str) {
        match result {
            Err(TransportError::CapabilityUnsupported { capability, .. }) => {
                assert_eq!(capability, expected)
            }
            Err(error) => panic!("expected unsupported {expected}, got {error}"),
            Ok(_) => panic!("expected unsupported {expected}, got success"),
        }
    }

    #[test]
    fn metadata_preserves_negotiated_server_capabilities() {
        let info = ServerInfo::new(
            ServerCapabilities::builder()
                .enable_completions()
                .enable_resources()
                .enable_resources_subscribe()
                .enable_resources_list_changed()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
        )
        .with_server_info(
            Implementation::new("fixture", "1.2.3")
                .with_title("Fixture server")
                .with_description("protocol fixture")
                .with_website_url("https://example.invalid"),
        )
        .with_instructions("Use negotiated capabilities only");

        let metadata = McpServerMetadata::from(&info);

        assert_eq!(metadata.server_info.name, "fixture");
        assert_eq!(metadata.protocol_version, info.protocol_version.to_string());
        assert!(metadata.capabilities.completions);
        assert!(metadata.capabilities.resources_subscribe);
        assert!(metadata.capabilities.resources_list_changed);
        assert!(metadata.capabilities.tools_list_changed);
        assert!(!metadata.capabilities.prompts);
        assert!(!metadata.capabilities.logging);
        assert!(!metadata.capabilities.tasks);
    }

    #[test]
    fn completion_request_uses_stable_domain_shape() {
        let request = McpCompletionRequest {
            reference: McpCompletionReference::Resource {
                uri_template: "repo://{owner}/{name}".to_string(),
            },
            argument_name: "owner".to_string(),
            value: "mc".to_string(),
            context: HashMap::from([("name".to_string(), "store".to_string())]),
        };

        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["reference"]["type"], "resource");
        assert_eq!(value["reference"]["uri_template"], "repo://{owner}/{name}");
        assert_eq!(value["argumentName"], "owner");
        assert_eq!(value["context"]["name"], "store");
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn native_operations_use_negotiated_rmcp_protocol() {
        let capabilities = ServerCapabilities::builder()
            .enable_completions()
            .enable_resources()
            .enable_resources_subscribe()
            .enable_resources_list_changed()
            .enable_tools()
            .enable_prompts()
            .enable_logging()
            .build();
        let (mut connection, server, calls) = connect_fixture(capabilities).await;

        let metadata = connection.server_metadata().unwrap();
        assert_eq!(metadata.server_info.name, "protocol-fixture");
        assert_eq!(metadata.server_info.version, "1.0.0");
        assert_eq!(
            metadata.instructions.as_deref(),
            Some("Use negotiated capabilities only")
        );
        assert!(metadata.capabilities.completions);
        assert!(metadata.capabilities.resources_subscribe);
        assert!(metadata.capabilities.logging);

        let prompt = connection
            .complete_prompt_argument(
                "review",
                "language",
                "ru",
                HashMap::from([("repository".to_string(), "mcpstore".to_string())]),
            )
            .await
            .unwrap();
        assert_eq!(prompt.values, vec!["prompt:review"]);
        assert_eq!(prompt.total, Some(1));
        assert_eq!(prompt.has_more, Some(false));

        let resource = connection
            .complete_resource_argument(
                "repo://{owner}/{name}",
                "owner",
                "mc",
                HashMap::from([("name".to_string(), "store".to_string())]),
            )
            .await
            .unwrap();
        assert_eq!(resource.values, vec!["resource:repo://{owner}/{name}"]);

        connection
            .subscribe_resource("repo://mcp/store")
            .await
            .unwrap();
        connection
            .unsubscribe_resource("repo://mcp/store")
            .await
            .unwrap();
        connection
            .set_logging_level(McpLoggingLevel::Info)
            .await
            .unwrap();

        assert_eq!(
            calls.lock().await.as_slice(),
            [
                ProtocolCall::Complete {
                    reference: "prompt:review".to_string(),
                    argument_name: "language".to_string(),
                    value: "ru".to_string(),
                    context: HashMap::from([("repository".to_string(), "mcpstore".to_string())]),
                },
                ProtocolCall::Complete {
                    reference: "resource:repo://{owner}/{name}".to_string(),
                    argument_name: "owner".to_string(),
                    value: "mc".to_string(),
                    context: HashMap::from([("name".to_string(), "store".to_string())]),
                },
                ProtocolCall::Subscribe("repo://mcp/store".to_string()),
                ProtocolCall::Unsubscribe("repo://mcp/store".to_string()),
                ProtocolCall::SetLevel("info".to_string()),
            ]
        );

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn prompt_arguments_must_be_an_object_before_request_is_sent() {
        let capabilities = ServerCapabilities::builder().enable_prompts().build();
        let (mut connection, server, calls) = connect_fixture(capabilities).await;

        let error = connection
            .get_prompt("review", serde_json::json!(["invalid"]))
            .await
            .unwrap_err();
        assert!(matches!(error, TransportError::InvalidInput(_)));
        assert!(calls.lock().await.is_empty());

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn list_tools_fetches_all_pages() {
        let capabilities = ServerCapabilities::builder().enable_tools().build();
        let (mut connection, server, calls) = connect_fixture(capabilities).await;

        let tools = connection.list_tools().await.unwrap();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(
            calls.lock().await.as_slice(),
            [
                ProtocolCall::ListTools(None),
                ProtocolCall::ListTools(Some("page-2".to_string())),
            ]
        );

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn task_operations_use_typed_rmcp_requests_and_preserve_lifecycle_data() {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_tasks_with(TasksCapability::server_default())
            .build();
        let (mut connection, server, calls) = connect_fixture(capabilities).await;

        let execution = connection
            .call_tool_task(
                "long_tool",
                serde_json::json!({"input": "value"}),
                Some(5_000),
            )
            .await
            .unwrap();
        assert_eq!(
            execution,
            McpToolExecution::Task {
                task: McpTask {
                    task_id: "task-created".to_string(),
                    status: McpTaskStatus::Working,
                    status_message: Some("fixture task".to_string()),
                    created_at: "2026-07-15T00:00:00Z".to_string(),
                    last_updated_at: "2026-07-15T00:00:01Z".to_string(),
                    ttl: Some(60_000),
                    poll_interval: Some(250),
                },
            }
        );

        let immediate = connection
            .call_tool("regular_tool", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(
            immediate,
            ToolCallResult {
                content: Vec::new(),
                is_error: false,
            }
        );

        let tasks = connection.list_tasks().await.unwrap();
        assert_eq!(
            tasks
                .iter()
                .map(|task| task.task_id.as_str())
                .collect::<Vec<_>>(),
            ["task-page-1", "task-page-2"]
        );
        assert_eq!(tasks[0].status, McpTaskStatus::Working);
        assert_eq!(tasks[1].status, McpTaskStatus::Completed);

        let task = connection.get_task("task-page-1").await.unwrap();
        assert_eq!(task.task_id, "task-page-1");
        assert_eq!(task.status, McpTaskStatus::Working);

        let result = connection.get_task_result("task-page-1").await.unwrap();
        assert_eq!(result["content"][0]["text"], "task result");
        assert_eq!(result["isError"], false);

        let cancelled = connection.cancel_task("task-page-1").await.unwrap();
        assert_eq!(cancelled.task_id, "task-page-1");
        assert_eq!(cancelled.status, McpTaskStatus::Cancelled);

        assert_eq!(
            calls.lock().await.as_slice(),
            [
                ProtocolCall::TaskToolCall {
                    name: "long_tool".to_string(),
                    arguments: serde_json::json!({"input": "value"}),
                    ttl: Some(5_000),
                },
                ProtocolCall::CallTool,
                ProtocolCall::ListTasks(None),
                ProtocolCall::ListTasks(Some("page-2".to_string())),
                ProtocolCall::GetTask("task-page-1".to_string()),
                ProtocolCall::GetTaskResult("task-page-1".to_string()),
                ProtocolCall::CancelTask("task-page-1".to_string()),
            ]
        );

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn task_resource_errors_are_classified_without_parsing_messages() {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_tasks_with(TasksCapability::server_default())
            .build();
        let (mut connection, server, _) = connect_fixture(capabilities).await;

        for error in [
            connection.get_task("missing").await.unwrap_err(),
            connection.get_task_result("missing").await.unwrap_err(),
            connection.cancel_task("missing").await.unwrap_err(),
        ] {
            assert!(matches!(
                error,
                TransportError::TaskNotFound { ref task_id } if task_id == "missing"
            ));
        }

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn unsupported_task_capabilities_fail_before_requests_are_sent() {
        let (mut connection, server, calls) = connect_fixture(ServerCapabilities::default()).await;

        assert_unsupported(
            connection
                .call_tool_task("long_tool", serde_json::json!({}), None)
                .await,
            "tasks.requests.tools",
        );
        assert_unsupported(connection.list_tasks().await, "tasks.list");
        assert_unsupported(connection.get_task("task-1").await, "tasks");
        assert_unsupported(connection.get_task_result("task-1").await, "tasks");
        assert_unsupported(connection.cancel_task("task-1").await, "tasks.cancel");
        assert!(calls.lock().await.is_empty());

        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn unsupported_capabilities_fail_before_requests_are_sent() {
        let (mut connection, server, calls) = connect_fixture(ServerCapabilities::default()).await;

        assert_unsupported(
            connection
                .complete_prompt_argument("review", "language", "r", HashMap::new())
                .await,
            "completions",
        );
        assert_unsupported(
            connection.subscribe_resource("repo://mcp/store").await,
            "resources.subscribe",
        );
        assert_unsupported(
            connection.unsubscribe_resource("repo://mcp/store").await,
            "resources.subscribe",
        );
        assert_unsupported(
            connection.set_logging_level(McpLoggingLevel::Warning).await,
            "logging",
        );
        assert_unsupported(connection.list_tools().await, "tools");
        assert_unsupported(
            connection.call_tool("review", serde_json::json!({})).await,
            "tools",
        );
        assert_unsupported(connection.list_resources().await, "resources");
        assert_unsupported(connection.list_resource_templates().await, "resources");
        assert_unsupported(
            connection.read_resource("repo://mcp/store").await,
            "resources",
        );
        assert_unsupported(connection.list_prompts().await, "prompts");
        assert_unsupported(
            connection.get_prompt("review", serde_json::json!({})).await,
            "prompts",
        );

        assert!(calls.lock().await.is_empty());
        connection.disconnect().await.unwrap();
        server.cancel().await.unwrap();
    }
}
