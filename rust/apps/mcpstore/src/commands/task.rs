use clap::{Args, Subcommand, ValueEnum};
use mcpstore::transport::TransportError;
use mcpstore::{
    InstanceId, MCPStore, McpTask, McpTaskRecord, McpTaskStatus, McpToolExecution, StoreError,
};
use serde_json::{json, Value};

use crate::store_args::{build_store, StoreSourceArgs};
use crate::BoxErr;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, ValueEnum)]
pub enum TaskOutputFormat {
    #[default]
    Human,
    Json,
    Jsonl,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TaskErrorCode {
    InvalidInput,
    ServiceNotFound,
    ConnectionFailed,
    AuthenticationRequired,
    CapabilityUnsupported,
    TaskNotFound,
    TaskExpired,
    TaskResultUnavailable,
    TaskFailed,
    TaskNotCancellable,
    TaskProtocolFailed,
    TaskStateFailed,
    CommandFailed,
}

impl TaskErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::ServiceNotFound => "service_not_found",
            Self::ConnectionFailed => "connection_failed",
            Self::AuthenticationRequired => "authentication_required",
            Self::CapabilityUnsupported => "capability_unsupported",
            Self::TaskNotFound => "task_not_found",
            Self::TaskExpired => "task_expired",
            Self::TaskResultUnavailable => "task_result_unavailable",
            Self::TaskFailed => "task_failed",
            Self::TaskNotCancellable => "task_not_cancellable",
            Self::TaskProtocolFailed => "task_protocol_failed",
            Self::TaskStateFailed => "task_state_failed",
            Self::CommandFailed => "task_command_failed",
        }
    }

    fn exit_code(self) -> i32 {
        match self {
            Self::InvalidInput => 2,
            Self::ServiceNotFound => 10,
            Self::ConnectionFailed => 11,
            Self::AuthenticationRequired => 12,
            Self::CapabilityUnsupported => 20,
            Self::TaskNotFound => 21,
            Self::TaskExpired => 22,
            Self::TaskResultUnavailable => 23,
            Self::TaskFailed => 24,
            Self::TaskProtocolFailed => 25,
            Self::TaskStateFailed => 26,
            Self::TaskNotCancellable => 27,
            Self::CommandFailed => 1,
        }
    }
}

#[derive(Debug)]
pub struct TaskCommandError {
    format: TaskOutputFormat,
    code: TaskErrorCode,
    message: String,
    instance_id: Option<InstanceId>,
    task_id: Option<String>,
}

impl TaskCommandError {
    fn new(format: TaskOutputFormat, code: TaskErrorCode, message: impl Into<String>) -> Self {
        Self {
            format,
            code,
            message: message.into(),
            instance_id: None,
            task_id: None,
        }
    }

    fn for_task(
        format: TaskOutputFormat,
        code: TaskErrorCode,
        message: impl Into<String>,
        instance_id: InstanceId,
        task_id: impl Into<String>,
    ) -> Self {
        Self {
            format,
            code,
            message: message.into(),
            instance_id: Some(instance_id),
            task_id: Some(task_id.into()),
        }
    }

    fn from_store(error: StoreError, format: TaskOutputFormat) -> Self {
        let code = match &error {
            StoreError::ServiceNotFound(_) => TaskErrorCode::ServiceNotFound,
            StoreError::Auth(_) => TaskErrorCode::AuthenticationRequired,
            StoreError::Transport(error) => match error {
                TransportError::AuthRequired(_) | TransportError::InsufficientScope { .. } => {
                    TaskErrorCode::AuthenticationRequired
                }
                TransportError::CapabilityUnsupported { .. } => {
                    TaskErrorCode::CapabilityUnsupported
                }
                TransportError::ConnectionFailed(_)
                | TransportError::NotConnected(_)
                | TransportError::RequestDisconnected { .. }
                | TransportError::Io(_) => TaskErrorCode::ConnectionFailed,
                TransportError::TaskNotFound { .. } => TaskErrorCode::TaskNotFound,
                TransportError::Protocol(_) => TaskErrorCode::TaskProtocolFailed,
                TransportError::TaskState(_) => TaskErrorCode::TaskStateFailed,
                TransportError::ToolCallFailed(_)
                | TransportError::RequestCancelled { .. }
                | TransportError::RequestTimedOut { .. } => TaskErrorCode::CommandFailed,
            },
            StoreError::Cache(_) => TaskErrorCode::TaskStateFailed,
            StoreError::Config(_) | StoreError::Other(_) => TaskErrorCode::CommandFailed,
        };
        Self::new(format, code, error.to_string())
    }

    pub fn exit_code(&self) -> i32 {
        self.code.exit_code()
    }

    fn json_value(&self) -> Value {
        json!({
            "event": "task.error",
            "error": {
                "code": self.code.as_str(),
                "message": self.message,
            },
            "instance_id": self.instance_id,
            "task_id": self.task_id,
        })
    }
}

impl std::fmt::Display for TaskCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.format {
            TaskOutputFormat::Human => {
                write!(formatter, "{}: {}", self.code.as_str(), self.message)
            }
            TaskOutputFormat::Json | TaskOutputFormat::Jsonl => self.json_value().fmt(formatter),
        }
    }
}

impl std::error::Error for TaskCommandError {}

#[derive(Args)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub action: TaskAction,
}

#[derive(Subcommand)]
pub enum TaskAction {
    Run(TaskRunArgs),
    List(TaskInstanceArgs),
    Status(TaskTargetArgs),
    Result(TaskTargetArgs),
    Cancel(TaskTargetArgs),
}

#[derive(Args)]
pub struct TaskRuntimeArgs {
    #[arg(
        long,
        value_enum,
        default_value_t = TaskOutputFormat::Human,
        help = "Output format: human, json, or jsonl"
    )]
    pub output: TaskOutputFormat,
    #[arg(long, help = "Guarantee that the command does not prompt for input")]
    pub non_interactive: bool,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Args)]
pub struct TaskRunArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: InstanceId,
    #[arg(help = "Tool name")]
    pub tool_name: String,
    #[arg(long, default_value = "{}", help = "Tool input JSON object")]
    pub input: String,
    #[arg(long, help = "Requested task retention TTL in milliseconds")]
    pub ttl: Option<u64>,
    #[command(flatten)]
    pub runtime: TaskRuntimeArgs,
}

#[derive(Args)]
pub struct TaskInstanceArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: InstanceId,
    #[command(flatten)]
    pub runtime: TaskRuntimeArgs,
}

#[derive(Args)]
pub struct TaskTargetArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: InstanceId,
    #[arg(help = "Task ID")]
    pub task_id: String,
    #[command(flatten)]
    pub runtime: TaskRuntimeArgs,
}

pub async fn run(args: TaskArgs) -> Result<(), BoxErr> {
    execute(args.action)
        .await
        .map_err(|error| Box::new(error) as BoxErr)
}

async fn execute(action: TaskAction) -> Result<(), TaskCommandError> {
    match action {
        TaskAction::Run(args) => run_task(args).await,
        TaskAction::List(args) => list_tasks(args).await,
        TaskAction::Status(args) => show_status(args).await,
        TaskAction::Result(args) => show_result(args).await,
        TaskAction::Cancel(args) => cancel_task(args).await,
    }
}

async fn run_task(args: TaskRunArgs) -> Result<(), TaskCommandError> {
    let output = args.runtime.output;
    let input = parse_input(&args.input, output)?;
    let store = loaded_store(&args.runtime, output).await?;
    let execution = store
        .call_task_tool(args.instance_id, &args.tool_name, input, args.ttl)
        .await
        .map_err(|error| TaskCommandError::from_store(error, output))?;

    match execution {
        McpToolExecution::Immediate { result } => emit(
            output,
            immediate_human(&args.tool_name, &result),
            json!({
                "event": "task.completed",
                "instance_id": args.instance_id,
                "tool_name": args.tool_name,
                "execution": "immediate",
                "result": result,
            }),
        ),
        McpToolExecution::Task { task } => {
            let record =
                require_task_record(&store, args.instance_id, &task.task_id, output).await?;
            emit(
                output,
                task_human("created", &record),
                task_event("task.created", &record),
            )
        }
    }
}

async fn list_tasks(args: TaskInstanceArgs) -> Result<(), TaskCommandError> {
    let output = args.runtime.output;
    let store = loaded_store(&args.runtime, output).await?;
    store
        .list_tasks(args.instance_id)
        .await
        .map_err(|error| TaskCommandError::from_store(error, output))?;
    let records = store
        .list_task_records(args.instance_id)
        .await
        .map_err(|error| TaskCommandError::from_store(error, output))?;

    match output {
        TaskOutputFormat::Human => {
            println!("tasks: {}", records.len());
            for record in &records {
                println!("{}", task_human("task", record));
            }
            Ok(())
        }
        TaskOutputFormat::Json => emit_value(
            output,
            json!({
                "event": "task.list",
                "instance_id": args.instance_id,
                "count": records.len(),
                "tasks": records,
            }),
        ),
        TaskOutputFormat::Jsonl => {
            for record in &records {
                emit_value(output, task_event("task.observed", record))?;
            }
            emit_value(
                output,
                json!({
                    "event": "task.list.completed",
                    "instance_id": args.instance_id,
                    "count": records.len(),
                }),
            )
        }
    }
}

async fn show_status(args: TaskTargetArgs) -> Result<(), TaskCommandError> {
    let output = args.runtime.output;
    let store = loaded_store(&args.runtime, output).await?;
    store
        .get_task(args.instance_id, &args.task_id)
        .await
        .map_err(|error| with_task_context(error, output, args.instance_id, &args.task_id))?;
    let record = require_task_record(&store, args.instance_id, &args.task_id, output).await?;
    emit(
        output,
        task_human("status", &record),
        task_event("task.status", &record),
    )
}

async fn show_result(args: TaskTargetArgs) -> Result<(), TaskCommandError> {
    let output = args.runtime.output;
    let store = loaded_store(&args.runtime, output).await?;
    let task = store
        .get_task(args.instance_id, &args.task_id)
        .await
        .map_err(|error| with_task_context(error, output, args.instance_id, &args.task_id))?;
    ensure_result_available(args.instance_id, &task, output)?;
    let result = store
        .get_task_result(args.instance_id, &args.task_id)
        .await
        .map_err(|error| with_task_context(error, output, args.instance_id, &args.task_id))?;
    emit(
        output,
        format!(
            "task_id: {}\nstatus: completed\nresult:\n{}",
            args.task_id,
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
        ),
        json!({
            "event": "task.result",
            "instance_id": args.instance_id,
            "task_id": args.task_id,
            "status": task.status,
            "result": result,
        }),
    )
}

async fn cancel_task(args: TaskTargetArgs) -> Result<(), TaskCommandError> {
    let output = args.runtime.output;
    let store = loaded_store(&args.runtime, output).await?;
    if let Some(record) = store
        .get_task_record(args.instance_id, &args.task_id)
        .await
        .map_err(|error| with_task_context(error, output, args.instance_id, &args.task_id))?
    {
        ensure_cancellable(args.instance_id, &record.task, output)?;
    }
    store
        .cancel_task(args.instance_id, &args.task_id)
        .await
        .map_err(|error| with_task_context(error, output, args.instance_id, &args.task_id))?;
    let record = require_task_record(&store, args.instance_id, &args.task_id, output).await?;
    emit(
        output,
        task_human("cancelled", &record),
        task_event("task.cancelled", &record),
    )
}

async fn loaded_store(
    runtime: &TaskRuntimeArgs,
    output: TaskOutputFormat,
) -> Result<MCPStore, TaskCommandError> {
    let store = build_store(&runtime.store).map_err(|error| {
        TaskCommandError::new(output, TaskErrorCode::CommandFailed, error.to_string())
    })?;
    store
        .load_from_source()
        .await
        .map_err(|error| TaskCommandError::from_store(error, output))?;
    Ok(store)
}

async fn require_task_record(
    store: &MCPStore,
    instance_id: InstanceId,
    task_id: &str,
    output: TaskOutputFormat,
) -> Result<McpTaskRecord, TaskCommandError> {
    store
        .get_task_record(instance_id, task_id)
        .await
        .map_err(|error| with_task_context(error, output, instance_id, task_id))?
        .ok_or_else(|| {
            TaskCommandError::for_task(
                output,
                TaskErrorCode::TaskStateFailed,
                "task state was not persisted after the operation",
                instance_id,
                task_id,
            )
        })
}

fn parse_input(input: &str, output: TaskOutputFormat) -> Result<Value, TaskCommandError> {
    let value: Value = serde_json::from_str(input).map_err(|error| {
        TaskCommandError::new(
            output,
            TaskErrorCode::InvalidInput,
            format!("invalid --input JSON: {error}"),
        )
    })?;
    if !value.is_object() {
        return Err(TaskCommandError::new(
            output,
            TaskErrorCode::InvalidInput,
            "--input must be a JSON object",
        ));
    }
    Ok(value)
}

fn with_task_context(
    error: StoreError,
    output: TaskOutputFormat,
    instance_id: InstanceId,
    task_id: &str,
) -> TaskCommandError {
    let mut error = TaskCommandError::from_store(error, output);
    error.instance_id = Some(instance_id);
    error.task_id = Some(task_id.to_string());
    error
}

fn ensure_result_available(
    instance_id: InstanceId,
    task: &McpTask,
    output: TaskOutputFormat,
) -> Result<(), TaskCommandError> {
    match task.status {
        McpTaskStatus::Completed => Ok(()),
        McpTaskStatus::Failed => Err(TaskCommandError::for_task(
            output,
            TaskErrorCode::TaskFailed,
            task.status_message
                .as_deref()
                .unwrap_or("task failed without a status message"),
            instance_id,
            &task.task_id,
        )),
        McpTaskStatus::Expired => Err(TaskCommandError::for_task(
            output,
            TaskErrorCode::TaskExpired,
            "task retention TTL has elapsed",
            instance_id,
            &task.task_id,
        )),
        _ => Err(TaskCommandError::for_task(
            output,
            TaskErrorCode::TaskResultUnavailable,
            format!(
                "task result is unavailable while status is {}",
                status_name(&task.status)
            ),
            instance_id,
            &task.task_id,
        )),
    }
}

fn ensure_cancellable(
    instance_id: InstanceId,
    task: &McpTask,
    output: TaskOutputFormat,
) -> Result<(), TaskCommandError> {
    match task.status {
        McpTaskStatus::Expired => Err(TaskCommandError::for_task(
            output,
            TaskErrorCode::TaskExpired,
            "task retention TTL has elapsed",
            instance_id,
            &task.task_id,
        )),
        McpTaskStatus::Completed | McpTaskStatus::Failed | McpTaskStatus::Cancelled => {
            Err(TaskCommandError::for_task(
                output,
                TaskErrorCode::TaskNotCancellable,
                format!(
                    "task cannot be cancelled while status is {}",
                    status_name(&task.status)
                ),
                instance_id,
                &task.task_id,
            ))
        }
        _ => Ok(()),
    }
}

fn task_event(event: &'static str, record: &McpTaskRecord) -> Value {
    json!({
        "event": event,
        "instance_id": record.instance_id,
        "task_id": record.task_id,
        "tool_name": record.tool_name,
        "task": record.task,
        "last_observed_at": record.last_observed_at,
        "last_error": record.last_error,
    })
}

fn task_human(label: &str, record: &McpTaskRecord) -> String {
    let mut lines = vec![
        format!("event: {label}"),
        format!("instance_id: {}", record.instance_id),
        format!("task_id: {}", record.task_id),
        format!("status: {}", status_name(&record.task.status)),
    ];
    if let Some(tool_name) = &record.tool_name {
        lines.push(format!("tool: {tool_name}"));
    }
    if let Some(message) = &record.task.status_message {
        lines.push(format!("message: {message}"));
    }
    if let Some(ttl) = record.task.ttl {
        lines.push(format!("ttl_ms: {ttl}"));
    }
    if let Some(poll_interval) = record.task.poll_interval {
        lines.push(format!("poll_interval_ms: {poll_interval}"));
    }
    if let Some(error) = &record.last_error {
        lines.push(format!("last_error: {error}"));
    }
    lines.join("\n")
}

fn immediate_human(tool_name: &str, result: &mcpstore::ToolCallResult) -> String {
    format!(
        "event: completed\nexecution: immediate\ntool: {tool_name}\nresult:\n{}",
        serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string())
    )
}

fn status_name(status: &McpTaskStatus) -> &'static str {
    match status {
        McpTaskStatus::Working => "working",
        McpTaskStatus::InputRequired => "input_required",
        McpTaskStatus::Completed => "completed",
        McpTaskStatus::Failed => "failed",
        McpTaskStatus::Cancelled => "cancelled",
        McpTaskStatus::Expired => "expired",
        McpTaskStatus::Disconnected => "disconnected",
        McpTaskStatus::Unknown => "unknown",
    }
}

fn emit(output: TaskOutputFormat, human: String, value: Value) -> Result<(), TaskCommandError> {
    match output {
        TaskOutputFormat::Human => {
            println!("{human}");
            Ok(())
        }
        TaskOutputFormat::Json | TaskOutputFormat::Jsonl => emit_value(output, value),
    }
}

fn emit_value(output: TaskOutputFormat, value: Value) -> Result<(), TaskCommandError> {
    let encoded = match output {
        TaskOutputFormat::Human => Ok(value.to_string()),
        TaskOutputFormat::Json => serde_json::to_string_pretty(&value),
        TaskOutputFormat::Jsonl => serde_json::to_string(&value),
    }
    .map_err(|error| {
        TaskCommandError::new(
            output,
            TaskErrorCode::CommandFailed,
            format!("failed to encode task output: {error}"),
        )
    })?;
    println!("{encoded}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(status: McpTaskStatus) -> McpTask {
        McpTask {
            task_id: "task-1".to_string(),
            status,
            status_message: None,
            created_at: "2026-07-15T00:00:00Z".to_string(),
            last_updated_at: "2026-07-15T00:00:01Z".to_string(),
            ttl: Some(60_000),
            poll_interval: Some(250),
        }
    }

    #[test]
    fn input_must_be_a_json_object() {
        assert_eq!(
            parse_input("{\"value\":1}", TaskOutputFormat::Human).unwrap()["value"],
            1
        );
        let error = parse_input("[]", TaskOutputFormat::Human).unwrap_err();
        assert_eq!(error.code, TaskErrorCode::InvalidInput);
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn task_result_states_have_stable_error_codes() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();
        assert!(ensure_result_available(
            instance_id,
            &task(McpTaskStatus::Completed),
            TaskOutputFormat::Jsonl,
        )
        .is_ok());

        for (status, code, exit_code) in [
            (McpTaskStatus::Failed, TaskErrorCode::TaskFailed, 24),
            (McpTaskStatus::Expired, TaskErrorCode::TaskExpired, 22),
            (
                McpTaskStatus::Working,
                TaskErrorCode::TaskResultUnavailable,
                23,
            ),
        ] {
            let error =
                ensure_result_available(instance_id, &task(status), TaskOutputFormat::Jsonl)
                    .unwrap_err();
            assert_eq!(error.code, code);
            assert_eq!(error.exit_code(), exit_code);
            let value: Value = serde_json::from_str(&error.to_string()).unwrap();
            assert_eq!(value["event"], "task.error");
            assert_eq!(value["error"]["code"], code.as_str());
        }
    }

    #[test]
    fn terminal_tasks_have_stable_cancellation_errors() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();

        let expired = ensure_cancellable(
            instance_id,
            &task(McpTaskStatus::Expired),
            TaskOutputFormat::Jsonl,
        )
        .unwrap_err();
        assert_eq!(expired.code, TaskErrorCode::TaskExpired);
        assert_eq!(expired.exit_code(), 22);

        for status in [
            McpTaskStatus::Completed,
            McpTaskStatus::Failed,
            McpTaskStatus::Cancelled,
        ] {
            let error = ensure_cancellable(instance_id, &task(status), TaskOutputFormat::Jsonl)
                .unwrap_err();
            assert_eq!(error.code, TaskErrorCode::TaskNotCancellable);
            assert_eq!(error.exit_code(), 27);
        }
    }

    #[test]
    fn store_errors_map_to_stable_task_codes() {
        let service = TaskCommandError::from_store(
            StoreError::ServiceNotFound("missing".to_string()),
            TaskOutputFormat::Human,
        );
        assert_eq!(service.code, TaskErrorCode::ServiceNotFound);
        assert_eq!(service.exit_code(), 10);

        let unsupported = TaskCommandError::from_store(
            StoreError::Transport(TransportError::CapabilityUnsupported {
                instance_id: "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap(),
                capability: "tasks.list",
            }),
            TaskOutputFormat::Json,
        );
        assert_eq!(unsupported.code, TaskErrorCode::CapabilityUnsupported);
        assert_eq!(unsupported.exit_code(), 20);

        let missing = TaskCommandError::from_store(
            StoreError::Transport(TransportError::TaskNotFound {
                task_id: "task-1".to_string(),
            }),
            TaskOutputFormat::Jsonl,
        );
        assert_eq!(missing.code, TaskErrorCode::TaskNotFound);
        assert_eq!(missing.exit_code(), 21);
    }

    #[test]
    fn task_event_includes_instance_and_persisted_state() {
        let record = McpTaskRecord {
            instance_id: "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap(),
            task_id: "task-1".to_string(),
            tool_name: Some("long_tool".to_string()),
            task: task(McpTaskStatus::Working),
            last_observed_at: "2026-07-15T00:00:02Z".to_string(),
            last_error: None,
        };
        let event = task_event("task.created", &record);
        assert_eq!(event["event"], "task.created");
        assert_eq!(event["task_id"], "task-1");
        assert_eq!(event["tool_name"], "long_tool");
        assert_eq!(event["task"]["status"], "working");
    }
}
