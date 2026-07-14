use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

#[allow(deprecated)]
use rmcp::model::LoggingMessageNotificationParam;
use rmcp::model::{
    CancelledNotificationParam, ClientCapabilities, ClientInfo, CustomNotification, Implementation,
    ProgressNotificationParam, ReadResourceRequestParams, ResourceUpdatedNotificationParam,
};
use rmcp::service::{NotificationContext, Peer, RoleClient};
use rmcp::ClientHandler;
use serde::Serialize;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;

use crate::events::types::EventKind;
use crate::events::{Event, EventBus};
use crate::identity::InstanceId;
use crate::registry::{ServiceRegistry, ToolInfo};
use crate::transport::{DiscoveredPrompt, DiscoveredResource, DiscoveredResourceTemplate};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpClientRuntimeSnapshot {
    pub resources: Vec<DiscoveredResource>,
    pub resource_templates: Vec<DiscoveredResourceTemplate>,
    pub resource_contents: HashMap<String, serde_json::Value>,
    pub prompts: Vec<DiscoveredPrompt>,
}

#[derive(Clone)]
pub(crate) struct McpStoreClientHandler {
    instance_id: InstanceId,
    registry: ServiceRegistry,
    event_bus: EventBus,
    runtime: Arc<RwLock<McpClientRuntimeSnapshot>>,
    notification_work: Arc<Mutex<JoinSet<()>>>,
}

impl std::fmt::Debug for McpStoreClientHandler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpStoreClientHandler")
            .field("instance_id", &self.instance_id)
            .finish_non_exhaustive()
    }
}

impl McpStoreClientHandler {
    pub(crate) fn new(
        instance_id: InstanceId,
        registry: ServiceRegistry,
        event_bus: EventBus,
    ) -> Self {
        Self {
            instance_id,
            registry,
            event_bus,
            runtime: Arc::new(RwLock::new(McpClientRuntimeSnapshot::default())),
            notification_work: Arc::new(Mutex::new(JoinSet::new())),
        }
    }

    #[cfg(test)]
    pub(crate) async fn snapshot(&self) -> McpClientRuntimeSnapshot {
        self.runtime.read().await.clone()
    }

    async fn track_notification_work(&self, work: impl Future<Output = ()> + Send + 'static) {
        let mut tasks = self.notification_work.lock().await;
        while tasks.try_join_next().is_some() {}
        tasks.spawn(work);
    }

    pub(crate) async fn shutdown(&self) {
        let mut tasks = self.notification_work.lock().await;
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }

    async fn publish(&self, kind: EventKind, payload: serde_json::Value) {
        self.event_bus
            .publish(
                Event::new(
                    kind.as_str(),
                    serde_json::json!({
                        "instanceId": self.instance_id,
                        "notification": payload,
                    }),
                ),
                true,
            )
            .await;
    }

    async fn publish_refresh_failure(&self, notification: &'static str, error: impl ToString) {
        let error = error.to_string();
        tracing::warn!(
            instance_id = %self.instance_id,
            notification,
            error = %error,
            "failed to refresh MCP state after server notification"
        );
        self.publish(
            EventKind::McpNotificationFailed,
            serde_json::json!({
                "method": notification,
                "error": error,
            }),
        )
        .await;
    }

    async fn refresh_tools(&self, peer: &Peer<RoleClient>) {
        let tools = match peer.list_all_tools().await {
            Ok(tools) => tools,
            Err(error) => {
                self.publish_refresh_failure("notifications/tools/list_changed", error)
                    .await;
                return;
            }
        };
        let discovered = tools
            .into_iter()
            .map(crate::transport::DiscoveredTool::from)
            .collect::<Vec<_>>();
        let registry_tools = discovered
            .iter()
            .cloned()
            .map(ToolInfo::from)
            .collect::<Vec<_>>();
        if !self
            .registry
            .replace_instance_tools(self.instance_id, registry_tools)
            .await
        {
            self.publish_refresh_failure(
                "notifications/tools/list_changed",
                "service instance is no longer registered",
            )
            .await;
            return;
        }
        self.publish(
            EventKind::McpToolsChanged,
            serde_json::json!({
                "method": "notifications/tools/list_changed",
                "tools": discovered,
            }),
        )
        .await;
    }

    async fn refresh_resources(&self, peer: &Peer<RoleClient>) {
        let (resources, templates) = tokio::join!(
            peer.list_all_resources(),
            peer.list_all_resource_templates()
        );
        let resources = match resources {
            Ok(resources) => resources,
            Err(error) => {
                self.publish_refresh_failure("notifications/resources/list_changed", error)
                    .await;
                return;
            }
        };
        let templates = match templates {
            Ok(templates) => templates,
            Err(error) => {
                self.publish_refresh_failure("notifications/resources/list_changed", error)
                    .await;
                return;
            }
        };
        let resources = match resources
            .into_iter()
            .map(|resource| {
                serde_json::to_value(resource)
                    .and_then(serde_json::from_value::<DiscoveredResource>)
            })
            .collect::<std::result::Result<Vec<_>, _>>()
        {
            Ok(resources) => resources,
            Err(error) => {
                self.publish_refresh_failure("notifications/resources/list_changed", error)
                    .await;
                return;
            }
        };
        let templates = match templates
            .into_iter()
            .map(|template| {
                serde_json::to_value(template)
                    .and_then(serde_json::from_value::<DiscoveredResourceTemplate>)
            })
            .collect::<std::result::Result<Vec<_>, _>>()
        {
            Ok(templates) => templates,
            Err(error) => {
                self.publish_refresh_failure("notifications/resources/list_changed", error)
                    .await;
                return;
            }
        };
        {
            let mut runtime = self.runtime.write().await;
            runtime.resources.clone_from(&resources);
            runtime.resource_templates.clone_from(&templates);
        }
        self.publish(
            EventKind::McpResourcesChanged,
            serde_json::json!({
                "method": "notifications/resources/list_changed",
                "resources": resources,
                "resourceTemplates": templates,
            }),
        )
        .await;
    }

    async fn refresh_resource(&self, uri: String, peer: &Peer<RoleClient>) {
        let result = match peer
            .read_resource(ReadResourceRequestParams::new(uri.clone()))
            .await
        {
            Ok(result) => result,
            Err(error) => {
                self.publish_refresh_failure("notifications/resources/updated", error)
                    .await;
                return;
            }
        };
        let result = match serde_json::to_value(result) {
            Ok(result) => result,
            Err(error) => {
                self.publish_refresh_failure("notifications/resources/updated", error)
                    .await;
                return;
            }
        };
        self.runtime
            .write()
            .await
            .resource_contents
            .insert(uri.clone(), result.clone());
        self.publish(
            EventKind::McpResourceUpdated,
            serde_json::json!({
                "method": "notifications/resources/updated",
                "uri": uri,
                "resource": result,
            }),
        )
        .await;
    }

    async fn refresh_prompts(&self, peer: &Peer<RoleClient>) {
        let prompts = match peer.list_all_prompts().await {
            Ok(prompts) => prompts,
            Err(error) => {
                self.publish_refresh_failure("notifications/prompts/list_changed", error)
                    .await;
                return;
            }
        };
        let prompts = match prompts
            .into_iter()
            .map(|prompt| {
                serde_json::to_value(prompt).and_then(serde_json::from_value::<DiscoveredPrompt>)
            })
            .collect::<std::result::Result<Vec<_>, _>>()
        {
            Ok(prompts) => prompts,
            Err(error) => {
                self.publish_refresh_failure("notifications/prompts/list_changed", error)
                    .await;
                return;
            }
        };
        self.runtime.write().await.prompts.clone_from(&prompts);
        self.publish(
            EventKind::McpPromptsChanged,
            serde_json::json!({
                "method": "notifications/prompts/list_changed",
                "prompts": prompts,
            }),
        )
        .await;
    }
}

impl ClientHandler for McpStoreClientHandler {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::new(
            ClientCapabilities::default(),
            Implementation::new("mcpstore", env!("CARGO_PKG_VERSION")),
        )
    }

    async fn on_cancelled(
        &self,
        params: CancelledNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.publish(
            EventKind::McpRequestCancelled,
            serde_json::json!({
                "method": "notifications/cancelled",
                "params": params,
            }),
        )
        .await;
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.publish(
            EventKind::McpProgress,
            serde_json::json!({
                "method": "notifications/progress",
                "params": params,
            }),
        )
        .await;
    }

    #[allow(deprecated)]
    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.publish(
            EventKind::McpLogMessage,
            serde_json::json!({
                "method": "notifications/message",
                "params": params,
            }),
        )
        .await;
    }

    async fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        context: NotificationContext<RoleClient>,
    ) {
        let handler = self.clone();
        self.track_notification_work(async move {
            handler.refresh_resource(params.uri, &context.peer).await;
        })
        .await;
    }

    async fn on_resource_list_changed(&self, context: NotificationContext<RoleClient>) {
        let handler = self.clone();
        self.track_notification_work(async move {
            handler.refresh_resources(&context.peer).await;
        })
        .await;
    }

    async fn on_tool_list_changed(&self, context: NotificationContext<RoleClient>) {
        let handler = self.clone();
        self.track_notification_work(async move {
            handler.refresh_tools(&context.peer).await;
        })
        .await;
    }

    async fn on_prompt_list_changed(&self, context: NotificationContext<RoleClient>) {
        let handler = self.clone();
        self.track_notification_work(async move {
            handler.refresh_prompts(&context.peer).await;
        })
        .await;
    }

    async fn on_custom_notification(
        &self,
        notification: CustomNotification,
        _context: NotificationContext<RoleClient>,
    ) {
        self.publish(
            EventKind::McpCustomNotification,
            serde_json::json!({
                "method": notification.method,
                "params": notification.params,
                "recognized": false,
            }),
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;

    use rmcp::model::{
        CancelledNotificationParam, CustomNotification, ListPromptsResult,
        ListResourceTemplatesResult, ListResourcesResult, ListToolsResult, NumberOrString,
        PaginatedRequestParams, ProgressNotificationParam, ProgressToken, Prompt,
        ReadResourceRequestParams, ReadResourceResult, Resource, ResourceContents,
        ResourceTemplate, ServerCapabilities, ServerInfo, ServerNotification, Tool,
    };
    #[allow(deprecated)]
    use rmcp::model::{LoggingLevel, LoggingMessageNotificationParam};
    use rmcp::service::{RequestContext, RoleServer};
    use rmcp::{ClientHandler, ServerHandler, ServiceExt};
    use serde_json::{json, Map};
    use tokio::sync::RwLock;

    use super::*;
    use crate::identity::{ScopeRef, ServiceInstanceKey};
    use crate::registry::{ConfigRevision, ConnectionStatus, ServiceInstance};

    #[derive(Clone)]
    struct NotificationServer {
        tool_name: Arc<RwLock<String>>,
    }

    impl ServerHandler for NotificationServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo::new(
                ServerCapabilities::builder()
                    .enable_tools()
                    .enable_tool_list_changed()
                    .enable_resources()
                    .enable_resources_list_changed()
                    .enable_prompts()
                    .enable_prompts_list_changed()
                    .build(),
            )
        }

        async fn list_tools(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListToolsResult, rmcp::ErrorData> {
            let name = self.tool_name.read().await.clone();
            Ok(ListToolsResult::with_all_items(vec![Tool::new(
                name,
                "notification test tool",
                Arc::new(Map::new()),
            )]))
        }

        async fn list_resources(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListResourcesResult, rmcp::ErrorData> {
            Ok(ListResourcesResult::with_all_items(vec![Resource::new(
                "test://resource",
                "resource",
            )]))
        }

        async fn list_resource_templates(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
            Ok(ListResourceTemplatesResult::with_all_items(vec![
                ResourceTemplate::new("test://{id}", "template"),
            ]))
        }

        async fn read_resource(
            &self,
            request: ReadResourceRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> Result<ReadResourceResult, rmcp::ErrorData> {
            Ok(ReadResourceResult::new(vec![ResourceContents::text(
                "updated",
                request.uri,
            )]))
        }

        async fn list_prompts(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListPromptsResult, rmcp::ErrorData> {
            Ok(ListPromptsResult::with_all_items(vec![Prompt::new(
                "review",
                Some("Review a change"),
                None,
            )]))
        }
    }

    fn instance(service_name: &str) -> ServiceInstance {
        let instance_id = ServiceInstanceKey::new(service_name, ScopeRef::Store).instance_id();
        ServiceInstance {
            instance_id,
            service_name: service_name.to_string(),
            scope: ScopeRef::Store,
            transport: "stdio".to_string(),
            url: None,
            command: Some("test".to_string()),
            status: ConnectionStatus::Connected,
            tools: Vec::new(),
            effective_config: Map::new(),
            config_revision: ConfigRevision {
                base_revision: 1,
                scope_revision: 0,
            },
            applied_config_revision: Some(ConfigRevision {
                base_revision: 1,
                scope_revision: 0,
            }),
            added_time: 1,
        }
    }

    async fn wait_for_event(bus: &EventBus, event_type: &str) {
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if bus
                    .get_history(100)
                    .await
                    .iter()
                    .any(|event| event.event_type == event_type)
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        if result.is_err() {
            panic!(
                "timed out waiting for {event_type}; history={:?}",
                bus.get_history(100).await
            );
        }
    }

    #[test]
    fn client_info_declares_only_capabilities_the_handler_implements() {
        let instance_id = ServiceInstanceKey::new("capabilities", ScopeRef::Store).instance_id();
        let handler =
            McpStoreClientHandler::new(instance_id, ServiceRegistry::new(), EventBus::new());

        let info = ClientHandler::get_info(&handler);
        assert_eq!(info.client_info.name, "mcpstore");
        assert_eq!(info.client_info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.capabilities, ClientCapabilities::default());
        assert!(info.capabilities.sampling.is_none());
        assert!(info.capabilities.roots.is_none());
        assert!(info.capabilities.elicitation.is_none());
        assert!(info.capabilities.tasks.is_none());
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn notifications_refresh_scoped_state_and_publish_domain_events() {
        let registry = ServiceRegistry::new();
        let primary = instance("primary");
        let primary_id = primary.instance_id;
        let untouched = instance("untouched");
        let untouched_id = untouched.instance_id;
        registry.register_instance(primary).await;
        registry.register_instance(untouched).await;
        let event_bus = EventBus::with_history(100);
        let handler = McpStoreClientHandler::new(primary_id, registry.clone(), event_bus.clone());
        let tool_name = Arc::new(RwLock::new("updated_tool".to_string()));
        let server_handler = NotificationServer {
            tool_name: tool_name.clone(),
        };
        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server_start =
            tokio::spawn(async move { server_handler.serve(server_transport).await.unwrap() });
        let client = handler.clone().serve(client_transport).await.unwrap();
        let server = server_start.await.unwrap();

        server.peer().notify_tool_list_changed().await.unwrap();
        wait_for_event(&event_bus, EventKind::McpToolsChanged.as_str()).await;
        assert!(registry
            .find_tool(primary_id, "updated_tool")
            .await
            .is_some());
        assert!(registry.list_instance_tools(untouched_id).await.is_empty());

        server.peer().notify_resource_list_changed().await.unwrap();
        wait_for_event(&event_bus, EventKind::McpResourcesChanged.as_str()).await;
        server
            .peer()
            .notify_resource_updated(ResourceUpdatedNotificationParam::new("test://resource"))
            .await
            .unwrap();
        wait_for_event(&event_bus, EventKind::McpResourceUpdated.as_str()).await;

        server.peer().notify_prompt_list_changed().await.unwrap();
        wait_for_event(&event_bus, EventKind::McpPromptsChanged.as_str()).await;

        server
            .peer()
            .notify_progress(
                ProgressNotificationParam::new(ProgressToken(NumberOrString::Number(7)), 0.5)
                    .with_total(1.0)
                    .with_message("halfway"),
            )
            .await
            .unwrap();
        server
            .peer()
            .notify_logging_message(LoggingMessageNotificationParam::new(
                LoggingLevel::Info,
                json!({"message": "ready"}),
            ))
            .await
            .unwrap();
        server
            .peer()
            .notify_cancelled(CancelledNotificationParam::new(
                Some(NumberOrString::Number(9)),
                Some("superseded".to_string()),
            ))
            .await
            .unwrap();
        server
            .send_notification(ServerNotification::CustomNotification(
                CustomNotification::new("notifications/example", Some(json!({"value": 1}))),
            ))
            .await
            .unwrap();

        for event_type in [
            EventKind::McpProgress.as_str(),
            EventKind::McpLogMessage.as_str(),
            EventKind::McpRequestCancelled.as_str(),
            EventKind::McpCustomNotification.as_str(),
        ] {
            wait_for_event(&event_bus, event_type).await;
        }

        let snapshot = handler.snapshot().await;
        assert_eq!(snapshot.resources[0].uri, "test://resource");
        assert_eq!(snapshot.resource_templates[0].uri_template, "test://{id}");
        assert!(snapshot.resource_contents.contains_key("test://resource"));
        assert_eq!(snapshot.prompts[0].name, "review");

        let history = event_bus.get_history(100).await;
        let event_types = history
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<HashSet<_>>();
        assert!(event_types.contains(EventKind::McpToolsChanged.as_str()));
        assert!(event_types.contains(EventKind::McpResourcesChanged.as_str()));
        assert!(event_types.contains(EventKind::McpResourceUpdated.as_str()));
        assert!(event_types.contains(EventKind::McpPromptsChanged.as_str()));
        let custom = history
            .iter()
            .find(|event| event.event_type == EventKind::McpCustomNotification.as_str())
            .unwrap();
        assert_eq!(
            custom.payload["notification"]["method"],
            "notifications/example"
        );
        assert_eq!(custom.payload["notification"]["recognized"], false);

        client.cancel().await.unwrap();
        handler.shutdown().await;
        let history_len = event_bus.get_history(100).await.len();
        assert!(server.peer().notify_tool_list_changed().await.is_err());
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(event_bus.get_history(100).await.len(), history_len);
        server.cancel().await.unwrap();
    }
}
