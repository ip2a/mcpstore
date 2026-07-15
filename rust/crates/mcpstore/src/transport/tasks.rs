use rmcp::model::{
    CallToolRequest, CallToolRequestParams, CancelTaskParams, CancelTaskRequest, ClientRequest,
    GetTaskParams, GetTaskPayloadParams, GetTaskPayloadRequest, GetTaskRequest, ListTasksRequest,
    PaginatedRequestParams, ServerResult, Task as RmcpTask, TaskMetadata, TaskStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::transport::client::McpConnection;
use crate::transport::content::content_item_from_rmcp;
use crate::transport::{Result, ToolCallResult, TransportError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpTaskStatus {
    Working,
    InputRequired,
    Completed,
    Failed,
    Cancelled,
    Expired,
    Disconnected,
    Unknown,
}

impl From<TaskStatus> for McpTaskStatus {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Working => Self::Working,
            TaskStatus::InputRequired => Self::InputRequired,
            TaskStatus::Completed => Self::Completed,
            TaskStatus::Failed => Self::Failed,
            TaskStatus::Cancelled => Self::Cancelled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpTask {
    pub task_id: String,
    pub status: McpTaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    pub created_at: String,
    pub last_updated_at: String,
    pub ttl: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

impl From<RmcpTask> for McpTask {
    fn from(task: RmcpTask) -> Self {
        Self {
            task_id: task.task_id,
            status: task.status.into(),
            status_message: task.status_message,
            created_at: task.created_at,
            last_updated_at: task.last_updated_at,
            ttl: task.ttl,
            poll_interval: task.poll_interval,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpToolExecution {
    Immediate { result: ToolCallResult },
    Task { task: McpTask },
}

impl McpConnection {
    pub async fn call_tool_task(
        &self,
        tool_name: &str,
        arguments: Value,
        ttl: Option<u64>,
    ) -> Result<McpToolExecution> {
        self.require_capability("tasks.requests.tools", |info| {
            info.capabilities
                .tasks
                .as_ref()
                .is_some_and(|tasks| tasks.supports_tools_call())
        })?;
        let client = self.get_client()?;
        let arguments = match arguments {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        let task = ttl.map_or_else(TaskMetadata::new, |ttl| TaskMetadata::new().with_ttl(ttl));
        let params = CallToolRequestParams::new(tool_name.to_string())
            .with_arguments(arguments)
            .with_task(task);
        let response = client
            .send_request(ClientRequest::CallToolRequest(CallToolRequest::new(params)))
            .await
            .map_err(|error| self.protocol_error("task tool call", error))?;

        match response {
            ServerResult::CreateTaskResult(result) => Ok(McpToolExecution::Task {
                task: result.task.into(),
            }),
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
            _ => Err(TransportError::Protocol(
                "task tool call returned an unexpected response".to_string(),
            )),
        }
    }

    pub async fn list_tasks(&self) -> Result<Vec<McpTask>> {
        self.require_capability("tasks.list", |info| {
            info.capabilities
                .tasks
                .as_ref()
                .is_some_and(|tasks| tasks.supports_list())
        })?;
        let client = self.get_client()?;
        let mut cursor = None;
        let mut tasks = Vec::new();
        loop {
            let response = client
                .send_request(ClientRequest::ListTasksRequest(
                    ListTasksRequest::with_param(
                        PaginatedRequestParams::default().with_cursor(cursor),
                    ),
                ))
                .await
                .map_err(|error| self.protocol_error("list tasks", error))?;
            let ServerResult::ListTasksResult(result) = response else {
                return Err(TransportError::Protocol(
                    "list tasks returned an unexpected response".to_string(),
                ));
            };
            tasks.extend(result.tasks.into_iter().map(McpTask::from));
            cursor = result.next_cursor;
            if cursor.is_none() {
                return Ok(tasks);
            }
        }
    }

    pub async fn get_task(&self, task_id: &str) -> Result<McpTask> {
        self.require_capability("tasks", |info| info.capabilities.tasks.is_some())?;
        let client = self.get_client()?;
        let response = client
            .send_request(ClientRequest::GetTaskRequest(GetTaskRequest::new(
                GetTaskParams::new(task_id),
            )))
            .await
            .map_err(|error| self.protocol_error("get task", error))?;
        match response {
            ServerResult::GetTaskResult(result) => Ok(result.task.into()),
            _ => Err(TransportError::Protocol(
                "get task returned an unexpected response".to_string(),
            )),
        }
    }

    pub async fn get_task_result(&self, task_id: &str) -> Result<Value> {
        self.require_capability("tasks", |info| info.capabilities.tasks.is_some())?;
        let client = self.get_client()?;
        let response = client
            .send_request(ClientRequest::GetTaskPayloadRequest(
                GetTaskPayloadRequest::new(GetTaskPayloadParams::new(task_id)),
            ))
            .await
            .map_err(|error| self.protocol_error("get task result", error))?;
        match response {
            ServerResult::GetTaskPayloadResult(result) => Ok(result.0),
            // rmcp 2.2.0 intentionally deserializes the wire shape of
            // `tasks/result` as `CustomResult`, because it is indistinguishable
            // from `GetTaskPayloadResult` on the wire.
            ServerResult::CustomResult(result) => Ok(result.0),
            ServerResult::CallToolResult(result) => serde_json::to_value(result).map_err(|error| {
                TransportError::Protocol(format!("failed to encode task result: {error}"))
            }),
            _ => Err(TransportError::Protocol(
                "get task result returned an unexpected response".to_string(),
            )),
        }
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<McpTask> {
        self.require_capability("tasks.cancel", |info| {
            info.capabilities
                .tasks
                .as_ref()
                .is_some_and(|tasks| tasks.supports_cancel())
        })?;
        let client = self.get_client()?;
        let response = client
            .send_request(ClientRequest::CancelTaskRequest(CancelTaskRequest::new(
                CancelTaskParams::new(task_id),
            )))
            .await
            .map_err(|error| self.protocol_error("cancel task", error))?;
        match response {
            ServerResult::CancelTaskResult(result) => Ok(result.task.into()),
            // `CancelTaskResult` and `GetTaskResult` have the same wire shape;
            // rmcp 2.2.0 resolves the deserialized response to `GetTaskResult`.
            ServerResult::GetTaskResult(result) => Ok(result.task.into()),
            _ => Err(TransportError::Protocol(
                "cancel task returned an unexpected response".to_string(),
            )),
        }
    }

    fn protocol_error(&self, operation: &str, error: rmcp::ServiceError) -> TransportError {
        TransportError::Protocol(format!("{operation} failed: {error}"))
    }
}
