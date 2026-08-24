use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use rmcp::model::{
    ArgumentInfo, ClientRequest, CompleteRequest, CompleteRequestParams, CompletionContext,
    CompletionInfo, Reference, ServerPeerInfo, ServerResult, SubscriptionFilter,
};
use rmcp::service::{Peer, PeerRequestOptions, RoleClient};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::identity::InstanceId;
use crate::transport::client::McpConnection;
use crate::transport::execution::map_service_error;
use crate::error::{Error, ErrorContext, FailureCode};
use crate::error::Result;

#[cfg(not(test))]
const PROTOCOL_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
const PROTOCOL_REQUEST_TIMEOUT: Duration = Duration::from_millis(100);

pub(crate) async fn send_protocol_request(
    peer: &Peer<RoleClient>,
    instance_id: InstanceId,
    request: ClientRequest,
    operation: &str,
) -> Result<ServerResult> {
    let handle = peer
        .send_cancellable_request(
            request,
            PeerRequestOptions::with_timeout(PROTOCOL_REQUEST_TIMEOUT),
        )
        .await
        .map_err(|error| map_service_error(instance_id, operation, error))?;
    handle
        .await_response()
        .await
        .map_err(|error| map_service_error(instance_id, operation, error))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpCompletionReference {
    Prompt { name: String },
    Resource { uri_template: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpCompletionRequest {
    pub reference: McpCompletionReference,
    pub argument_name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpCompletion {
    pub values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

impl From<CompletionInfo> for McpCompletion {
    fn from(value: CompletionInfo) -> Self {
        Self {
            values: value.values,
            total: value.total,
            has_more: value.has_more,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerImplementation {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerCapabilities {
    pub tools: bool,
    pub tools_list_changed: bool,
    pub resources: bool,
    pub resources_list_changed: bool,
    pub prompts: bool,
    pub prompts_list_changed: bool,
    pub completions: bool,
    pub tasks: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub experimental: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerMetadata {
    pub protocol_version: String,
    pub server_info: Option<McpServerImplementation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub capabilities: McpServerCapabilities,
}

impl From<&ServerPeerInfo> for McpServerMetadata {
    fn from(info: &ServerPeerInfo) -> Self {
        let capabilities = &info.capabilities;
        Self {
            protocol_version: info.protocol_version.to_string(),
            server_info: info
                .server_info
                .as_ref()
                .map(|server_info| McpServerImplementation {
                    name: server_info.name.clone(),
                    title: server_info.title.clone(),
                    version: server_info.version.clone(),
                    description: server_info.description.clone(),
                    website_url: server_info.website_url.clone(),
                }),
            instructions: info.instructions.clone(),
            capabilities: McpServerCapabilities {
                tools: capabilities.tools.is_some(),
                tools_list_changed: capabilities
                    .tools
                    .as_ref()
                    .and_then(|value| value.list_changed)
                    == Some(true),
                resources: capabilities.resources.is_some(),
                resources_list_changed: capabilities
                    .resources
                    .as_ref()
                    .and_then(|value| value.list_changed)
                    == Some(true),
                prompts: capabilities.prompts.is_some(),
                prompts_list_changed: capabilities
                    .prompts
                    .as_ref()
                    .and_then(|value| value.list_changed)
                    == Some(true),
                completions: capabilities.completions.is_some(),
                tasks: capabilities.supports_tasks(),
                extensions: capabilities
                    .extensions
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, value)| (name, Value::Object(value)))
                    .collect(),
                experimental: capabilities
                    .experimental
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, value)| (name, Value::Object(value)))
                    .collect(),
            },
        }
    }
}

impl McpConnection {
    pub fn server_metadata(&self) -> Result<McpServerMetadata> {
        let info = self.peer_info()?;
        Ok(McpServerMetadata::from(info.as_ref()))
    }

    pub async fn complete(&self, request: McpCompletionRequest) -> Result<McpCompletion> {
        self.require_capability("completions", |info| {
            info.capabilities.completions.is_some()
        })?;
        let context = (!request.context.is_empty())
            .then(|| CompletionContext::with_arguments(request.context));
        let reference = match request.reference {
            McpCompletionReference::Prompt { name } => Reference::for_prompt(name),
            McpCompletionReference::Resource { uri_template } => {
                Reference::for_resource(uri_template)
            }
        };
        let mut params = CompleteRequestParams::new(
            reference,
            ArgumentInfo::new(request.argument_name, request.value),
        );
        if let Some(context) = context {
            params = params.with_context(context);
        }
        match send_protocol_request(
            self.get_client()?,
            self.instance_id(),
            ClientRequest::CompleteRequest(CompleteRequest::new(params)),
            "completion",
        )
        .await
        {
            Ok(ServerResult::CompleteResult(result)) => Ok(result.completion.into()),
            Ok(_) => Err(Error::new(
                FailureCode::ToolFailed,
                "completion returned an unexpected response",
            )),
            Err(error) => Err(self.classify_client_failure(error).await),
        }
    }

    pub async fn complete_prompt_argument(
        &self,
        prompt_name: impl Into<String>,
        argument_name: impl Into<String>,
        value: impl Into<String>,
        context: HashMap<String, String>,
    ) -> Result<McpCompletion> {
        self.complete(McpCompletionRequest {
            reference: McpCompletionReference::Prompt {
                name: prompt_name.into(),
            },
            argument_name: argument_name.into(),
            value: value.into(),
            context,
        })
        .await
    }

    pub async fn complete_resource_argument(
        &self,
        uri_template: impl Into<String>,
        argument_name: impl Into<String>,
        value: impl Into<String>,
        context: HashMap<String, String>,
    ) -> Result<McpCompletion> {
        self.complete(McpCompletionRequest {
            reference: McpCompletionReference::Resource {
                uri_template: uri_template.into(),
            },
            argument_name: argument_name.into(),
            value: value.into(),
            context,
        })
        .await
    }

    pub async fn refresh_subscription(&mut self, uris: &HashSet<String>) -> Result<()> {
        if let Some(task) = self.subscription_task.take() {
            task.abort();
            let _ = task.await;
        }

        let info = self.peer_info()?;
        // subscriptions/listen is a 2026-07-28 method. Legacy servers push
        // listChanged notifications automatically after initialize, so skip
        // the explicit listen call for older protocol versions.
        if info.protocol_version.as_str() < rmcp::model::ProtocolVersion::V_2026_07_28.as_str() {
            return Ok(());
        }
        let mut builder = SubscriptionFilter::builder()
            .tools_list_changed()
            .prompts_list_changed()
            .resources_list_changed();
        if !uris.is_empty() {
            builder = builder.resource_subscriptions(uris.iter().cloned());
        }
        let filter = builder.build().supported_by(&info.capabilities);
        if filter.tools_list_changed.is_none()
            && filter.prompts_list_changed.is_none()
            && filter.resources_list_changed.is_none()
            && filter.resource_subscriptions.is_none()
        {
            return Ok(());
        }

        let client = self.get_client()?;
        let peer = client.peer().clone();
        let mut subscription = client
            .listen(filter)
            .await
            .map_err(|error| map_service_error(self.instance_id(), "subscription listen", error))?;
        let handler = self.handler.clone();
        let instance_id = self.instance_id();
        self.subscription_task = Some(tokio::spawn(async move {
            loop {
                match subscription.next().await {
                    Ok(Some(notification)) => {
                        handler
                            .handle_subscription_notification(notification, &peer)
                            .await;
                    }
                    Ok(None) => break,
                    Err(error) => {
                        tracing::warn!(
                            instance_id = %instance_id,
                            error = %error,
                            "MCP subscription ended with an error"
                        );
                        break;
                    }
                }
            }
        }));
        Ok(())
    }

    pub(in crate::transport) fn require_tools(&self) -> Result<()> {
        self.require_capability("tools", |info| info.capabilities.tools.is_some())
    }

    pub(in crate::transport) fn require_resources(&self) -> Result<()> {
        self.require_capability("resources", |info| info.capabilities.resources.is_some())
    }

    pub(in crate::transport) fn require_prompts(&self) -> Result<()> {
        self.require_capability("prompts", |info| info.capabilities.prompts.is_some())
    }

    pub(in crate::transport) fn require_capability(
        &self,
        capability: &'static str,
        supported: impl FnOnce(&ServerPeerInfo) -> bool,
    ) -> Result<()> {
        let info = self.peer_info()?;
        if supported(info.as_ref()) {
            Ok(())
        } else {
            Err(Error::new(
                FailureCode::CapabilityUnsupported,
                format!(
                    "MCP service instance {} does not support capability {capability}",
                    self.instance_id()
                ),
            )
            .with_context(ErrorContext::Service {
                instance_id: self.instance_id(),
                service_name: String::new(),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerPeerInfo};

    use super::*;

    #[test]
    fn completion_info_maps_without_losing_pagination() {
        let completion = McpCompletion::from(
            CompletionInfo::with_pagination(vec!["rust".to_string()], Some(3), true).unwrap(),
        );

        assert_eq!(completion.values, ["rust"]);
        assert_eq!(completion.total, Some(3));
        assert_eq!(completion.has_more, Some(true));
    }

    #[test]
    fn metadata_uses_discover_peer_info_and_latest_capabilities() {
        let peer = ServerPeerInfo::new(
            ProtocolVersion::V_2026_07_28,
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .enable_tasks()
                .build(),
        )
        .with_server_info(Implementation::new("fixture", "3.1.0"))
        .with_instructions("latest only");

        let metadata = McpServerMetadata::from(&peer);

        assert_eq!(metadata.protocol_version, "2026-07-28");
        assert_eq!(metadata.server_info.as_ref().unwrap().name, "fixture");
        assert_eq!(metadata.instructions.as_deref(), Some("latest only"));
        assert!(metadata.capabilities.tools);
        assert!(metadata.capabilities.resources);
        assert!(metadata.capabilities.prompts);
        assert!(metadata.capabilities.tasks);
    }

    #[test]
    fn metadata_allows_discovery_without_server_identity() {
        let peer =
            ServerPeerInfo::new(ProtocolVersion::V_2026_07_28, ServerCapabilities::default());

        let metadata = McpServerMetadata::from(&peer);

        assert!(metadata.server_info.is_none());
        assert!(!metadata.capabilities.tasks);
    }
}
