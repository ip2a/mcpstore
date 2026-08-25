use rmcp::model::{
    CallToolRequestParams, CancelTaskParams, DetailedTask, ErrorCode, GetTaskParams,
    Task as RmcpTask, TaskPayload, TaskStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;
use crate::error::{Error, ErrorContext, FailureCode};
use crate::transport::client::McpConnection;
use crate::transport::{McpExecutionOptions, ToolCallResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpTaskStatus {
    Working,
    InputRequired,
    Completed,
    Failed,
    Cancelled,
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
    pub ttl_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval_ms: Option<u64>,
}

impl From<RmcpTask> for McpTask {
    fn from(task: RmcpTask) -> Self {
        Self {
            task_id: task.task_id,
            status: task.status.into(),
            status_message: task.status_message,
            created_at: task.created_at,
            last_updated_at: task.last_updated_at,
            ttl_ms: task.ttl_ms,
            poll_interval_ms: task.poll_interval_ms,
        }
    }
}

impl From<DetailedTask> for McpTask {
    fn from(task: DetailedTask) -> Self {
        task.task.into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpToolExecution {
    Immediate { result: ToolCallResult },
    Task { task: McpTask },
}

impl McpConnection {
    pub async fn start_tool_task(
        &self,
        tool_name: &str,
        arguments: Value,
        meta: Option<rmcp::model::RequestMetaObject>,
        options: McpExecutionOptions,
    ) -> Result<crate::transport::McpToolExecutionHandle> {
        self.require_capability("io.modelcontextprotocol/tasks", |info| {
            info.capabilities.supports_tasks()
        })?;
        let arguments = match arguments {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        let mut params =
            CallToolRequestParams::new(tool_name.to_string()).with_arguments(arguments);
        params.meta = meta;
        self.start_tool_request(params, options, true, "task tool call")
            .await
    }

    pub async fn call_tool_task(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<McpToolExecution> {
        self.start_tool_task(tool_name, arguments, None, McpExecutionOptions::default())
            .await?
            .wait()
            .await
    }

    pub async fn get_task(&self, task_id: &str) -> Result<McpTask> {
        Ok(self.get_detailed_task(task_id).await?.into())
    }

    pub async fn get_task_result(&self, task_id: &str) -> Result<Value> {
        let task = self.get_detailed_task(task_id).await?;
        match task.payload {
            TaskPayload::Completed { result } => Ok(Value::Object(result)),
            TaskPayload::Failed { error } => Err(Error::new(
                FailureCode::TaskFailed,
                format!("task {task_id} failed: {}", Value::Object(error)),
            )),
            TaskPayload::Cancelled => Err(Error::new(
                FailureCode::TaskFailed,
                format!("task {task_id} was cancelled"),
            )),
            TaskPayload::Working | TaskPayload::InputRequired { .. } => Err(Error::new(
                FailureCode::TaskUnavailable,
                format!("task {task_id} result is not available"),
            )),
            _ => Err(Error::new(
                FailureCode::TaskFailed,
                format!("task {task_id} returned an unsupported payload"),
            )),
        }
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<()> {
        self.require_tasks()?;
        self.get_client()?
            .cancel_task(CancelTaskParams::new(task_id))
            .await
            .map_err(|error| self.task_protocol_error("cancel task", task_id, error))
    }

    async fn get_detailed_task(&self, task_id: &str) -> Result<DetailedTask> {
        self.require_tasks()?;
        self.get_client()?
            .get_task(GetTaskParams::new(task_id))
            .await
            .map(|result| result.task)
            .map_err(|error| self.task_protocol_error("get task", task_id, error))
    }

    fn require_tasks(&self) -> Result<()> {
        self.require_capability("io.modelcontextprotocol/tasks", |info| {
            info.capabilities.supports_tasks()
        })
    }

    fn protocol_error(&self, operation: &str, error: rmcp::ServiceError) -> Error {
        Error::new(
            FailureCode::TaskFailed,
            format!("{operation} failed: {error}"),
        )
    }

    fn task_protocol_error(
        &self,
        operation: &str,
        task_id: &str,
        error: rmcp::ServiceError,
    ) -> Error {
        if matches!(
            &error,
            rmcp::ServiceError::McpError(error)
                if error.code == ErrorCode::RESOURCE_NOT_FOUND
                    || error.code == ErrorCode::INVALID_PARAMS
        ) {
            return Error::new(
                FailureCode::TaskNotFound,
                format!("task not found: {task_id}"),
            )
            .with_context(ErrorContext::Task {
                task_id: task_id.to_string(),
            });
        }
        self.protocol_error(operation, error)
    }
}
