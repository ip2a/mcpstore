//! CLI error rendering: one failure code → one exit code, one output shape.
//!
//! Commands propagate `mcpstore::Error`; `cli_app::run` renders the final
//! error here. The JSON shape is `{"error": {code, message, hint}}` with the
//! error context flattened onto the root object — no `event` field, no
//! per-command-family exit codes.

use clap::ValueEnum;
use mcpstore::error::{Error, ErrorContext, FailureCode};
use mcpstore::InstanceId;
use serde_json::json;

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

/// Exit code per failure code (cluster-banded: config 2/1x, connection 2x,
/// handshake 3x, invocation 4x, task 5x, elicitation 6x, auth 7x, session 8x,
/// runtime 9x, internal 99).
pub fn exit_code(code: FailureCode) -> i32 {
    use FailureCode as Code;
    match code {
        Code::InvalidInput => 2,
        Code::ServiceNotFound => 10,
        Code::ConfigInvalid => 11,
        Code::ConnectionUnsupported => 20,
        Code::ConnectionSpawnFailed => 21,
        Code::ConnectionRefused => 22,
        Code::ConnectionTimedOut => 23,
        Code::ConnectionTls => 24,
        Code::ConnectionClosed => 25,
        Code::ConnectionAuthRequired => 26,
        Code::ConnectionScope => 27,
        Code::HandshakeIncompatible => 30,
        Code::HandshakeRejected => 31,
        Code::HandshakeUncorrelated => 32,
        Code::HandshakeFailed => 33,
        Code::NotConnected => 40,
        Code::ToolNotAvailable => 41,
        Code::ToolFailed => 42,
        Code::CallTimedOut => 43,
        Code::CallCancelled => 44,
        Code::CallDisconnected => 45,
        Code::CapabilityUnsupported => 46,
        Code::TaskNotFound => 50,
        Code::TaskUnavailable => 51,
        Code::TaskFailed => 52,
        Code::TaskStateFailed => 53,
        Code::TaskNotCancellable => 54,
        Code::ElicitationInputRequired => 60,
        Code::ElicitationCancelled => 61,
        Code::ElicitationTimedOut => 62,
        Code::ElicitationInvalidResponse => 63,
        Code::AuthFailed => 70,
        Code::OauthProviderFailed => 71,
        Code::SecureStorageUnavailable => 72,
        Code::SessionNotFound => 80,
        Code::SessionNotActive => 81,
        Code::ServiceUnavailable => 90,
        Code::HealthCheckFailed => 91,
        Code::ProbeTimedOut => 92,
        Code::ToolSyncFailed => 93,
        Code::StopFailed => 94,
        Code::OpenapiRequestFailed => 95,
        Code::Internal => 99,
    }
}

/// Render an error in the requested output format.
pub fn render(error: &Error, format: OutputFormat) -> String {
    let code = error.code();
    match format {
        OutputFormat::Human => match code.hint() {
            Some(hint) => format!("{code}: {}\n  hint: {hint}", error.message()),
            None => format!("{code}: {}", error.message()),
        },
        OutputFormat::Json | OutputFormat::Jsonl => {
            let mut payload = json!({
                "error": {
                    "code": code.as_str(),
                    "message": error.message(),
                    "hint": code.hint(),
                    "retryable": error.retryable(),
                },
            });
            if let (Some(object), Ok(context)) = (
                payload.as_object_mut(),
                serde_json::to_value(error.context()),
            ) {
                if let Some(context) = context.as_object() {
                    for (key, value) in context {
                        object.insert(key.clone(), value.clone());
                    }
                }
            }
            if format == OutputFormat::Json {
                serde_json::to_string_pretty(&payload)
                    .unwrap_or_else(|_| format!("{code}: {}", error.message()))
            } else {
                payload.to_string()
            }
        }
    }
}

/// Attach service context at CLI sites that only know the instance id.
/// Keeps any richer context already carried by the error.
pub(crate) fn attach_instance(error: Error, instance_id: InstanceId) -> Error {
    if matches!(error.context(), ErrorContext::None) {
        return error.with_context(ErrorContext::Service {
            instance_id,
            service_name: String::new(),
        });
    }
    error
}

/// Attach task context at CLI sites; keeps richer existing context.
pub(crate) fn attach_task(error: Error, task_id: impl Into<String>) -> Error {
    if matches!(error.context(), ErrorContext::None) {
        return error.with_context(ErrorContext::Task {
            task_id: task_id.into(),
        });
    }
    error
}

/// Attach tool context at CLI sites; keeps richer existing context.
pub(crate) fn attach_tool(
    error: Error,
    instance_id: InstanceId,
    tool_name: impl Into<String>,
) -> Error {
    if matches!(error.context(), ErrorContext::None) {
        return error.with_context(ErrorContext::Tool {
            instance_id,
            tool_name: tool_name.into(),
        });
    }
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_has_an_exit_code() {
        for code in FailureCode::ALL {
            let exit = exit_code(code);
            assert!(exit > 1 && exit < 100, "{code} has no exit code");
        }
    }

    #[test]
    fn json_error_shape_has_code_message_and_flattened_context() {
        let error = Error::new(
            FailureCode::ToolNotAvailable,
            "tool is not available: search",
        )
        .with_context(ErrorContext::Tool {
            instance_id: "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap(),
            tool_name: "search".to_string(),
        });
        let payload: serde_json::Value =
            serde_json::from_str(&render(&error, OutputFormat::Jsonl)).unwrap();
        assert_eq!(payload["error"]["code"], "tool_not_available");
        assert_eq!(payload["error"]["message"], "tool is not available: search");
        assert_eq!(payload["tool_name"], "search");
        assert!(payload.get("event").is_none());
    }

    #[test]
    fn human_render_appends_hint() {
        let error = Error::new(FailureCode::ServiceNotFound, "no such service");
        assert_eq!(
            render(&error, OutputFormat::Human),
            "service_not_found: no such service\n  hint: run `mcpstore list` to see configured services"
        );
    }
}
