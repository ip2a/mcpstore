//! Unified error vocabulary and value used across MCPStore.

use crate::auth::AuthRequired;
use crate::config::HandshakeMode;
use crate::identity::InstanceId;
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    InvalidInput,
    ServiceNotFound,
    ConfigInvalid,
    ConnectionUnsupported,
    ConnectionSpawnFailed,
    ConnectionRefused,
    ConnectionTimedOut,
    ConnectionTls,
    ConnectionClosed,
    ConnectionAuthRequired,
    ConnectionScope,
    HandshakeIncompatible,
    HandshakeRejected,
    HandshakeUncorrelated,
    HandshakeFailed,
    NotConnected,
    ToolNotAvailable,
    ToolFailed,
    CallTimedOut,
    CallCancelled,
    CallDisconnected,
    CapabilityUnsupported,
    TaskNotFound,
    TaskUnavailable,
    TaskFailed,
    TaskStateFailed,
    TaskNotCancellable,
    ElicitationInputRequired,
    ElicitationCancelled,
    ElicitationTimedOut,
    ElicitationInvalidResponse,
    AuthFailed,
    OauthProviderFailed,
    SecureStorageUnavailable,
    SessionNotFound,
    SessionNotActive,
    ServiceUnavailable,
    HealthCheckFailed,
    ProbeTimedOut,
    ToolSyncFailed,
    StopFailed,
    OpenapiRequestFailed,
    #[serde(other)]
    Internal,
}

impl FailureCode {
    pub const ALL: [Self; 43] = [
        Self::InvalidInput,
        Self::ServiceNotFound,
        Self::ConfigInvalid,
        Self::ConnectionUnsupported,
        Self::ConnectionSpawnFailed,
        Self::ConnectionRefused,
        Self::ConnectionTimedOut,
        Self::ConnectionTls,
        Self::ConnectionClosed,
        Self::ConnectionAuthRequired,
        Self::ConnectionScope,
        Self::HandshakeIncompatible,
        Self::HandshakeRejected,
        Self::HandshakeUncorrelated,
        Self::HandshakeFailed,
        Self::NotConnected,
        Self::ToolNotAvailable,
        Self::ToolFailed,
        Self::CallTimedOut,
        Self::CallCancelled,
        Self::CallDisconnected,
        Self::CapabilityUnsupported,
        Self::TaskNotFound,
        Self::TaskUnavailable,
        Self::TaskFailed,
        Self::TaskStateFailed,
        Self::TaskNotCancellable,
        Self::ElicitationInputRequired,
        Self::ElicitationCancelled,
        Self::ElicitationTimedOut,
        Self::ElicitationInvalidResponse,
        Self::AuthFailed,
        Self::OauthProviderFailed,
        Self::SecureStorageUnavailable,
        Self::SessionNotFound,
        Self::SessionNotActive,
        Self::ServiceUnavailable,
        Self::HealthCheckFailed,
        Self::ProbeTimedOut,
        Self::ToolSyncFailed,
        Self::StopFailed,
        Self::OpenapiRequestFailed,
        Self::Internal,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::ServiceNotFound => "service_not_found",
            Self::ConfigInvalid => "config_invalid",
            Self::ConnectionUnsupported => "connection_unsupported",
            Self::ConnectionSpawnFailed => "connection_spawn_failed",
            Self::ConnectionRefused => "connection_refused",
            Self::ConnectionTimedOut => "connection_timed_out",
            Self::ConnectionTls => "connection_tls",
            Self::ConnectionClosed => "connection_closed",
            Self::ConnectionAuthRequired => "connection_auth_required",
            Self::ConnectionScope => "connection_scope",
            Self::HandshakeIncompatible => "handshake_incompatible",
            Self::HandshakeRejected => "handshake_rejected",
            Self::HandshakeUncorrelated => "handshake_uncorrelated",
            Self::HandshakeFailed => "handshake_failed",
            Self::NotConnected => "not_connected",
            Self::ToolNotAvailable => "tool_not_available",
            Self::ToolFailed => "tool_failed",
            Self::CallTimedOut => "call_timed_out",
            Self::CallCancelled => "call_cancelled",
            Self::CallDisconnected => "call_disconnected",
            Self::CapabilityUnsupported => "capability_unsupported",
            Self::TaskNotFound => "task_not_found",
            Self::TaskUnavailable => "task_unavailable",
            Self::TaskFailed => "task_failed",
            Self::TaskStateFailed => "task_state_failed",
            Self::TaskNotCancellable => "task_not_cancellable",
            Self::ElicitationInputRequired => "elicitation_input_required",
            Self::ElicitationCancelled => "elicitation_cancelled",
            Self::ElicitationTimedOut => "elicitation_timed_out",
            Self::ElicitationInvalidResponse => "elicitation_invalid_response",
            Self::AuthFailed => "auth_failed",
            Self::OauthProviderFailed => "oauth_provider_failed",
            Self::SecureStorageUnavailable => "secure_storage_unavailable",
            Self::SessionNotFound => "session_not_found",
            Self::SessionNotActive => "session_not_active",
            Self::ServiceUnavailable => "service_unavailable",
            Self::HealthCheckFailed => "health_check_failed",
            Self::ProbeTimedOut => "probe_timed_out",
            Self::ToolSyncFailed => "tool_sync_failed",
            Self::StopFailed => "stop_failed",
            Self::OpenapiRequestFailed => "openapi_request_failed",
            Self::Internal => "internal",
        }
    }

    pub const fn category(self) -> FailureCategory {
        match self {
            Self::InvalidInput | Self::ServiceNotFound | Self::ConfigInvalid => {
                FailureCategory::Config
            }
            Self::ConnectionUnsupported
            | Self::ConnectionSpawnFailed
            | Self::ConnectionRefused
            | Self::ConnectionTimedOut
            | Self::ConnectionTls
            | Self::ConnectionClosed
            | Self::ConnectionAuthRequired
            | Self::ConnectionScope => FailureCategory::Connection,
            Self::HandshakeIncompatible
            | Self::HandshakeRejected
            | Self::HandshakeUncorrelated
            | Self::HandshakeFailed => FailureCategory::Handshake,
            Self::NotConnected
            | Self::ToolNotAvailable
            | Self::ToolFailed
            | Self::CallTimedOut
            | Self::CallCancelled
            | Self::CallDisconnected
            | Self::CapabilityUnsupported => FailureCategory::Invocation,
            Self::TaskNotFound
            | Self::TaskUnavailable
            | Self::TaskFailed
            | Self::TaskStateFailed
            | Self::TaskNotCancellable => FailureCategory::Task,
            Self::ElicitationInputRequired
            | Self::ElicitationCancelled
            | Self::ElicitationTimedOut
            | Self::ElicitationInvalidResponse => FailureCategory::Elicitation,
            Self::AuthFailed | Self::OauthProviderFailed | Self::SecureStorageUnavailable => {
                FailureCategory::Auth
            }
            Self::SessionNotFound | Self::SessionNotActive => FailureCategory::Session,
            Self::ServiceUnavailable
            | Self::HealthCheckFailed
            | Self::ProbeTimedOut
            | Self::ToolSyncFailed
            | Self::StopFailed
            | Self::OpenapiRequestFailed => FailureCategory::Runtime,
            Self::Internal => FailureCategory::Internal,
        }
    }

    pub const fn policy(self) -> RecoveryPolicy {
        match self {
            Self::ConnectionSpawnFailed
            | Self::ConnectionRefused
            | Self::ConnectionTimedOut
            | Self::ConnectionClosed
            | Self::HandshakeFailed
            | Self::NotConnected
            | Self::CallDisconnected
            | Self::TaskUnavailable
            | Self::ServiceUnavailable
            | Self::HealthCheckFailed
            | Self::ProbeTimedOut
            | Self::ToolSyncFailed
            | Self::OpenapiRequestFailed => RecoveryPolicy::Retry,
            Self::ConnectionAuthRequired
            | Self::ConnectionScope
            | Self::AuthFailed
            | Self::OauthProviderFailed => RecoveryPolicy::ReAuth,
            Self::HandshakeIncompatible | Self::HandshakeRejected | Self::HandshakeUncorrelated => {
                RecoveryPolicy::HandshakeFallback
            }
            _ => RecoveryPolicy::None,
        }
    }

    pub const fn hint(self) -> Option<&'static str> {
        match self {
            Self::InvalidInput => {
                Some("check the tool schema with `mcpstore tools <target> --schema`")
            }
            Self::ServiceNotFound => Some("run `mcpstore list` to see configured services"),
            Self::ConnectionSpawnFailed
            | Self::ConnectionRefused
            | Self::ConnectionTimedOut
            | Self::ConnectionClosed
            | Self::NotConnected
            | Self::ServiceUnavailable => {
                Some("run `mcpstore check <target>` or `mcpstore restart <target>`")
            }
            Self::ConnectionAuthRequired | Self::ConnectionScope | Self::AuthFailed => {
                Some("run `mcpstore auth login <target>`")
            }
            Self::HandshakeIncompatible | Self::HandshakeRejected | Self::HandshakeUncorrelated => {
                Some("retry with handshake_mode=initialize")
            }
            Self::CallTimedOut => Some("retry, or raise --timeout / --max-total-timeout"),
            Self::ElicitationInputRequired => {
                Some("re-run without --non-interactive to answer the prompt")
            }
            _ => None,
        }
    }
}

impl fmt::Display for FailureCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCategory {
    Config,
    Connection,
    Handshake,
    Invocation,
    Task,
    Elicitation,
    Auth,
    Session,
    Runtime,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPolicy {
    None,
    Retry,
    ReAuth,
    HandshakeFallback,
}

impl RecoveryPolicy {
    pub const fn is_retry(self) -> bool {
        matches!(self, Self::Retry)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ErrorContext {
    None,
    Service {
        instance_id: InstanceId,
        service_name: String,
    },
    Tool {
        instance_id: InstanceId,
        tool_name: String,
    },
    Handshake {
        mode: HandshakeMode,
        rpc_code: Option<i64>,
        expected_id: Option<String>,
        received_id: Option<String>,
    },
    Task {
        task_id: String,
    },
    Session {
        session_key: String,
    },
    Scope {
        instance_id: InstanceId,
        #[serde(skip_serializing_if = "Option::is_none")]
        required_scope: Option<String>,
    },
    Auth {
        required: AuthRequired,
    },
}

#[derive(Debug)]
pub struct Error {
    code: FailureCode,
    message: String,
    context: ErrorContext,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

impl Error {
    pub fn new(code: FailureCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: ErrorContext::None,
            source: None,
        }
    }

    pub fn with_context(mut self, context: ErrorContext) -> Self {
        self.context = context;
        self
    }

    pub fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub const fn code(&self) -> FailureCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn context(&self) -> &ErrorContext {
        &self.context
    }

    pub const fn retryable(&self) -> bool {
        self.code.policy().is_retry()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn StdError + 'static))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_has_stable_serialization_and_policy() {
        for code in FailureCode::ALL {
            assert_eq!(
                serde_json::to_string(&code).unwrap(),
                format!("\"{}\"", code.as_str())
            );
            assert_eq!(
                serde_json::from_str::<FailureCode>(&format!("\"{}\"", code.as_str())).unwrap(),
                code
            );
            assert!(matches!(
                code.policy(),
                RecoveryPolicy::None
                    | RecoveryPolicy::Retry
                    | RecoveryPolicy::ReAuth
                    | RecoveryPolicy::HandshakeFallback
            ));
        }
    }

    #[test]
    fn unknown_code_deserializes_as_internal() {
        assert_eq!(
            serde_json::from_str::<FailureCode>("\"future_failure\"").unwrap(),
            FailureCode::Internal
        );
    }

    #[test]
    fn error_preserves_code_message_context_and_source() {
        let error = Error::new(FailureCode::ConnectionRefused, "connection refused")
            .with_context(ErrorContext::Session {
                session_key: "session-1".to_string(),
            })
            .with_source(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "socket refused",
            ));

        assert_eq!(error.code(), FailureCode::ConnectionRefused);
        assert_eq!(error.message(), "connection refused");
        assert!(
            matches!(error.context(), ErrorContext::Session { session_key } if session_key == "session-1")
        );
        assert!(error.source().is_some());
        assert!(error.retryable());
        assert_eq!(error.to_string(), "connection_refused: connection refused");
    }
}
