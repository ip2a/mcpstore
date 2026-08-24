use clap::{Args, Subcommand};
#[cfg(test)]
use mcpstore::error::{Error, ErrorContext, FailureCode};
use mcpstore::{
    InstanceId, MCPStore, McpExecutionOptions, McpStoreExecutionUpdate, McpTask, McpTaskRecord,
    McpTaskStatus, McpToolExecution, StoreError,
};
use serde_json::{json, Value};
use std::time::Duration;

use crate::commands::elicitation::{
    handle_elicitation, settle_execution_after_elicitation_error, ElicitationArgs,
    ElicitationCommandError, ElicitationErrorKind,
};
use crate::error::{CliError, Domain, ErrorCode, OutputFormat};
use crate::store_args::{build_store, StoreSourceArgs};
use crate::BoxErr;

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
        default_value_t = OutputFormat::Human,
        help = "Output format: human, json, or jsonl"
    )]
    pub output: OutputFormat,
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
    #[arg(
        long,
        value_name = "SECONDS",
        help = "Idle timeout, reset by matching progress"
    )]
    pub timeout: Option<u64>,
    #[arg(
        long = "max-total-timeout",
        value_name = "SECONDS",
        help = "Maximum total execution time"
    )]
    pub max_total_timeout: Option<u64>,
    #[command(flatten)]
    pub elicitation: ElicitationArgs,
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

async fn execute(action: TaskAction) -> Result<(), CliError> {
    match action {
        TaskAction::Run(args) => run_task(args).await,
        TaskAction::List(args) => list_tasks(args).await,
        TaskAction::Status(args) => show_status(args).await,
        TaskAction::Result(args) => show_result(args).await,
        TaskAction::Cancel(args) => cancel_task(args).await,
    }
}

async fn run_task(args: TaskRunArgs) -> Result<(), CliError> {
    let output = args.runtime.output;
    let input = parse_input(&args.input, output)?;
    let store = loaded_store(&args.runtime, output).await?;
    let mut options = McpExecutionOptions::default();
    if let Some(timeout) = args.timeout {
        options = options.with_idle_timeout(Duration::from_secs(timeout));
    }
    if let Some(timeout) = args.max_total_timeout {
        options = options.with_max_total_timeout(Duration::from_secs(timeout));
    }

    let mut elicitation = store
        .open_elicitation_session(args.instance_id, args.elicitation.session_options())
        .await
        .map_err(|error| CliError::from_store(&error, output, Domain::Task))?;
    let mut execution = store
        .start_task_execution(args.instance_id, &args.tool_name, input, options)
        .await
        .map_err(|error| CliError::from_store(&error, output, Domain::Task))?;
    emit_task_started(output, &args.tool_name, &execution)?;

    let mut cancellation_requested = false;
    loop {
        let update = if cancellation_requested {
            execution.next_update().await
        } else {
            tokio::select! {
                biased;
                update = execution.next_update() => update,
                request = async {
                    match elicitation.as_mut() {
                        Some(session) => session.next_request().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match request {
                        Some(request) => {
                            if let Err(error) = handle_elicitation(
                                request,
                                &args.elicitation,
                                output,
                                args.runtime.non_interactive,
                            )
                            .await
                            {
                                settle_execution_after_elicitation_error(&mut execution).await;
                                return Err(task_elicitation_error(
                                    error,
                                    output,
                                    args.instance_id,
                                ));
                            }
                        }
                        None => elicitation = None,
                    }
                    continue;
                }
                signal = tokio::signal::ctrl_c() => {
                    signal.map_err(|error| CliError::new(
                        output,
                        Domain::Task,
                        ErrorCode::CommandFailed,
                        format!("failed to listen for Ctrl+C: {error}"),
                    ))?;
                    if execution.cancel("cancelled by user (Ctrl+C)") {
                        cancellation_requested = true;
                        emit_task_cancellation_requested(output, args.instance_id, &args.tool_name)?;
                    }
                    continue;
                }
            }
        };

        match update {
            Some(McpStoreExecutionUpdate::Progress(progress)) => {
                emit_task_progress(output, &args.tool_name, &progress)?;
            }
            Some(McpStoreExecutionUpdate::Finished(result)) => {
                let execution = result.map_err(|error| {
                    CliError::from_store(&error, output, Domain::Task)
                        .with("instance_id", args.instance_id.to_string())
                })?;
                if cancellation_requested {
                    return cancel_created_task(
                        &store,
                        output,
                        args.instance_id,
                        &args.tool_name,
                        execution,
                    )
                    .await;
                }
                return finish_task_execution(
                    &store,
                    output,
                    args.instance_id,
                    &args.tool_name,
                    execution,
                )
                .await;
            }
            None => {
                return Err(CliError::new(
                    output,
                    Domain::Task,
                    ErrorCode::TaskProtocolFailed,
                    "task execution ended without a result",
                )
                .with("instance_id", args.instance_id.to_string()));
            }
        }
    }
}

fn task_elicitation_error(
    error: ElicitationCommandError,
    output: OutputFormat,
    instance_id: InstanceId,
) -> CliError {
    let code = match error.kind() {
        ElicitationErrorKind::InputRequired => ErrorCode::ElicitationInputRequired,
        ElicitationErrorKind::Cancelled => ErrorCode::ElicitationCancelled,
        ElicitationErrorKind::TimedOut => ErrorCode::ElicitationTimedOut,
        ElicitationErrorKind::InvalidResponse => ErrorCode::ElicitationInvalidResponse,
    };
    CliError::new(output, Domain::Task, code, error.message())
        .with("instance_id", instance_id.to_string())
}

async fn finish_task_execution(
    store: &MCPStore,
    output: OutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
    execution: McpToolExecution,
) -> Result<(), CliError> {
    match execution {
        McpToolExecution::Immediate { result } => emit(
            output,
            immediate_human(tool_name, &result),
            json!({
                "event": "task.completed",
                "instance_id": instance_id,
                "tool_name": tool_name,
                "execution": "immediate",
                "result": result,
            }),
        ),
        McpToolExecution::Task { task } => {
            let record = require_task_record(store, instance_id, &task.task_id, output).await?;
            emit(
                output,
                task_human("created", &record),
                task_event("task.created", &record),
            )
        }
    }
}

async fn cancel_created_task(
    store: &MCPStore,
    output: OutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
    execution: McpToolExecution,
) -> Result<(), CliError> {
    let task_id = match execution {
        McpToolExecution::Task { task } => {
            store
                .cancel_task(instance_id, &task.task_id)
                .await
                .map_err(|error| with_task_context(error, output, instance_id, &task.task_id))?;
            Some(task.task_id)
        }
        McpToolExecution::Immediate { .. } => None,
    };
    let error = CliError::new(
        output,
        Domain::Task,
        ErrorCode::Cancelled,
        format!("task execution for {tool_name} was cancelled by user"),
    )
    .with("instance_id", instance_id.to_string());
    let error = match &task_id {
        Some(id) => error.with("task_id", id.as_str()),
        None => error,
    };
    Err(error)
}

fn emit_task_started(
    output: OutputFormat,
    tool_name: &str,
    execution: &mcpstore::McpStoreToolExecutionHandle<'_>,
) -> Result<(), CliError> {
    if output != OutputFormat::Jsonl {
        return Ok(());
    }
    emit_value(
        output,
        json!({
            "event": "task.started",
            "instance_id": execution.instance_id(),
            "tool_name": tool_name,
            "request_id": execution.request_id(),
            "progress_token": execution.progress_token(),
            "cancellable": execution.supports_cancellation(),
        }),
    )
}

fn emit_task_progress(
    output: OutputFormat,
    tool_name: &str,
    progress: &mcpstore::McpExecutionProgress,
) -> Result<(), CliError> {
    match output {
        OutputFormat::Human => {
            let amount = progress.total.map_or_else(
                || progress.progress.to_string(),
                |total| format!("{}/{}", progress.progress, total),
            );
            if let Some(message) = &progress.message {
                eprintln!("[Progress] {tool_name}: {amount} {message}");
            } else {
                eprintln!("[Progress] {tool_name}: {amount}");
            }
            Ok(())
        }
        OutputFormat::Json => Ok(()),
        OutputFormat::Jsonl => emit_value(
            output,
            json!({
                "event": "task.progress",
                "instance_id": progress.instance_id,
                "tool_name": tool_name,
                "progress_token": progress.progress_token,
                "progress": progress.progress,
                "total": progress.total,
                "message": progress.message,
            }),
        ),
    }
}

fn emit_task_cancellation_requested(
    output: OutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
) -> Result<(), CliError> {
    match output {
        OutputFormat::Human => {
            eprintln!("[Cancellation requested] {tool_name}");
            Ok(())
        }
        OutputFormat::Json => Ok(()),
        OutputFormat::Jsonl => emit_value(
            output,
            json!({
                "event": "task.cancellation_requested",
                "instance_id": instance_id,
                "tool_name": tool_name,
            }),
        ),
    }
}

async fn list_tasks(args: TaskInstanceArgs) -> Result<(), CliError> {
    let output = args.runtime.output;
    let store = loaded_store(&args.runtime, output).await?;
    let records = store
        .list_task_records(args.instance_id)
        .await
        .map_err(|error| CliError::from_store(&error, output, Domain::Task))?;

    match output {
        OutputFormat::Human => {
            println!("tasks: {}", records.len());
            for record in &records {
                println!("{}", task_human("task", record));
            }
            Ok(())
        }
        OutputFormat::Json => emit_value(
            output,
            json!({
                "event": "task.list",
                "instance_id": args.instance_id,
                "count": records.len(),
                "tasks": records,
            }),
        ),
        OutputFormat::Jsonl => {
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

async fn show_status(args: TaskTargetArgs) -> Result<(), CliError> {
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

async fn show_result(args: TaskTargetArgs) -> Result<(), CliError> {
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

async fn cancel_task(args: TaskTargetArgs) -> Result<(), CliError> {
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
        task_human("cancellation_requested", &record),
        task_event("task.cancellation_requested", &record),
    )
}

async fn loaded_store(
    runtime: &TaskRuntimeArgs,
    output: OutputFormat,
) -> Result<std::sync::Arc<MCPStore>, CliError> {
    let store = build_store(&runtime.store).map_err(|error| {
        CliError::new(
            output,
            Domain::Task,
            ErrorCode::CommandFailed,
            error.to_string(),
        )
    })?;
    store
        .load_from_source()
        .await
        .map_err(|error| CliError::from_store(&error, output, Domain::Task))?;
    Ok(store)
}

async fn require_task_record(
    store: &MCPStore,
    instance_id: InstanceId,
    task_id: &str,
    output: OutputFormat,
) -> Result<McpTaskRecord, CliError> {
    store
        .get_task_record(instance_id, task_id)
        .await
        .map_err(|error| with_task_context(error, output, instance_id, task_id))?
        .ok_or_else(|| {
            CliError::new(
                output,
                Domain::Task,
                ErrorCode::TaskStateFailed,
                "task state was not persisted after the operation",
            )
            .with("instance_id", instance_id.to_string())
            .with("task_id", task_id)
        })
}

fn parse_input(input: &str, output: OutputFormat) -> Result<Value, CliError> {
    let value: Value = serde_json::from_str(input).map_err(|error| {
        CliError::new(
            output,
            Domain::Task,
            ErrorCode::InvalidInput,
            format!("invalid --input JSON: {error}"),
        )
    })?;
    if !value.is_object() {
        return Err(CliError::new(
            output,
            Domain::Task,
            ErrorCode::InvalidInput,
            "--input must be a JSON object",
        ));
    }
    Ok(value)
}

fn with_task_context(
    error: StoreError,
    output: OutputFormat,
    instance_id: InstanceId,
    task_id: &str,
) -> CliError {
    CliError::from_store(&error, output, Domain::Task)
        .with("instance_id", instance_id.to_string())
        .with("task_id", task_id)
}

fn ensure_result_available(
    instance_id: InstanceId,
    task: &McpTask,
    output: OutputFormat,
) -> Result<(), CliError> {
    match task.status {
        McpTaskStatus::Completed => Ok(()),
        McpTaskStatus::Failed => Err(CliError::new(
            output,
            Domain::Task,
            ErrorCode::TaskFailed,
            task.status_message
                .as_deref()
                .unwrap_or("task failed without a status message"),
        )
        .with("instance_id", instance_id.to_string())
        .with("task_id", task.task_id.as_str())),
        _ => Err(CliError::new(
            output,
            Domain::Task,
            ErrorCode::TaskResultUnavailable,
            format!(
                "task result is unavailable while status is {}",
                status_name(&task.status)
            ),
        )
        .with("instance_id", instance_id.to_string())
        .with("task_id", task.task_id.as_str())),
    }
}

fn ensure_cancellable(
    instance_id: InstanceId,
    task: &McpTask,
    output: OutputFormat,
) -> Result<(), CliError> {
    match task.status {
        McpTaskStatus::Completed | McpTaskStatus::Failed | McpTaskStatus::Cancelled => {
            Err(CliError::new(
                output,
                Domain::Task,
                ErrorCode::TaskNotCancellable,
                format!(
                    "task cannot be cancelled while status is {}",
                    status_name(&task.status)
                ),
            )
            .with("instance_id", instance_id.to_string())
            .with("task_id", task.task_id.as_str()))
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
    if let Some(ttl) = record.task.ttl_ms {
        lines.push(format!("ttl_ms: {ttl}"));
    }
    if let Some(poll_interval) = record.task.poll_interval_ms {
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
        McpTaskStatus::Disconnected => "disconnected",
        McpTaskStatus::Unknown => "unknown",
    }
}

fn emit(output: OutputFormat, human: String, value: Value) -> Result<(), CliError> {
    match output {
        OutputFormat::Human => {
            println!("{human}");
            Ok(())
        }
        OutputFormat::Json | OutputFormat::Jsonl => emit_value(output, value),
    }
}

fn emit_value(output: OutputFormat, value: Value) -> Result<(), CliError> {
    let encoded = match output {
        OutputFormat::Human => Ok(value.to_string()),
        OutputFormat::Json => serde_json::to_string_pretty(&value),
        OutputFormat::Jsonl => serde_json::to_string(&value),
    }
    .map_err(|error| {
        CliError::new(
            output,
            Domain::Task,
            ErrorCode::CommandFailed,
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
            ttl_ms: Some(60_000),
            poll_interval_ms: Some(250),
        }
    }

    #[test]
    fn input_must_be_a_json_object() {
        assert_eq!(
            parse_input("{\"value\":1}", OutputFormat::Human).unwrap()["value"],
            1
        );
        let error = parse_input("[]", OutputFormat::Human).unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidInput);
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn task_result_states_have_stable_error_codes() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();
        assert!(ensure_result_available(
            instance_id,
            &task(McpTaskStatus::Completed),
            OutputFormat::Jsonl,
        )
        .is_ok());

        for (status, code, exit_code) in [
            (McpTaskStatus::Failed, ErrorCode::TaskFailed, 24),
            (McpTaskStatus::Working, ErrorCode::TaskResultUnavailable, 23),
        ] {
            let error = ensure_result_available(instance_id, &task(status), OutputFormat::Jsonl)
                .unwrap_err();
            assert_eq!(error.code(), code);
            assert_eq!(error.exit_code(), exit_code);
            let value: Value = serde_json::from_str(&error.to_string()).unwrap();
            assert_eq!(value["event"], "task.failed");
            assert_eq!(value["error"]["code"], code.as_str());
        }
    }

    #[test]
    fn terminal_tasks_have_stable_cancellation_errors() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();

        for status in [
            McpTaskStatus::Completed,
            McpTaskStatus::Failed,
            McpTaskStatus::Cancelled,
        ] {
            let error =
                ensure_cancellable(instance_id, &task(status), OutputFormat::Jsonl).unwrap_err();
            assert_eq!(error.code(), ErrorCode::TaskNotCancellable);
            assert_eq!(error.exit_code(), 27);
        }
    }

    #[test]
    fn store_errors_map_to_stable_task_codes() {
        let service = CliError::from_store(
            &StoreError::ServiceNotFound("missing".to_string()),
            OutputFormat::Human,
            Domain::Task,
        );
        assert_eq!(service.code(), ErrorCode::ServiceNotFound);
        assert_eq!(service.exit_code(), 10);

        let unsupported = CliError::from_store(
            &StoreError::Transport(
                Error::new(
                    FailureCode::CapabilityUnsupported,
                    "MCP service instance does not support capability tasks.list",
                )
                .with_context(ErrorContext::Service {
                    instance_id: "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap(),
                    service_name: String::new(),
                }),
            ),
            OutputFormat::Json,
            Domain::Task,
        );
        assert_eq!(unsupported.code(), ErrorCode::CapabilityUnsupported);
        assert_eq!(unsupported.exit_code(), 20);

        let missing = CliError::from_store(
            &StoreError::Transport(
                Error::new(FailureCode::TaskNotFound, "task not found: task-1")
                    .with_context(ErrorContext::Task {
                        task_id: "task-1".to_string(),
                    }),
            ),
            OutputFormat::Jsonl,
            Domain::Task,
        );
        assert_eq!(missing.code(), ErrorCode::TaskNotFound);
        assert_eq!(missing.exit_code(), 21);
    }

    #[test]
    fn execution_errors_have_stable_task_codes_and_events() {
        let instance_id: InstanceId = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();
        for (error, code, exit_code, event) in [
            (
                Error::new(FailureCode::CallCancelled, "MCP request cancelled"),
                ErrorCode::Cancelled,
                30,
                "task.cancelled",
            ),
            (
                Error::new(
                    FailureCode::CallTimedOut,
                    "MCP request timed out after 1s",
                ),
                ErrorCode::TimedOut,
                31,
                "task.timed_out",
            ),
            (
                Error::new(
                    FailureCode::CallDisconnected,
                    format!("MCP request disconnected for service instance {instance_id}"),
                ),
                ErrorCode::Disconnected,
                32,
                "task.failed",
            ),
        ] {
            let error = CliError::from_store(
                &StoreError::Transport(error),
                OutputFormat::Jsonl,
                Domain::Task,
            )
            .with("instance_id", instance_id.to_string());
            assert_eq!(error.code(), code);
            assert_eq!(error.exit_code(), exit_code);
            let v: Value = serde_json::from_str(&error.to_string()).unwrap();
            assert_eq!(v["event"], event);
        }
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
