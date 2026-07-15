use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::{Args, ValueEnum};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use mcpstore::{
    McpElicitationRequest, McpElicitationRequestKind, McpElicitationResponseError,
    McpElicitationSessionOptions, McpStoreExecutionUpdate, McpStoreToolExecutionHandle,
};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, ValueEnum)]
pub enum ElicitationActionArg {
    #[default]
    Auto,
    Accept,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, Args)]
pub struct ElicitationArgs {
    #[arg(
        long = "elicitation-timeout",
        value_name = "SECONDS",
        default_value_t = 300,
        help = "Maximum time to answer each server elicitation request"
    )]
    pub elicitation_timeout: u64,
    #[arg(
        long = "elicitation-input-file",
        value_name = "PATH",
        help = "JSON object used to answer form elicitation without terminal input"
    )]
    pub input_file: Option<PathBuf>,
    #[arg(
        long = "elicitation-action",
        value_enum,
        default_value_t = ElicitationActionArg::Auto,
        help = "Elicitation policy: auto, accept, decline, or cancel"
    )]
    pub action: ElicitationActionArg,
}

impl ElicitationArgs {
    pub fn session_options(&self) -> McpElicitationSessionOptions {
        McpElicitationSessionOptions::default()
            .with_response_timeout(Duration::from_secs(self.elicitation_timeout))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ElicitationOutputFormat {
    Human,
    Json,
    Jsonl,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ElicitationErrorKind {
    InputRequired,
    Cancelled,
    TimedOut,
    InvalidResponse,
}

#[derive(Debug)]
pub struct ElicitationCommandError {
    kind: ElicitationErrorKind,
    message: String,
}

impl ElicitationCommandError {
    pub fn kind(&self) -> ElicitationErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(kind: ElicitationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub async fn settle_execution_after_elicitation_error(
    execution: &mut McpStoreToolExecutionHandle<'_>,
) {
    execution.cancel("elicitation handling failed");
    while let Some(update) = execution.next_update().await {
        if matches!(update, McpStoreExecutionUpdate::Finished(_)) {
            break;
        }
    }
}

pub async fn handle_elicitation(
    request: McpElicitationRequest,
    args: &ElicitationArgs,
    output: ElicitationOutputFormat,
    non_interactive: bool,
) -> Result<(), ElicitationCommandError> {
    emit_requested(output, &request)?;
    let interactive = !non_interactive && io::stdin().is_terminal() && io::stderr().is_terminal();
    let action = match args.action {
        ElicitationActionArg::Auto => match request.kind() {
            McpElicitationRequestKind::Form { .. } if args.input_file.is_some() => {
                PromptAction::Accept
            }
            McpElicitationRequestKind::Form { .. } if interactive => {
                match prompt_action(&request).await {
                    Ok(action) => action,
                    Err(error) => return abort_request(request, error, output),
                }
            }
            McpElicitationRequestKind::Form { .. } => {
                let identity = request_identity(&request);
                request.cancel().map_err(response_error)?;
                emit_state(output, "elicitation.input_required", &identity)?;
                return Err(ElicitationCommandError::new(
                    ElicitationErrorKind::InputRequired,
                    "form elicitation requires --elicitation-input-file or an interactive terminal",
                ));
            }
            McpElicitationRequestKind::Url { .. } if interactive => {
                match prompt_action(&request).await {
                    Ok(action) => action,
                    Err(error) => return abort_request(request, error, output),
                }
            }
            McpElicitationRequestKind::Url { .. } => PromptAction::Accept,
        },
        ElicitationActionArg::Accept => PromptAction::Accept,
        ElicitationActionArg::Decline => PromptAction::Decline,
        ElicitationActionArg::Cancel => PromptAction::Cancel,
    };

    match action {
        PromptAction::Decline => {
            let identity = request_identity(&request);
            request
                .decline()
                .map_err(|error| response_failure(error, output, &identity))?;
            emit_state(output, "elicitation.declined", &identity)
        }
        PromptAction::Cancel => {
            let identity = request_identity(&request);
            request
                .cancel()
                .map_err(|error| response_failure(error, output, &identity))?;
            emit_state(output, "elicitation.cancelled", &identity)?;
            Err(ElicitationCommandError::new(
                ElicitationErrorKind::Cancelled,
                "elicitation cancelled by user policy",
            ))
        }
        PromptAction::Accept => accept_request(request, args, output, interactive).await,
    }
}

async fn accept_request(
    request: McpElicitationRequest,
    args: &ElicitationArgs,
    output: ElicitationOutputFormat,
    interactive: bool,
) -> Result<(), ElicitationCommandError> {
    match request.kind() {
        McpElicitationRequestKind::Form { .. } => {
            let content = if let Some(path) = &args.input_file {
                match read_form_file(path) {
                    Ok(content) => content,
                    Err(error) => return abort_request(request, error, output),
                }
            } else if interactive {
                match prompt_form_content(&request).await {
                    Ok(content) => content,
                    Err(error) => return abort_request(request, error, output),
                }
            } else {
                let identity = request_identity(&request);
                request.cancel().map_err(response_error)?;
                emit_state(output, "elicitation.input_required", &identity)?;
                return Err(ElicitationCommandError::new(
                    ElicitationErrorKind::InputRequired,
                    "accepted form elicitation requires --elicitation-input-file in non-interactive mode",
                ));
            };
            let identity = request_identity(&request);
            request
                .accept(Some(content))
                .map_err(|error| response_failure(error, output, &identity))?;
            emit_state(output, "elicitation.accepted", &identity)
        }
        McpElicitationRequestKind::Url {
            url,
            elicitation_id,
            ..
        } => {
            request
                .validate(None)
                .map_err(|error| response_failure(error, output, &request_identity(&request)))?;
            emit_url_handoff(output, &request, url, elicitation_id)?;
            let identity = request_identity(&request);
            request.accept(None).map_err(response_error)?;
            emit_state(output, "elicitation.accepted", &identity)
        }
    }
}

fn read_form_file(path: &PathBuf) -> Result<Value, ElicitationCommandError> {
    let input = fs::read_to_string(path).map_err(|error| {
        ElicitationCommandError::new(
            ElicitationErrorKind::InvalidResponse,
            format!("failed to read elicitation input file: {error}"),
        )
    })?;
    serde_json::from_str(&input).map_err(|error| {
        ElicitationCommandError::new(
            ElicitationErrorKind::InvalidResponse,
            format!("elicitation input file is not valid JSON: {error}"),
        )
    })
}

async fn prompt_action(
    request: &McpElicitationRequest,
) -> Result<PromptAction, ElicitationCommandError> {
    eprint!("Choose [a]ccept, [d]ecline, or [c]ancel: ");
    io::stderr().flush().map_err(io_error)?;
    let deadline = Instant::now() + request.response_timeout();
    let action = tokio::task::spawn_blocking(move || read_action_key(deadline))
        .await
        .map_err(|error| {
            ElicitationCommandError::new(
                ElicitationErrorKind::InvalidResponse,
                format!("elicitation prompt failed: {error}"),
            )
        })??;
    eprintln!();
    Ok(action)
}

async fn prompt_form_content(
    request: &McpElicitationRequest,
) -> Result<Value, ElicitationCommandError> {
    let deadline = Instant::now() + request.response_timeout();
    loop {
        eprint!("Enter response JSON (input hidden): ");
        io::stderr().flush().map_err(io_error)?;
        let line = tokio::task::spawn_blocking(move || read_hidden_line(deadline))
            .await
            .map_err(|error| {
                ElicitationCommandError::new(
                    ElicitationErrorKind::InvalidResponse,
                    format!("elicitation prompt failed: {error}"),
                )
            })??;
        eprintln!();
        let content = match serde_json::from_str::<Value>(&line) {
            Ok(content) => content,
            Err(error) => {
                eprintln!(
                    "Invalid JSON at line {}, column {}.",
                    error.line(),
                    error.column()
                );
                continue;
            }
        };
        match request.validate(Some(&content)) {
            Ok(()) => return Ok(content),
            Err(error) => {
                eprintln!("Invalid response: {error}");
            }
        }
    }
}

fn read_action_key(deadline: Instant) -> Result<PromptAction, ElicitationCommandError> {
    let _raw = RawModeGuard::new()?;
    loop {
        let event = read_event_until(deadline)?;
        let Event::Key(key) = event else {
            continue;
        };
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }
        match key.code {
            KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Enter => {
                return Ok(PromptAction::Accept)
            }
            KeyCode::Char('d') | KeyCode::Char('D') => return Ok(PromptAction::Decline),
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                return Ok(PromptAction::Cancel)
            }
            _ if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c')) =>
            {
                return Ok(PromptAction::Cancel)
            }
            _ => {}
        }
    }
}

fn read_hidden_line(deadline: Instant) -> Result<String, ElicitationCommandError> {
    let _raw = RawModeGuard::new()?;
    let mut input = String::new();
    loop {
        let event = read_event_until(deadline)?;
        let Event::Key(key) = event else {
            continue;
        };
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            return Err(ElicitationCommandError::new(
                ElicitationErrorKind::Cancelled,
                "elicitation cancelled by user",
            ));
        }
        match key.code {
            KeyCode::Enter => return Ok(input),
            KeyCode::Esc => {
                return Err(ElicitationCommandError::new(
                    ElicitationErrorKind::Cancelled,
                    "elicitation cancelled by user",
                ))
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(character) => input.push(character),
            _ => {}
        }
    }
}

fn read_event_until(deadline: Instant) -> Result<Event, ElicitationCommandError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() || !event::poll(remaining).map_err(io_error)? {
        return Err(ElicitationCommandError::new(
            ElicitationErrorKind::TimedOut,
            "elicitation response timed out",
        ));
    }
    event::read().map_err(io_error)
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> Result<Self, ElicitationCommandError> {
        enable_raw_mode().map_err(io_error)?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

#[derive(Debug, Clone, Copy)]
enum PromptAction {
    Accept,
    Decline,
    Cancel,
}

fn emit_requested(
    output: ElicitationOutputFormat,
    request: &McpElicitationRequest,
) -> Result<(), ElicitationCommandError> {
    let (kind, message, schema) = match request.kind() {
        McpElicitationRequestKind::Form { message, schema } => {
            ("form", message.as_str(), serde_json::to_value(schema).ok())
        }
        McpElicitationRequestKind::Url { message, .. } => ("url", message.as_str(), None),
    };
    let mut event = request_identity(request);
    event["event"] = json!("elicitation.requested");
    event["kind"] = json!(kind);
    event["message"] = json!(message);
    if let Some(schema) = schema {
        event["schema"] = schema;
    }
    match output {
        ElicitationOutputFormat::Human => {
            eprintln!("\nServer requested {kind} elicitation: {message}");
            if let Some(schema) = event.get("schema") {
                eprintln!(
                    "Schema:\n{}",
                    serde_json::to_string_pretty(schema).unwrap_or_else(|_| schema.to_string())
                );
            }
            Ok(())
        }
        ElicitationOutputFormat::Json => write_json_stderr(&event),
        ElicitationOutputFormat::Jsonl => write_json_stdout(&event),
    }
}

fn emit_url_handoff(
    output: ElicitationOutputFormat,
    request: &McpElicitationRequest,
    url: &str,
    elicitation_id: &str,
) -> Result<(), ElicitationCommandError> {
    let mut event = request_identity(request);
    event["event"] = json!("elicitation.url_handoff");
    event["url"] = json!(url);
    event["elicitation_id"] = json!(elicitation_id);
    match output {
        ElicitationOutputFormat::Human => {
            eprintln!("Open this server-provided URL after verifying its destination:");
            eprintln!("{url}");
            Ok(())
        }
        ElicitationOutputFormat::Json => write_json_stderr(&event),
        ElicitationOutputFormat::Jsonl => write_json_stdout(&event),
    }
}

fn emit_state(
    output: ElicitationOutputFormat,
    event_name: &'static str,
    identity: &Value,
) -> Result<(), ElicitationCommandError> {
    let mut event = identity.clone();
    event["event"] = json!(event_name);
    match output {
        ElicitationOutputFormat::Human => {
            eprintln!("{}", event_name.replace('.', " "));
            Ok(())
        }
        ElicitationOutputFormat::Json => write_json_stderr(&event),
        ElicitationOutputFormat::Jsonl => write_json_stdout(&event),
    }
}

fn request_identity(request: &McpElicitationRequest) -> Value {
    json!({
        "instance_id": request.instance_id(),
        "request_id": request.request_id(),
    })
}

fn write_json_stdout(value: &Value) -> Result<(), ElicitationCommandError> {
    println!("{}", serde_json::to_string(value).map_err(json_error)?);
    Ok(())
}

fn write_json_stderr(value: &Value) -> Result<(), ElicitationCommandError> {
    eprintln!("{}", serde_json::to_string(value).map_err(json_error)?);
    Ok(())
}

fn abort_request(
    request: McpElicitationRequest,
    error: ElicitationCommandError,
    output: ElicitationOutputFormat,
) -> Result<(), ElicitationCommandError> {
    let identity = request_identity(&request);
    let _ = request.cancel();
    emit_state(output, error_event(error.kind), &identity)?;
    Err(error)
}

fn response_failure(
    error: McpElicitationResponseError,
    output: ElicitationOutputFormat,
    identity: &Value,
) -> ElicitationCommandError {
    let kind = match error {
        McpElicitationResponseError::TimedOut => ElicitationErrorKind::TimedOut,
        _ => ElicitationErrorKind::InvalidResponse,
    };
    let _ = emit_state(output, error_event(kind), identity);
    ElicitationCommandError::new(kind, error.to_string())
}

fn error_event(kind: ElicitationErrorKind) -> &'static str {
    match kind {
        ElicitationErrorKind::InputRequired => "elicitation.input_required",
        ElicitationErrorKind::Cancelled => "elicitation.cancelled",
        ElicitationErrorKind::TimedOut => "elicitation.timed_out",
        ElicitationErrorKind::InvalidResponse => "elicitation.invalid_response",
    }
}

fn response_error(error: McpElicitationResponseError) -> ElicitationCommandError {
    let kind = match error {
        McpElicitationResponseError::TimedOut => ElicitationErrorKind::TimedOut,
        _ => ElicitationErrorKind::InvalidResponse,
    };
    ElicitationCommandError::new(kind, error.to_string())
}

fn io_error(error: io::Error) -> ElicitationCommandError {
    ElicitationCommandError::new(
        ElicitationErrorKind::InvalidResponse,
        format!("elicitation terminal I/O failed: {error}"),
    )
}

fn json_error(error: serde_json::Error) -> ElicitationCommandError {
    ElicitationCommandError::new(
        ElicitationErrorKind::InvalidResponse,
        format!("failed to encode elicitation event: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_events_never_contain_form_answers() {
        let identity = json!({"instance_id": "test-instance", "request_id": 1});
        let event = {
            let mut event = identity.clone();
            event["event"] = json!("elicitation.accepted");
            event
        };
        let encoded = serde_json::to_string(&event).unwrap();
        assert!(!encoded.contains("secret-answer"));
    }
}
