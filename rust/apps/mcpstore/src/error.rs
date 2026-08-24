//! Unified CLI error model shared by every command.
//!
//! Before this module the taxonomy lived in triplicate (`CallCommandError`,
//! `TaskCommandError`, `ProtocolCommandError`) and service commands had none —
//! they leaked `StoreError`/`String` as generic exit-1 failures. `CliError`
//! centralizes classification (`StoreError` → `ErrorCode` → exit code / label /
//! hint) once, while `Domain` keeps each command family's `event` prefix and
//! `context` carries per-command fields (instance_id, tool_name, task_id).

use clap::ValueEnum;
use mcpstore::error::FailureCode;
use mcpstore::StoreError;
use serde_json::{json, Map, Value};

/// Machine/human output format, shared by all commands that emit results.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Jsonl,
}

impl OutputFormat {
    pub fn is_machine(self) -> bool {
        !matches!(self, OutputFormat::Human)
    }
}

/// Command family, used only to prefix the JSON `event` field so existing
/// per-family event strings (`execution.*`, `task.*`, `protocol.*`) are kept.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Domain {
    Execution,
    Task,
    Protocol,
    Service,
}

impl Domain {
    fn prefix(self) -> &'static str {
        match self {
            Domain::Execution => "execution",
            Domain::Task => "task",
            Domain::Protocol => "protocol",
            Domain::Service => "service",
        }
    }
}

/// The full union of error classes across all commands. Exit codes are stable
/// and never overlap, so an agent can branch on `$?` alone.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidInput,
    ServiceNotFound,
    ConnectionFailed,
    AuthenticationRequired,
    CapabilityUnsupported,
    TaskNotFound,
    TaskResultUnavailable,
    TaskFailed,
    TaskProtocolFailed,
    TaskStateFailed,
    TaskNotCancellable,
    Cancelled,
    TimedOut,
    Disconnected,
    ToolFailed,
    ProtocolFailed,
    ElicitationInputRequired,
    ElicitationCancelled,
    ElicitationTimedOut,
    ElicitationInvalidResponse,
    CommandFailed,
}
impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::ServiceNotFound => "service_not_found",
            Self::ConnectionFailed => "connection_failed",
            Self::AuthenticationRequired => "authentication_required",
            Self::CapabilityUnsupported => "capability_unsupported",
            Self::TaskNotFound => "task_not_found",
            Self::TaskResultUnavailable => "task_result_unavailable",
            Self::TaskFailed => "task_failed",
            Self::TaskProtocolFailed => "task_protocol_failed",
            Self::TaskStateFailed => "task_state_failed",
            Self::TaskNotCancellable => "task_not_cancellable",
            Self::Cancelled => "execution_cancelled",
            Self::TimedOut => "execution_timed_out",
            Self::Disconnected => "execution_disconnected",
            Self::ToolFailed => "tool_failed",
            Self::ProtocolFailed => "protocol_failed",
            Self::ElicitationInputRequired => "input_required",
            Self::ElicitationCancelled => "elicitation_cancelled",
            Self::ElicitationTimedOut => "elicitation_timed_out",
            Self::ElicitationInvalidResponse => "elicitation_invalid_response",
            Self::CommandFailed => "command_failed",
        }
    }

    pub fn exit_code(self) -> i32 {
        match self {
            Self::CommandFailed => 1,
            Self::InvalidInput => 2,
            Self::ServiceNotFound => 10,
            Self::ConnectionFailed => 11,
            Self::AuthenticationRequired => 12,
            Self::CapabilityUnsupported => 20,
            Self::TaskNotFound => 21,
            Self::TaskResultUnavailable => 23,
            Self::TaskFailed => 24,
            Self::TaskProtocolFailed => 25,
            Self::TaskStateFailed => 26,
            Self::TaskNotCancellable => 27,
            Self::Cancelled => 30,
            Self::TimedOut => 31,
            Self::Disconnected => 32,
            Self::ToolFailed => 33,
            Self::ProtocolFailed => 34,
            Self::ElicitationInputRequired => 35,
            Self::ElicitationCancelled => 36,
            Self::ElicitationTimedOut => 37,
            Self::ElicitationInvalidResponse => 38,
        }
    }

    /// A brief next-step suggestion for human output, when one is useful.
    pub fn hint(self) -> Option<&'static str> {
        match self {
            Self::InvalidInput => {
                Some("check the tool schema with `mcpstore tools <target> --schema`")
            }
            Self::ServiceNotFound => Some("run `mcpstore list` to see configured services"),
            Self::ConnectionFailed => {
                Some("run `mcpstore check <target>` or `mcpstore restart <target>`")
            }
            Self::AuthenticationRequired => Some("run `mcpstore auth login <target>`"),
            Self::TimedOut => Some("retry, or raise --timeout / --max-total-timeout"),
            Self::ElicitationInputRequired => {
                Some("re-run without --non-interactive to answer the prompt")
            }
            _ => None,
        }
    }

    /// The single, authoritative `StoreError` classifier. Domain-aware: the task
    /// command family collapses ToolCallFailed and Protocol to match its historical
    /// exit codes (1 and 25, not 33 and 34).
    pub fn from_store(error: &StoreError, domain: Domain) -> Self {
        match error {
            StoreError::ToolNotAvailable { .. } => Self::InvalidInput,
            StoreError::ServiceNotFound(_) => Self::ServiceNotFound,
            StoreError::Auth(_) => Self::AuthenticationRequired,
            StoreError::Transport(error) => match error.code() {
                FailureCode::InvalidInput => Self::InvalidInput,
                FailureCode::ConnectionAuthRequired
                | FailureCode::ConnectionScope
                | FailureCode::AuthFailed
                | FailureCode::OauthProviderFailed
                | FailureCode::SecureStorageUnavailable => Self::AuthenticationRequired,
                FailureCode::CapabilityUnsupported => Self::CapabilityUnsupported,
                FailureCode::CallCancelled => Self::Cancelled,
                FailureCode::CallTimedOut => Self::TimedOut,
                FailureCode::CallDisconnected => Self::Disconnected,
                FailureCode::TaskNotFound => Self::TaskNotFound,
                FailureCode::TaskStateFailed => Self::TaskStateFailed,
                FailureCode::ElicitationInputRequired => Self::ElicitationInputRequired,
                FailureCode::ElicitationCancelled => Self::ElicitationCancelled,
                FailureCode::ElicitationTimedOut => Self::ElicitationTimedOut,
                FailureCode::ElicitationInvalidResponse => Self::ElicitationInvalidResponse,
                FailureCode::ToolFailed | FailureCode::TaskFailed | FailureCode::TaskUnavailable => {
                    match domain {
                        Domain::Task => Self::CommandFailed,
                        _ => Self::ToolFailed,
                    }
                }
                FailureCode::ConfigInvalid | FailureCode::ConnectionUnsupported => Self::InvalidInput,
                FailureCode::SessionNotFound | FailureCode::SessionNotActive => Self::CommandFailed,
                _ => Self::ConnectionFailed,
            },
            StoreError::Cache(_) => Self::TaskStateFailed,
            StoreError::Config(_) | StoreError::State(_) | StoreError::Other(_) => {
                Self::CommandFailed
            }
        }
    }

    /// Best-effort classification from a daemon error string. The daemon wire
    /// protocol only carries a message (no code channel), so recover intent
    /// from well-known substrings, defaulting to `CommandFailed`.
    pub fn from_daemon_message(message: &str) -> Self {
        let lower = message.to_ascii_lowercase();
        if lower.contains("not found") {
            Self::ServiceNotFound
        } else if lower.contains("auth") {
            Self::AuthenticationRequired
        } else if lower.contains("timed out") || lower.contains("timeout") {
            Self::TimedOut
        } else if lower.contains("connect") || lower.contains("daemon not running") {
            Self::ConnectionFailed
        } else {
            Self::CommandFailed
        }
    }
}
/// The single CLI error type. `main.rs` downcasts to this to derive the exit
/// code and render the message. `context` carries per-command fields
/// (instance_id / tool_name / task_id) without the type needing to know them.
#[derive(Debug)]
pub struct CliError {
    format: OutputFormat,
    domain: Domain,
    code: ErrorCode,
    message: String,
    context: Map<String, Value>,
}

impl CliError {
    pub fn new(
        format: OutputFormat,
        domain: Domain,
        code: ErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            format,
            domain,
            code,
            message: message.into(),
            context: Map::new(),
        }
    }

    /// Attach a context field (e.g. `("instance_id", id)`, `("tool_name", name)`).
    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.context.insert(key.to_string(), value.into());
        self
    }

    /// Convenience for the common `execution`-domain shape: a code + message
    /// tagged with the instance and tool the call targeted.
    pub fn for_call(
        format: OutputFormat,
        code: ErrorCode,
        message: impl Into<String>,
        instance_id: impl std::fmt::Display,
        tool_name: impl Into<String>,
    ) -> Self {
        Self::new(format, Domain::Execution, code, message)
            .with("instance_id", instance_id.to_string())
            .with("tool_name", tool_name.into())
    }

    /// Classify a `StoreError` into a `CliError` for the given command family.
    pub fn from_store(error: &StoreError, format: OutputFormat, domain: Domain) -> Self {
        Self::new(
            format,
            domain,
            ErrorCode::from_store(error, domain),
            error.to_string(),
        )
    }

    /// Classify a daemon error string into a `CliError`.
    pub fn from_daemon(message: String, format: OutputFormat, domain: Domain) -> Self {
        let code = ErrorCode::from_daemon_message(&message);
        Self::new(format, domain, code, message)
    }

    /// Convenience for the `execution` domain without instance/tool context.
    pub fn new_execution(
        format: OutputFormat,
        code: ErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self::new(format, Domain::Execution, code, message)
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn exit_code(&self) -> i32 {
        self.code.exit_code()
    }

    fn event(&self) -> String {
        // Preserve the task command's established invalid-input event contract.
        if self.domain == Domain::Task && self.code == ErrorCode::InvalidInput {
            return "task.error".into();
        }
        // Elicitation events are unprefixed (same across all command families).
        let detail = match self.code {
            ErrorCode::ElicitationInputRequired => return "elicitation.input_required".into(),
            ErrorCode::ElicitationCancelled => return "elicitation.cancelled".into(),
            ErrorCode::ElicitationTimedOut => return "elicitation.timed_out".into(),
            ErrorCode::ElicitationInvalidResponse => return "elicitation.invalid_response".into(),
            ErrorCode::Cancelled => "cancelled",
            ErrorCode::TimedOut => "timed_out",
            _ => "failed",
        };
        format!("{}.{}", self.domain.prefix(), detail)
    }

    fn json_value(&self) -> Value {
        let mut root = Map::new();
        root.insert("event".to_string(), Value::String(self.event()));
        root.insert(
            "error".to_string(),
            json!({ "code": self.code.as_str(), "message": self.message }),
        );
        for (key, value) in &self.context {
            root.insert(key.clone(), value.clone());
        }
        Value::Object(root)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.format {
            OutputFormat::Human => match self.code.hint() {
                Some(hint) => write!(
                    f,
                    "{}: {}\n  hint: {}",
                    self.code.as_str(),
                    self.message,
                    hint
                ),
                None => write!(f, "{}: {}", self.code.as_str(), self.message),
            },
            OutputFormat::Json => match serde_json::to_string_pretty(&self.json_value()) {
                Ok(s) => f.write_str(&s),
                Err(_) => write!(f, "{}: {}", self.code.as_str(), self.message),
            },
            OutputFormat::Jsonl => self.json_value().fmt(f),
        }
    }
}

impl std::error::Error for CliError {}
