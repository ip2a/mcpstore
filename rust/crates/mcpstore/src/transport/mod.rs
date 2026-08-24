//! Transport layer: connections to MCP services (stdio and streamable-http).
//!
//! Logging conventions: subscriber output shows tracing targets, so never
//! hand-write prefixes like `[TRANSPORT]` in messages — filter by target
//! (e.g. `RUST_LOG=mcpstore::transport::http=debug`) instead. New logs on
//! connection paths carry `service` / `instance_id` fields so failures are
//! greppable.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub mod client;
mod content;
mod elicitation;
mod execution;
mod handler;
mod http;
mod oauth;
mod pool;
mod prompts;
mod protocol;
mod resources;
mod stdio;
mod task_state;
mod tasks;
mod tools;

pub(crate) use elicitation::McpElicitationController;
pub use elicitation::{
    validate_form_response, validate_handoff_url, McpElicitationRequest, McpElicitationRequestKind,
    McpElicitationResponseError, McpElicitationSession, McpElicitationSessionOptions,
};
pub use execution::{
    McpExecutionOptions, McpExecutionProgress, McpExecutionUpdate, McpToolExecutionHandle,
};
pub use protocol::{
    McpCompletion, McpCompletionReference, McpCompletionRequest, McpServerCapabilities,
    McpServerImplementation, McpServerMetadata,
};
pub use task_state::McpTaskRecord;
pub(crate) use task_state::TaskStateStore;
pub use tasks::{McpTask, McpTaskStatus, McpToolExecution};

use crate::config::HandshakeMode;

/// Maps a configured [`HandshakeMode`] to the rmcp lifecycle mode used at the
/// transport layer. Only `discover`/`initialize`/`auto` (2026-07-28 preferred)
/// are produced here; no fallback is applied — the selected mode wins and a
/// failure is surfaced as a normal connection error.
pub(crate) fn client_lifecycle_mode(mode: HandshakeMode) -> rmcp::service::ClientLifecycleMode {
    let preferred_versions = vec![rmcp::model::ProtocolVersion::V_2026_07_28];
    match mode {
        HandshakeMode::Auto => rmcp::service::ClientLifecycleMode::Auto {
            preferred_versions,
            legacy_version: None,
        },
        HandshakeMode::Discover => {
            rmcp::service::ClientLifecycleMode::Discover { preferred_versions }
        }
        HandshakeMode::Initialize => rmcp::service::ClientLifecycleMode::Initialize,
    }
}

/// Classifies a rmcp handshake failure into the unified error value.
/// Structured rmcp variants map directly (no string matching); the ids and
/// JSON-RPC code ride along in `ErrorContext::Handshake` for logs and the
/// fallback executor.
pub(crate) fn handshake_error(
    mode: crate::config::HandshakeMode,
    error: rmcp::service::ClientInitializeError,
) -> crate::error::Error {
    use crate::error::{Error, ErrorContext, FailureCode};
    use rmcp::model::ErrorCode;
    use rmcp::service::ClientInitializeError;

    let (code, rpc_code, expected_id, received_id): (
        FailureCode,
        Option<i32>,
        Option<String>,
        Option<String>,
    ) = match &error {
        ClientInitializeError::UncorrelatedErrorResponse { expected, received } => (
            FailureCode::HandshakeUncorrelated,
            None,
            Some(expected.to_string()),
            Some(received.to_string()),
        ),
        ClientInitializeError::ConflictInitResponseId(expected, received) => (
            FailureCode::HandshakeUncorrelated,
            None,
            Some(expected.to_string()),
            Some(received.to_string()),
        ),
        ClientInitializeError::JsonRpcError(data) => {
            let rpc = data.code.0;
            let code = if rpc == ErrorCode::METHOD_NOT_FOUND.0 {
                FailureCode::HandshakeIncompatible
            } else if rpc == ErrorCode::INVALID_REQUEST.0 || rpc == ErrorCode::INVALID_PARAMS.0 {
                FailureCode::HandshakeRejected
            } else {
                FailureCode::HandshakeFailed
            };
            (code, Some(rpc), None, None)
        }
        // Version negotiation failed against a discover-only offer set: a
        // legacy initialize handshake may still succeed.
        ClientInitializeError::NoCompatibleProtocolVersion { .. } => {
            (FailureCode::HandshakeIncompatible, None, None, None)
        }
        // rmcp's Auto already exhausted discover→initialize internally;
        // re-falling back would be a third attempt, so classify as a
        // generic handshake failure (Retry policy, not fallback).
        ClientInitializeError::LegacyFallbackFailed { discover, fallback } => {
            return Error::new(
                    FailureCode::HandshakeFailed,
                    format!("handshake failed after fallback (discover: {discover}; initialize: {fallback})"),
                )
                .with_context(ErrorContext::Handshake {
                    mode,
                    rpc_code: None,
                    expected_id: None,
                    received_id: None,
                })
                .with_source(error);
        }
        ClientInitializeError::ConnectionClosed(_) | ClientInitializeError::Cancelled => {
            (FailureCode::ConnectionClosed, None, None, None)
        }
        ClientInitializeError::TransportError { .. } => {
            (FailureCode::ConnectionRefused, None, None, None)
        }
        ClientInitializeError::ExpectedInitResponse(_)
        | ClientInitializeError::ExpectedInitResult(_)
        | ClientInitializeError::NoPreferredProtocolVersion => {
            (FailureCode::HandshakeFailed, None, None, None)
        }
        #[allow(unreachable_patterns)]
        _ => (FailureCode::HandshakeFailed, None, None, None),
    };
    Error::new(code, format!("HTTP MCP handshake failed: {error}"))
        .with_context(ErrorContext::Handshake {
            mode,
            rpc_code: rpc_code.map(i64::from),
            expected_id,
            received_id,
        })
        .with_source(error)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl From<rmcp::model::Tool> for DiscoveredTool {
    fn from(tool: rmcp::model::Tool) -> Self {
        Self {
            name: tool.name.to_string(),
            title: tool.title,
            description: tool.description.unwrap_or_default().to_string(),
            input_schema: serde_json::to_value(&tool.input_schema).unwrap_or_default(),
            output_schema: tool
                .output_schema
                .as_ref()
                .and_then(|schema| serde_json::to_value(schema).ok()),
            annotations: tool
                .annotations
                .as_ref()
                .and_then(|annotations| serde_json::to_value(annotations).ok()),
            icons: tool
                .icons
                .as_ref()
                .and_then(|icons| serde_json::to_value(icons).ok()),
            meta: tool
                .meta
                .as_ref()
                .and_then(|meta| serde_json::to_value(meta).ok()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredResource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredResourceTemplate {
    pub uri_template: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredPrompt {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallResult {
    pub content: Vec<ContentItem>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentItem {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<serde_json::Value>,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
    },
    #[serde(rename = "image")]
    Image {
        data: String,
        mime_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<serde_json::Value>,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
    },
    #[serde(rename = "audio")]
    Audio {
        data: String,
        mime_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<serde_json::Value>,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
    },
    #[serde(rename = "resource")]
    Resource {
        resource: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<serde_json::Value>,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
    },
    #[serde(rename = "resource_link")]
    ResourceLink {
        resource: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<serde_json::Value>,
    },
}
