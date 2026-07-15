use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

pub mod client;
mod content;
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

pub use protocol::{
    McpCompletion, McpCompletionReference, McpCompletionRequest, McpLoggingLevel,
    McpServerCapabilities, McpServerImplementation, McpServerMetadata,
};
pub use task_state::McpTaskRecord;
pub(crate) use task_state::TaskStateStore;
pub use tasks::{McpTask, McpTaskStatus, McpToolExecution};

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("{0}")]
    AuthRequired(crate::auth::AuthRequired),
    #[error("insufficient OAuth scope for service instance {instance_id}")]
    InsufficientScope {
        instance_id: crate::identity::InstanceId,
        required_scope: Option<String>,
    },
    #[error("MCP service instance {instance_id} does not support capability {capability}")]
    CapabilityUnsupported {
        instance_id: crate::identity::InstanceId,
        capability: &'static str,
    },
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Not connected: {0}")]
    NotConnected(String),
    #[error("Tool call failed: {0}")]
    ToolCallFailed(String),
    #[error("MCP protocol error: {0}")]
    Protocol(String),
    #[error("task state error: {0}")]
    TaskState(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, TransportError>;

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
    pub execution: Option<Value>,
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
            execution: tool
                .execution
                .as_ref()
                .and_then(|execution| serde_json::to_value(execution).ok()),
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
