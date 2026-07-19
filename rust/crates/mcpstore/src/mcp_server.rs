use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::{
    config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig},
    events::{bus::EventHandler, Event},
    CacheStorage, ContentItem, InstanceId, MCPStore, OpenApiBundleOptions, OpenApiImportOptions,
    OpenApiRefCachePolicy, ScopeRef, SourceMode, StoreError, StoreOptions, ToolTransformPatch,
};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, ContentBlock, GetPromptRequestParams,
        GetPromptResult, Implementation, ListPromptsResult, ListResourceTemplatesResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, Prompt,
        ReadResourceRequestParams, ReadResourceResult, Resource, ResourceContents,
        ResourceTemplate, ServerCapabilities, ServerInfo, Tool, ToolAnnotations,
    },
    serve_server,
    transport::{
        stdio, streamable_http_server::session::local::LocalSessionManager,
        StreamableHttpServerConfig, StreamableHttpService,
    },
    ErrorData, RoleServer, ServerHandler,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// Transport protocol for the MCP server runner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum McpServerTransport {
    #[default]
    Stdio,
    StreamableHttp,
}

impl McpServerTransport {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::StreamableHttp => "streamable-http",
        }
    }
}

/// Alias for the dynamic error type used in the MCP server runner.
pub type BoxErr = Box<dyn std::error::Error>;

#[derive(Clone, Debug)]
pub struct McpServerOptions {
    pub config_path: Option<String>,
    pub source_mode: SourceMode,
    pub backend: Option<CacheStorage>,
    pub redis_url: Option<String>,
    pub namespace: Option<String>,
    pub scope: ScopeRef,
    pub instance_id: Option<InstanceId>,
    pub transport: McpServerTransport,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub session_key: Option<String>,
    pub expose_session_state_tools: bool,
    pub expose_tool_transform_tools: bool,
    pub expose_openapi_tools: bool,
    pub expose_service_tools: bool,
    pub expose_cache_tools: bool,
    pub expose_event_tools: bool,
}

impl Default for McpServerOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            source_mode: SourceMode::Local,
            backend: None,
            redis_url: None,
            namespace: None,
            scope: ScopeRef::Store,
            instance_id: None,
            transport: McpServerTransport::Stdio,
            host: "127.0.0.1".to_string(),
            port: 18300,
            path: "/mcp".to_string(),
            session_key: None,
            expose_session_state_tools: false,
            expose_tool_transform_tools: false,
            expose_openapi_tools: false,
            expose_service_tools: false,
            expose_cache_tools: false,
            expose_event_tools: false,
        }
    }
}

impl McpServerOptions {
    pub fn to_store_options(&self) -> StoreOptions {
        StoreOptions {
            config_path: self.config_path.clone(),
            source_mode: self.source_mode,
            backend: self.backend.clone(),
            redis_url: self.redis_url.clone(),
            namespace: self.namespace.clone(),
        }
    }
}

const SESSION_STATE_LIST_TOOL: &str = "mcpstore_session_state_list";
const SESSION_STATE_GET_TOOL: &str = "mcpstore_session_state_get";
const SESSION_STATE_SET_TOOL: &str = "mcpstore_session_state_set";
const SESSION_STATE_DELETE_TOOL: &str = "mcpstore_session_state_delete";
const SESSION_STATE_CLEAR_TOOL: &str = "mcpstore_session_state_clear";
const SESSION_SNAPSHOT_EXPORT_TOOL: &str = "mcpstore_session_snapshot_export";
const SESSION_SNAPSHOT_IMPORT_TOOL: &str = "mcpstore_session_snapshot_import";
const TOOL_TRANSFORM_LIST_TOOL: &str = "mcpstore_tool_transform_list";
const TOOL_TRANSFORM_GET_TOOL: &str = "mcpstore_tool_transform_get";
const TOOL_TRANSFORM_SET_TOOL: &str = "mcpstore_tool_transform_set";
const TOOL_TRANSFORM_DELETE_TOOL: &str = "mcpstore_tool_transform_delete";
const OPENAPI_IMPORT_LIST_TOOL: &str = "mcpstore_openapi_import_list";
const OPENAPI_IMPORT_GET_TOOL: &str = "mcpstore_openapi_import_get";
const OPENAPI_IMPORT_SET_TOOL: &str = "mcpstore_openapi_import";
const OPENAPI_BUNDLE_TOOL: &str = "mcpstore_openapi_bundle";
const OPENAPI_BUNDLE_ARTIFACT_TOOL: &str = "mcpstore_openapi_bundle_artifact";
const SERVICE_LIST_TOOL: &str = "mcpstore_scope_instance_list";
const SERVICE_INFO_TOOL: &str = "mcpstore_service_scope_info";
const SERVICE_STATE_TOOL: &str = "mcpstore_service_state";
const SERVICE_CHECK_TOOL: &str = "mcpstore_instance_check";
const SERVICE_ADD_TOOL: &str = "mcpstore_service_definition_add";
const SERVICE_PATCH_TOOL: &str = "mcpstore_service_scope_set";
const SERVICE_REMOVE_TOOL: &str = "mcpstore_service_scope_remove";
const SERVICE_CONNECT_TOOL: &str = "mcpstore_instance_connect";
const SERVICE_DISCONNECT_TOOL: &str = "mcpstore_instance_disconnect";
const SERVICE_RESTART_TOOL: &str = "mcpstore_instance_restart";
const SERVICE_WAIT_TOOL: &str = "mcpstore_instance_wait";
const CACHE_HEALTH_TOOL: &str = "mcpstore_cache_health";
const CACHE_INSPECT_TOOL: &str = "mcpstore_cache_inspect";
const CACHE_SWITCH_TOOL: &str = "mcpstore_cache_switch";
const EVENT_HISTORY_TOOL: &str = "mcpstore_event_history";
const EVENT_CAPABILITY_REPORT_TOOL: &str = "mcpstore_event_capability_report";

#[derive(Clone)]
struct ToolBinding {
    tool: Tool,
    instance_id: InstanceId,
    tool_name: String,
}

struct AggregateToolsChangedNotification {
    peer: rmcp::service::Peer<RoleServer>,
    instance_id: Option<InstanceId>,
}

#[async_trait::async_trait]
impl EventHandler for AggregateToolsChangedNotification {
    async fn handle(&self, event: &Event) {
        if event.event_type != crate::events::types::EventKind::McpToolsChanged.as_str() {
            return;
        }
        if let Some(instance_id) = self.instance_id {
            let changed_instance = event
                .payload
                .get("instanceId")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<InstanceId>().ok());
            if changed_instance != Some(instance_id) {
                return;
            }
        }
        if let Err(error) = self.peer.notify_tool_list_changed().await {
            tracing::debug!(%error, "aggregate MCP client disconnected before tools/list_changed");
        }
    }

    fn is_alive(&self) -> bool {
        !self.peer.is_transport_closed()
    }
}

#[derive(Clone)]
struct McpStoreServer {
    store: Arc<MCPStore>,
    scope: ScopeRef,
    instance_id: Option<InstanceId>,
    session_key: Option<String>,
    scope_label: String,
    bindings: Arc<HashMap<String, ToolBinding>>,
    session_state_tools: Arc<HashMap<String, Tool>>,
    tool_transform_tools: Arc<HashMap<String, Tool>>,
    openapi_tools: Arc<HashMap<String, Tool>>,
    service_tools: Arc<HashMap<String, Tool>>,
    cache_tools: Arc<HashMap<String, Tool>>,
    event_tools: Arc<HashMap<String, Tool>>,
    tools: Arc<Vec<Tool>>,
}

impl McpStoreServer {
    async fn from_store(
        store: Arc<MCPStore>,
        scope: ScopeRef,
        instance_id: Option<InstanceId>,
        session_key: Option<String>,
        expose_session_state_tools: bool,
        expose_tool_transform_tools: bool,
        expose_openapi_tools: bool,
        expose_service_tools: bool,
        expose_cache_tools: bool,
        expose_event_tools: bool,
    ) -> Result<Self, BoxErr> {
        connect_target_instances(&store, &scope, instance_id).await?;
        if let Some(session_key) = session_key.as_deref() {
            store.session_by_key(session_key).status().await?;
        }
        let bindings =
            build_tool_bindings(&store, &scope, instance_id, session_key.as_deref()).await?;
        let session_state_tools = if expose_session_state_tools {
            build_session_state_tools()
        } else {
            HashMap::new()
        };
        let tool_transform_tools = if expose_tool_transform_tools {
            build_tool_transform_tools()
        } else {
            HashMap::new()
        };
        let openapi_tools = if expose_openapi_tools {
            build_openapi_tools()
        } else {
            HashMap::new()
        };
        let service_tools = if expose_service_tools {
            build_service_tools()
        } else {
            HashMap::new()
        };
        let cache_tools = if expose_cache_tools {
            build_cache_tools()
        } else {
            HashMap::new()
        };
        let event_tools = if expose_event_tools {
            build_event_tools()
        } else {
            HashMap::new()
        };
        for tool_name in session_state_tools
            .keys()
            .chain(tool_transform_tools.keys())
            .chain(openapi_tools.keys())
            .chain(service_tools.keys())
            .chain(cache_tools.keys())
            .chain(event_tools.keys())
        {
            if bindings.contains_key(tool_name) {
                return Err(format!(
                    "MCPStore 管理工具与下游工具重名，无法构建 Rust MCP server: tool={tool_name}"
                )
                .into());
            }
        }
        let mut tools = bindings
            .values()
            .map(|binding| binding.tool.clone())
            .collect::<Vec<_>>();
        tools.extend(session_state_tools.values().cloned());
        tools.extend(tool_transform_tools.values().cloned());
        tools.extend(openapi_tools.values().cloned());
        tools.extend(service_tools.values().cloned());
        tools.extend(cache_tools.values().cloned());
        tools.extend(event_tools.values().cloned());
        tools.sort_by(|left, right| left.name.cmp(&right.name));

        let scope_label = match &scope {
            ScopeRef::Store => "store".to_string(),
            ScopeRef::Agent { agent_id } => format!("agent:{agent_id}"),
        };
        let scope_label = match instance_id {
            Some(instance_id) => format!("{scope_label} instance:{instance_id}"),
            None => scope_label,
        };
        let scope_label = match session_key.as_deref() {
            Some(session_key) => format!("{scope_label} session:{session_key}"),
            None => scope_label,
        };

        Ok(Self {
            store,
            scope,
            instance_id,
            session_key,
            scope_label,
            bindings: Arc::new(bindings),
            session_state_tools: Arc::new(session_state_tools),
            tool_transform_tools: Arc::new(tool_transform_tools),
            openapi_tools: Arc::new(openapi_tools),
            service_tools: Arc::new(service_tools),
            cache_tools: Arc::new(cache_tools),
            event_tools: Arc::new(event_tools),
            tools: Arc::new(tools),
        })
    }

    fn instructions(&self) -> String {
        format!(
            "Rust MCPStore server. scope={} tool_count={}",
            self.scope_label,
            self.tools.len()
        )
    }

    async fn current_bindings(&self) -> Result<HashMap<String, ToolBinding>, ErrorData> {
        build_tool_bindings(
            &self.store,
            &self.scope,
            self.instance_id,
            self.session_key.as_deref(),
        )
        .await
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))
    }

    async fn current_tools(&self) -> Result<Vec<Tool>, ErrorData> {
        let bindings = self.current_bindings().await?;
        for tool_name in self
            .session_state_tools
            .keys()
            .chain(self.tool_transform_tools.keys())
            .chain(self.openapi_tools.keys())
            .chain(self.service_tools.keys())
            .chain(self.cache_tools.keys())
            .chain(self.event_tools.keys())
        {
            if bindings.contains_key(tool_name) {
                return Err(ErrorData::internal_error(
                    format!(
                        "MCPStore 管理工具与下游工具重名，无法构建 Rust MCP server: tool={tool_name}"
                    ),
                    None,
                ));
            }
        }

        let mut tools = bindings
            .values()
            .map(|binding| binding.tool.clone())
            .collect::<Vec<_>>();
        tools.extend(self.session_state_tools.values().cloned());
        tools.extend(self.tool_transform_tools.values().cloned());
        tools.extend(self.openapi_tools.values().cloned());
        tools.extend(self.service_tools.values().cloned());
        tools.extend(self.cache_tools.values().cloned());
        tools.extend(self.event_tools.values().cloned());
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(tools)
    }
}

impl ServerHandler for McpStoreServer {
    async fn on_initialized(&self, context: rmcp::service::NotificationContext<RoleServer>) {
        self.store
            .event_bus
            .subscribe(
                crate::events::types::EventKind::McpToolsChanged.as_str(),
                0,
                Arc::new(AggregateToolsChangedNotification {
                    peer: context.peer,
                    instance_id: self.instance_id,
                }),
            )
            .await;
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("mcpstore", env!("CARGO_PKG_VERSION")))
        .with_instructions(self.instructions())
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        async move { Ok(ListToolsResult::with_all_items(self.current_tools().await?)) }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.bindings
            .get(name)
            .map(|binding| binding.tool.clone())
            .or_else(|| self.session_state_tools.get(name).cloned())
            .or_else(|| self.tool_transform_tools.get(name).cloned())
            .or_else(|| self.openapi_tools.get(name).cloned())
            .or_else(|| self.service_tools.get(name).cloned())
            .or_else(|| self.cache_tools.get(name).cloned())
            .or_else(|| self.event_tools.get(name).cloned())
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let resources = if let Some(session_key) = session_key.as_deref() {
                store.list_resources_in_session(session_key).await
            } else if let Some(instance_id) = instance_id {
                store.list_resources_for_instance(instance_id).await
            } else {
                store.list_resources_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let resources = project_catalog_uris(resources, "uri", false)?;
            let resources = deserialize_items::<Resource>(resources, "resource")?;
            Ok(ListResourcesResult::with_all_items(resources))
        }
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourceTemplatesResult, ErrorData>> + '_
    {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let templates = if let Some(session_key) = session_key.as_deref() {
                store.list_resource_templates_in_session(session_key).await
            } else if let Some(instance_id) = instance_id {
                store
                    .list_resource_templates_for_instance(instance_id)
                    .await
            } else {
                store.list_resource_templates_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let templates = project_catalog_uris(templates, "uriTemplate", true)?;
            let templates = deserialize_items::<ResourceTemplate>(templates, "resource template")?;
            Ok(ListResourceTemplatesResult::with_all_items(templates))
        }
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResult, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let target_instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let resources = if let Some(session_key) = session_key.as_deref() {
                store.list_resources_in_session(session_key).await
            } else if let Some(instance_id) = target_instance_id {
                store.list_resources_for_instance(instance_id).await
            } else {
                store.list_resources_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let (instance_id, original_uri) =
                resolve_projected_catalog_uri(&resources, "uri", false, &request.uri)?;
            let result = store
                .read_resource_scoped(instance_id, &original_uri)
                .await
                .map_err(map_store_error)?;
            deserialize_item::<ReadResourceResult>(result, "read resource result")
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListPromptsResult, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let prompts = if let Some(session_key) = session_key.as_deref() {
                store.list_prompts_in_session(session_key).await
            } else if let Some(instance_id) = instance_id {
                store.list_prompts_for_instance(instance_id).await
            } else {
                store.list_prompts_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let prompts = project_prompt_names(prompts)?;
            let prompts = deserialize_items::<Prompt>(prompts, "prompt")?;
            Ok(ListPromptsResult::with_all_items(prompts))
        }
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetPromptResult, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let target_instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let arguments = Value::Object(request.arguments.unwrap_or_default());
            let prompts = if let Some(session_key) = session_key.as_deref() {
                store.list_prompts_in_session(session_key).await
            } else if let Some(instance_id) = target_instance_id {
                store.list_prompts_for_instance(instance_id).await
            } else {
                store.list_prompts_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let (instance_id, original_name) = resolve_projected_prompt(&prompts, &request.name)?;
            let result = store
                .get_prompt_scoped(instance_id, &original_name, arguments)
                .await
                .map_err(map_store_error)?;
            deserialize_item::<GetPromptResult>(result, "prompt result")
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + '_ {
        let tool_name = request.name.as_ref().to_string();
        let binding = self.bindings.get(tool_name.as_str()).cloned();
        let is_session_state_tool = self.session_state_tools.contains_key(tool_name.as_str());
        let is_tool_transform_tool = self.tool_transform_tools.contains_key(tool_name.as_str());
        let is_openapi_tool = self.openapi_tools.contains_key(tool_name.as_str());
        let is_service_tool = self.service_tools.contains_key(tool_name.as_str());
        let is_cache_tool = self.cache_tools.contains_key(tool_name.as_str());
        let is_event_tool = self.event_tools.contains_key(tool_name.as_str());
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let default_session_key = self.session_key.clone();
        let meta = request.meta.clone();
        let arguments = request.arguments.unwrap_or_default();
        async move {
            if is_session_state_tool {
                return call_session_state_tool(
                    &store,
                    &tool_name,
                    meta.as_ref(),
                    arguments,
                    default_session_key.as_deref(),
                )
                .await;
            }
            if is_tool_transform_tool {
                return call_tool_transform_tool(&store, &tool_name, arguments).await;
            }
            if is_openapi_tool {
                return call_openapi_tool(&store, &tool_name, arguments).await;
            }
            if is_service_tool {
                return call_service_tool(&store, &tool_name, &scope, arguments).await;
            }
            if is_cache_tool {
                return call_cache_tool(&store, &tool_name, arguments).await;
            }
            if is_event_tool {
                return call_event_tool(&store, &tool_name, arguments).await;
            }

            let (args, session_key) = extract_business_session_key(
                meta.as_ref(),
                arguments,
                default_session_key.as_deref(),
            );
            let binding = if let Some(session_key) = session_key.as_deref() {
                build_tool_bindings(&store, &scope, instance_id, Some(session_key))
                    .await
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?
                    .remove(tool_name.as_str())
            } else {
                binding.or(build_tool_bindings(&store, &scope, instance_id, None)
                    .await
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?
                    .remove(tool_name.as_str()))
            }
            .ok_or_else(|| ErrorData::invalid_params(format!("未知工具: {tool_name}"), None))?;
            let result = store
                .call_tool(binding.instance_id, &binding.tool_name, args)
                .await
                .map_err(map_store_error)?;

            let mut content = Vec::with_capacity(result.content.len());
            for item in result.content {
                match item {
                    ContentItem::Text { text, .. } => {
                        content.push(ContentBlock::text(text));
                    }
                    ContentItem::Image {
                        data, mime_type, ..
                    } => {
                        content.push(ContentBlock::image(data, mime_type));
                    }
                    ContentItem::Audio {
                        data, mime_type, ..
                    } => {
                        content.push(ContentBlock::audio(data, mime_type));
                    }
                    ContentItem::Resource { resource, .. } => {
                        content.push(match serde_json::from_value::<ResourceContents>(resource) {
                            Ok(resource) => ContentBlock::resource(resource),
                            Err(error) => ContentBlock::text(format!(
                                "Failed to decode resource content: {error}"
                            )),
                        });
                    }
                    ContentItem::ResourceLink { resource, .. } => {
                        content.push(match serde_json::from_value::<Resource>(resource) {
                            Ok(resource) => ContentBlock::resource_link(resource),
                            Err(error) => ContentBlock::text(format!(
                                "Failed to decode resource link: {error}"
                            )),
                        });
                    }
                }
            }

            Ok(if result.is_error {
                CallToolResult::error(content)
            } else {
                CallToolResult::success(content)
            })
        }
    }
}

/// Start the MCPStore MCP server with the provided options.
pub async fn run(args: McpServerOptions) -> Result<(), BoxErr> {
    let store = Arc::new(MCPStore::setup_with_options(args.to_store_options())?);
    store.load_from_source().await?;

    let server = McpStoreServer::from_store(
        Arc::clone(&store),
        args.scope.clone(),
        args.instance_id,
        args.session_key.clone(),
        args.expose_session_state_tools,
        args.expose_tool_transform_tools,
        args.expose_openapi_tools,
        args.expose_service_tools,
        args.expose_cache_tools,
        args.expose_event_tools,
    )
    .await?;
    match args.transport {
        McpServerTransport::Stdio => {
            let running = serve_server(server, stdio()).await?;
            running.waiting().await?;
            Ok(())
        }
        McpServerTransport::StreamableHttp => run_streamable_http(server, &args).await,
    }
}

async fn connect_target_instances(
    store: &MCPStore,
    scope: &ScopeRef,
    target_instance_id: Option<InstanceId>,
) -> Result<(), BoxErr> {
    let instances = store.list_scope_instances(scope).await?;
    let instance_ids = if let Some(instance_id) = target_instance_id {
        if !instances
            .iter()
            .any(|instance| instance.instance_id == instance_id)
        {
            return Err(
                format!("Instance {instance_id} is not declared in scope {scope:?}").into(),
            );
        }
        vec![instance_id]
    } else {
        instances
            .into_iter()
            .map(|instance| instance.instance_id)
            .collect()
    };

    for instance_id in instance_ids {
        store.connect_service(instance_id).await?;
    }
    Ok(())
}

async fn build_tool_bindings(
    store: &MCPStore,
    scope: &ScopeRef,
    target_instance_id: Option<InstanceId>,
    session_key: Option<&str>,
) -> Result<HashMap<String, ToolBinding>, BoxErr> {
    let tool_payloads = if let Some(session_key) = session_key {
        serde_json::to_value(store.list_tools_in_session(session_key).await?)
            .and_then(serde_json::from_value::<Vec<Value>>)
            .map_err(|error| format!("session tool metadata serialization failed: {error}"))?
    } else if let Some(instance_id) = target_instance_id {
        store
            .list_tools_for_instance_with_filter(
                instance_id,
                crate::agent::tool_visibility::ToolVisibilityFilter::Available,
            )
            .await?
    } else {
        store.list_tools_scoped(scope).await?
    };

    let names = catalog_name_counts(&tool_payloads, "name")?;

    let mut bindings = HashMap::with_capacity(tool_payloads.len());
    for payload in tool_payloads {
        let original_name = read_required_string(&payload, "name")?;
        let canonical_tool_name = read_required_string(&payload, "tool_name")?;
        let instance_id = read_required_instance_id(&payload, "instance_id")?;
        let service_name = read_required_string(&payload, "service_name")?;
        let exposed_name = if names.get(&original_name).copied().unwrap_or_default() > 1 {
            format!(
                "{}__{}",
                stable_namespace(&service_name, instance_id),
                original_name
            )
        } else {
            original_name
        };
        let description = payload
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);
        let schema = read_required_object(&payload, "input_schema")?;

        let tool = Tool::new_with_raw(
            exposed_name.clone(),
            description.clone().map(Into::into),
            Arc::new(schema),
        );
        if bindings
            .insert(
                exposed_name.clone(),
                ToolBinding {
                    tool,
                    instance_id,
                    tool_name: canonical_tool_name,
                },
            )
            .is_some()
        {
            return Err(format!(
                "工具名称投影冲突，无法构建 Rust MCP server: scope={scope:?} tool={exposed_name}"
            )
            .into());
        }
    }

    Ok(bindings)
}

fn project_prompt_names(mut payloads: Vec<Value>) -> Result<Vec<Value>, ErrorData> {
    let names = catalog_name_counts(&payloads, "name")
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    for payload in &mut payloads {
        let original = read_required_string(payload, "name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        if names.get(&original).copied().unwrap_or_default() < 2 {
            continue;
        }
        let service_name = read_required_string(payload, "service_name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let instance_id = read_required_instance_id(payload, "instance_id")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        payload
            .as_object_mut()
            .ok_or_else(|| ErrorData::internal_error("prompt must be an object", None))?
            .insert(
                "name".to_string(),
                Value::String(format!(
                    "{}__{}",
                    stable_namespace(&service_name, instance_id),
                    original
                )),
            );
    }
    Ok(payloads)
}

fn resolve_projected_prompt(
    payloads: &[Value],
    projected_name: &str,
) -> Result<(InstanceId, String), ErrorData> {
    let names = catalog_name_counts(payloads, "name")
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    for payload in payloads {
        let original = read_required_string(payload, "name")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let instance_id = read_required_instance_id(payload, "instance_id")
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let candidate = if names.get(&original).copied().unwrap_or_default() > 1 {
            let service_name = read_required_string(payload, "service_name")
                .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
            format!(
                "{}__{}",
                stable_namespace(&service_name, instance_id),
                original
            )
        } else {
            original.clone()
        };
        if candidate == projected_name {
            return Ok((instance_id, original));
        }
    }
    Err(ErrorData::invalid_params(
        format!("Unknown aggregate prompt: {projected_name}"),
        None,
    ))
}

fn catalog_name_counts(payloads: &[Value], field: &str) -> Result<HashMap<String, usize>, BoxErr> {
    let mut names = HashMap::new();
    for payload in payloads {
        *names
            .entry(read_required_string(payload, field)?)
            .or_default() += 1;
    }
    Ok(names)
}

fn project_catalog_uris(
    mut payloads: Vec<Value>,
    field: &str,
    template: bool,
) -> Result<Vec<Value>, ErrorData> {
    for payload in &mut payloads {
        let projected = projected_catalog_uri(payload, field, template)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        let object = payload
            .as_object_mut()
            .ok_or_else(|| ErrorData::internal_error("catalog item must be an object", None))?;
        object.insert(field.to_string(), Value::String(projected));
    }
    Ok(payloads)
}

fn resolve_projected_catalog_uri(
    payloads: &[Value],
    field: &str,
    template: bool,
    projected_uri: &str,
) -> Result<(InstanceId, String), ErrorData> {
    for payload in payloads {
        let candidate = projected_catalog_uri(payload, field, template)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        if candidate == projected_uri {
            return Ok((
                read_required_instance_id(payload, "instance_id")
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
                read_required_string(payload, field)
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
            ));
        }
    }
    Err(ErrorData::invalid_params(
        format!("Unknown aggregate resource URI: {projected_uri}"),
        None,
    ))
}

fn projected_catalog_uri(payload: &Value, field: &str, template: bool) -> Result<String, BoxErr> {
    let original = read_required_string(payload, field)?;
    let service_name = read_required_string(payload, "service_name")?;
    let instance_id = read_required_instance_id(payload, "instance_id")?;
    let namespace = stable_namespace(&service_name, instance_id);
    let mut uri = reqwest::Url::parse("mcpstore://aggregate/")?;
    {
        let mut segments = uri
            .path_segments_mut()
            .map_err(|_| "aggregate URI cannot contain path segments")?;
        segments.push(&namespace);
        if template {
            segments.push("template");
        }
        segments.push(&original);
    }
    Ok(uri.into())
}

fn stable_namespace(service_name: &str, instance_id: InstanceId) -> String {
    let service_name = service_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{service_name}__{instance_id}")
}

fn build_session_state_tools() -> HashMap<String, Tool> {
    [
        session_state_tool(
            SESSION_STATE_LIST_TOOL,
            "List all JSON session_state values for a MCPStore business session.",
            session_state_schema(&[]),
            true,
        ),
        session_state_tool(
            SESSION_STATE_GET_TOOL,
            "Get one JSON session_state value for a MCPStore business session.",
            session_state_schema(&["key"]),
            true,
        ),
        session_state_tool(
            SESSION_STATE_SET_TOOL,
            "Set one JSON session_state value for a MCPStore business session.",
            session_state_schema(&["key", "value"]),
            false,
        ),
        session_state_tool(
            SESSION_STATE_DELETE_TOOL,
            "Delete one JSON session_state value for a MCPStore business session.",
            session_state_schema(&["key"]),
            false,
        ),
        session_state_tool(
            SESSION_STATE_CLEAR_TOOL,
            "Clear all JSON session_state values for a MCPStore business session.",
            session_state_schema(&[]),
            false,
        ),
        session_state_tool(
            SESSION_SNAPSHOT_EXPORT_TOOL,
            "Export all MCPStore business sessions and session state as a portable snapshot.",
            session_snapshot_schema(&[]),
            true,
        ),
        session_state_tool(
            SESSION_SNAPSHOT_IMPORT_TOOL,
            "Import a MCPStore business session snapshot without overwriting changed local state.",
            session_snapshot_schema(&["snapshot"]),
            false,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

fn session_state_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(!read_only)
        .idempotent(matches!(
            name,
            SESSION_STATE_LIST_TOOL
                | SESSION_STATE_GET_TOOL
                | SESSION_STATE_CLEAR_TOOL
                | SESSION_SNAPSHOT_EXPORT_TOOL
                | SESSION_SNAPSHOT_IMPORT_TOOL
        ))
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

fn session_snapshot_schema(required: &[&str]) -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "snapshot".to_string(),
        serde_json::json!({
            "type": "object",
            "description": "Snapshot returned by mcpstore_session_snapshot_export."
        }),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

fn session_state_schema(required: &[&str]) -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "session_key".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "MCPStore business session key. Optional when the server was started with a default session key."
        }),
    );
    properties.insert(
        "_mcpstore_session_key".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Alias for session_key used by MCPStore business session routing."
        }),
    );
    properties.insert(
        "key".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Session state key. Must be non-empty and must not contain ':'."
        }),
    );
    properties.insert(
        "value".to_string(),
        serde_json::json!({
            "description": "JSON-serializable session state value."
        }),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

fn build_tool_transform_tools() -> HashMap<String, Tool> {
    [
        tool_transform_tool(
            TOOL_TRANSFORM_LIST_TOOL,
            "List all Rust-backed MCPStore tool transform rules.",
            tool_transform_schema(&[]),
            true,
        ),
        tool_transform_tool(
            TOOL_TRANSFORM_GET_TOOL,
            "Get one Rust-backed MCPStore tool transform rule.",
            tool_transform_schema(&["instance_id", "tool_name"]),
            true,
        ),
        tool_transform_tool(
            TOOL_TRANSFORM_SET_TOOL,
            "Set one Rust-backed MCPStore tool transform rule.",
            tool_transform_schema(&["instance_id", "tool_name"]),
            false,
        ),
        tool_transform_tool(
            TOOL_TRANSFORM_DELETE_TOOL,
            "Delete one Rust-backed MCPStore tool transform rule.",
            tool_transform_schema(&["instance_id", "tool_name"]),
            false,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

fn tool_transform_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(!read_only)
        .idempotent(matches!(
            name,
            TOOL_TRANSFORM_LIST_TOOL | TOOL_TRANSFORM_GET_TOOL | TOOL_TRANSFORM_DELETE_TOOL
        ))
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

fn tool_transform_schema(required: &[&str]) -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "instance_id".to_string(),
        serde_json::json!({
            "type": "string",
            "format": "uuid",
            "description": "Exact deterministic MCPStore service instance id."
        }),
    );
    properties.insert(
        "tool_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Original tool name or current transformed display name."
        }),
    );
    properties.insert(
        "display_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Optional display name exposed on scoped agent/tool surfaces."
        }),
    );
    properties.insert(
        "description".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Optional description override exposed on scoped agent/tool surfaces."
        }),
    );
    properties.insert(
        "arguments".to_string(),
        serde_json::json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "original_name": {"type": "string"},
                    "new_name": {"type": "string"},
                    "hidden": {"type": "boolean"},
                    "default_value": {},
                    "description": {"type": "string"}
                },
                "required": ["original_name", "hidden"],
                "additionalProperties": false
            }
        }),
    );
    properties.insert(
        "tags".to_string(),
        serde_json::json!({"type": "array", "items": {"type": "string"}}),
    );
    properties.insert(
        "enabled".to_string(),
        serde_json::json!({"type": "boolean"}),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

fn build_openapi_tools() -> HashMap<String, Tool> {
    [
        openapi_tool(
            OPENAPI_IMPORT_LIST_TOOL,
            "List all Rust-backed MCPStore OpenAPI import analysis records.",
            openapi_schema(&[], false, false, false),
            true,
        ),
        openapi_tool(
            OPENAPI_IMPORT_GET_TOOL,
            "Get one Rust-backed MCPStore OpenAPI import analysis record.",
            openapi_schema(&["name"], true, false, false),
            true,
        ),
        openapi_tool(
            OPENAPI_IMPORT_SET_TOOL,
            "Import an OpenAPI spec into MCPStore shared state and register an executable HTTP virtual service.",
            openapi_schema(&["name", "spec_url"], true, true, true),
            false,
        ),
        openapi_tool(
            OPENAPI_BUNDLE_TOOL,
            "Bundle an OpenAPI spec with external references resolved without importing or registering a virtual service.",
            openapi_schema(&["spec_url"], false, true, false),
            true,
        ),
        openapi_tool(
            OPENAPI_BUNDLE_ARTIFACT_TOOL,
            "Bundle an OpenAPI spec and return dependency metadata without importing or registering a virtual service.",
            openapi_schema(&["spec_url"], false, true, false),
            true,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

fn openapi_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(false)
        .idempotent(matches!(
            name,
            OPENAPI_IMPORT_LIST_TOOL
                | OPENAPI_IMPORT_GET_TOOL
                | OPENAPI_BUNDLE_TOOL
                | OPENAPI_BUNDLE_ARTIFACT_TOOL
        ))
        .open_world(matches!(
            name,
            OPENAPI_IMPORT_SET_TOOL | OPENAPI_BUNDLE_TOOL | OPENAPI_BUNDLE_ARTIFACT_TOOL
        ));
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

fn openapi_schema(
    required: &[&str],
    include_name: bool,
    include_spec_input: bool,
    include_import_options: bool,
) -> Map<String, Value> {
    let mut properties = Map::new();
    if include_name {
        properties.insert(
            "name".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "MCPStore OpenAPI import/service name."
            }),
        );
    }
    if include_spec_input {
        properties.insert(
            "spec_url".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "OpenAPI spec URL. When spec or spec_text is also provided, this is used as source metadata and as the base URI for relative external references."
            }),
        );
        properties.insert(
            "spec".to_string(),
            serde_json::json!({
                "type": "object",
                "description": "Optional OpenAPI document. If omitted, MCPStore fetches spec_url."
            }),
        );
        properties.insert(
            "spec_text".to_string(),
            serde_json::json!({
                "type": "string",
                "description": "Optional OpenAPI JSON or YAML document text. Mutually exclusive with spec."
            }),
        );
    }
    if include_import_options {
        properties.insert(
            "headers".to_string(),
            serde_json::json!({
                "type": "object",
                "description": "Optional static HTTP headers sent by the OpenAPI virtual service.",
                "additionalProperties": { "type": "string" }
            }),
        );
        properties.insert(
            "auth".to_string(),
            serde_json::json!({
                "type": "object",
                "description": "Optional credentials keyed by OpenAPI security scheme name. Values may be strings, token/value objects, or username/password objects for basic auth."
            }),
        );
    }
    if include_spec_input {
        properties.insert(
            "ref_cache".to_string(),
            serde_json::json!({
                "type": "object",
                "description": "Optional external $ref shared document cache policy.",
                "properties": {
                    "enabled": { "type": "boolean" },
                    "ttl_seconds": { "type": "integer", "minimum": 0 }
                },
                "additionalProperties": false
            }),
        );
        properties.insert(
            "fetch_timeout_millis".to_string(),
            serde_json::json!({
                "type": "integer",
                "minimum": 1,
                "description": "Optional timeout in milliseconds for fetching OpenAPI specs and external references."
            }),
        );
        properties.insert(
            "timeout_millis".to_string(),
            serde_json::json!({
                "type": "integer",
                "minimum": 1,
                "description": "Optional timeout in milliseconds for OpenAPI import runtime operations. Bundle tools use this as a fallback when fetch_timeout_millis is omitted."
            }),
        );
    }

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

fn build_cache_tools() -> HashMap<String, Tool> {
    [
        cache_tool(
            CACHE_HEALTH_TOOL,
            "Read MCPStore cache backend health and collection coverage from Rust core.",
            empty_object_schema(),
            true,
        ),
        cache_tool(
            CACHE_INSPECT_TOOL,
            "Inspect MCPStore cache backend state, counts, collections, and request metrics from Rust core.",
            empty_object_schema(),
            true,
        ),
        cache_tool(
            CACHE_SWITCH_TOOL,
            "Switch the MCPStore cache backend and migrate existing Rust core state into the target backend.",
            cache_switch_schema(),
            false,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

fn build_service_tools() -> HashMap<String, Tool> {
    [
        service_tool(
            SERVICE_LIST_TOOL,
            "List MCPStore service instances in the server scope from Rust core.",
            empty_object_schema(),
            true,
        ),
        service_tool(
            SERVICE_INFO_TOOL,
            "Read one MCPStore service instance selected by service_name and scope from Rust core.",
            service_scope_schema(),
            true,
        ),
        service_tool(
            SERVICE_STATE_TOOL,
            "Read one MCPStore service instance status from Rust core.",
            instance_id_schema(),
            true,
        ),
        service_tool(
            SERVICE_CHECK_TOOL,
            "Run a health check for one MCPStore service instance from Rust core.",
            instance_id_schema(),
            true,
        ),
        service_tool(
            SERVICE_ADD_TOOL,
            "Add one MCPStore service definition with an explicit initial scope through Rust core.",
            service_add_schema(),
            false,
        ),
        service_tool(
            SERVICE_PATCH_TOOL,
            "Set one MCPStore service scope descriptor through Rust core.",
            service_scope_descriptor_schema(),
            false,
        ),
        service_tool(
            SERVICE_REMOVE_TOOL,
            "Remove one explicit MCPStore service scope through Rust core.",
            service_scope_schema(),
            false,
        ),
        service_tool(
            SERVICE_CONNECT_TOOL,
            "Connect one MCPStore service instance through Rust core.",
            instance_id_schema(),
            false,
        ),
        service_tool(
            SERVICE_DISCONNECT_TOOL,
            "Disconnect one MCPStore service instance through Rust core.",
            instance_id_schema(),
            false,
        ),
        service_tool(
            SERVICE_RESTART_TOOL,
            "Restart one MCPStore service instance through Rust core.",
            instance_id_schema(),
            false,
        ),
        service_tool(
            SERVICE_WAIT_TOOL,
            "Wait for one MCPStore service instance to become ready through Rust core.",
            service_wait_schema(),
            false,
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

fn build_event_tools() -> HashMap<String, Tool> {
    [
        event_tool(
            EVENT_HISTORY_TOOL,
            "Read recent MCPStore event history from Rust core.",
            event_history_schema(),
        ),
        event_tool(
            EVENT_CAPABILITY_REPORT_TOOL,
            "Read MCPStore event capability report from Rust core.",
            empty_object_schema(),
        ),
    ]
    .into_iter()
    .map(|tool| (tool.name.as_ref().to_string(), tool))
    .collect()
}

fn service_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(matches!(name, SERVICE_REMOVE_TOOL))
        .idempotent(read_only)
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

fn event_tool(name: &'static str, description: &'static str, schema: Map<String, Value>) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(true)
        .destructive(false)
        .idempotent(true)
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

fn cache_tool(
    name: &'static str,
    description: &'static str,
    schema: Map<String, Value>,
    read_only: bool,
) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(false)
        .idempotent(read_only)
        .open_world(false);
    Tool::new(name, description, Arc::new(schema)).with_annotations(annotations)
}

fn empty_object_schema() -> Map<String, Value> {
    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(Map::new()));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    schema
}

fn object_schema(properties: Map<String, Value>, required: &[&str]) -> Map<String, Value> {
    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|field| Value::String((*field).to_string()))
                    .collect(),
            ),
        );
    }
    schema
}

fn event_history_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "count".to_string(),
        serde_json::json!({
            "type": "integer",
            "minimum": 1,
            "description": "Maximum number of recent events to return. Defaults to 100."
        }),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    schema
}

fn scope_ref_schema() -> Value {
    serde_json::json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "type": {"const": "store"}
                },
                "required": ["type"],
                "additionalProperties": false
            },
            {
                "type": "object",
                "properties": {
                    "type": {"const": "agent"},
                    "agent_id": {"type": "string", "minLength": 1}
                },
                "required": ["type", "agent_id"],
                "additionalProperties": false
            }
        ]
    })
}

fn service_scope_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "service_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Exact MCPStore service definition name."
        }),
    );
    properties.insert("scope".to_string(), scope_ref_schema());
    object_schema(properties, &["service_name", "scope"])
}

fn instance_id_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "instance_id".to_string(),
        serde_json::json!({
            "type": "string",
            "format": "uuid",
            "description": "Exact deterministic MCPStore service instance id."
        }),
    );
    object_schema(properties, &["instance_id"])
}

fn service_add_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "service_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Exact MCPStore service definition name."
        }),
    );
    properties.insert("scope".to_string(), scope_ref_schema());
    properties.insert(
        "config".to_string(),
        serde_json::json!({
            "type": "object",
            "description": "MCP service base config used to create the new definition."
        }),
    );
    object_schema(properties, &["service_name", "scope", "config"])
}

fn service_scope_descriptor_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "service_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Exact MCPStore service definition name."
        }),
    );
    properties.insert("scope".to_string(), scope_ref_schema());
    properties.insert(
        "descriptor".to_string(),
        serde_json::json!({
            "type": "object",
            "description": "Complete scope descriptor. Its config object is a partial override; null deletes inherited fields."
        }),
    );
    object_schema(properties, &["service_name", "scope", "descriptor"])
}

fn service_wait_schema() -> Map<String, Value> {
    let mut properties = instance_id_schema()
        .remove("properties")
        .and_then(|value| value.as_object().cloned())
        .expect("instance id schema must contain properties");
    properties.insert(
        "timeout".to_string(),
        serde_json::json!({
            "type": "integer",
            "minimum": 1,
            "description": "Maximum wait time in seconds. Defaults to 10."
        }),
    );
    object_schema(properties, &["instance_id"])
}

fn cache_switch_schema() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "backend".to_string(),
        serde_json::json!({
            "type": "string",
            "enum": ["memory", "redis", "openkeyv_memory", "openkeyv_redis"],
            "description": "Target MCPStore cache backend. Redis backends require redis_url unless the store already has one."
        }),
    );
    properties.insert(
        "redis_url".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Optional Redis URL for redis/openkeyv_redis backends."
        }),
    );
    properties.insert(
        "namespace".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Optional target namespace. Use the same namespace to share state across processes."
        }),
    );

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    schema.insert(
        "required".to_string(),
        Value::Array(vec![Value::String("backend".to_string())]),
    );
    schema
}

async fn call_event_tool(
    store: &MCPStore,
    tool_name: &str,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        EVENT_HISTORY_TOOL => {
            let count = optional_positive_usize_argument(&arguments, "count")?.unwrap_or(100);
            let events = store.event_history(count).await;
            serde_json::json!({"events": events, "total": events.len()})
        }
        EVENT_CAPABILITY_REPORT_TOOL => {
            let report = store.event_capability_report().await;
            serde_json::json!({"report": report})
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore event 观测工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

async fn call_service_tool(
    store: &MCPStore,
    tool_name: &str,
    server_scope: &ScopeRef,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        SERVICE_LIST_TOOL => {
            let services = store
                .list_services_scoped(server_scope)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"services": services, "total": services.len()})
        }
        SERVICE_INFO_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?;
            let scope = required_scope_argument(&arguments)?;
            let instance_id = store
                .instance_id_for_scope(service_name, &scope)
                .await
                .map_err(map_store_error)?;
            let service = store
                .service_info_scoped(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"service": service})
        }
        SERVICE_STATE_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let status = store
                .service_state(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": status})
        }
        SERVICE_CHECK_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let check = store
                .health_check(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"check": check})
        }
        SERVICE_ADD_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?.to_string();
            let scope = required_scope_argument(&arguments)?;
            let mut config = service_config_from_arguments(&arguments)?;
            if config.mcpstore.is_some() {
                return Err(ErrorData::invalid_params(
                    "config must contain only MCP service fields; scope is provided separately",
                    None,
                ));
            }
            let mut scopes = ScopeDeclarations::default();
            match &scope {
                ScopeRef::Store => scopes.store = Some(ScopeDescriptor::default()),
                ScopeRef::Agent { agent_id } => {
                    scopes
                        .agents
                        .insert(agent_id.clone(), ScopeDescriptor::default());
                }
            };
            config.mcpstore = Some(McpStoreExtension {
                scopes,
                revision: 1,
                ..McpStoreExtension::default()
            });
            store
                .add_service(&service_name, config)
                .await
                .map_err(map_store_error)?;
            let instance_id = store
                .instance_id_for_scope(&service_name, &scope)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({
                "status": "ok",
                "service_name": service_name,
                "scope": scope,
                "instance_id": instance_id
            })
        }
        SERVICE_PATCH_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?;
            let scope = required_scope_argument(&arguments)?;
            let descriptor = service_scope_descriptor_from_arguments(&arguments)?;
            let instance_id = store
                .declare_service_scope(service_name, &scope, descriptor)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({
                "status": "ok",
                "service_name": service_name,
                "scope": scope,
                "instance_id": instance_id
            })
        }
        SERVICE_REMOVE_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?;
            let scope = required_scope_argument(&arguments)?;
            let instance_id = store
                .instance_id_for_scope(service_name, &scope)
                .await
                .map_err(map_store_error)?;
            store
                .remove_service_scope(service_name, &scope)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({
                "status": "ok",
                "service_name": service_name,
                "scope": scope,
                "instance_id": instance_id
            })
        }
        SERVICE_CONNECT_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            store
                .connect_service(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": "ok", "instance_id": instance_id})
        }
        SERVICE_DISCONNECT_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            store
                .disconnect_service(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": "ok", "instance_id": instance_id})
        }
        SERVICE_RESTART_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            store
                .restart_service(instance_id)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": "ok", "instance_id": instance_id})
        }
        SERVICE_WAIT_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let timeout =
                optional_positive_u64_argument_with_label(&arguments, "timeout", "service wait")?
                    .unwrap_or(10);
            let status = store
                .wait_instance_ready(instance_id, timeout)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({
                "status": status,
                "instance_id": instance_id,
                "timeout": timeout
            })
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore service 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

async fn call_session_state_tool(
    store: &MCPStore,
    tool_name: &str,
    meta: Option<&rmcp::model::Meta>,
    arguments: Map<String, Value>,
    default_session_key: Option<&str>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        SESSION_SNAPSHOT_EXPORT_TOOL => {
            let snapshot = store
                .export_sessions_snapshot()
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"snapshot": snapshot})
        }
        SESSION_SNAPSHOT_IMPORT_TOOL => {
            let snapshot = arguments
                .get("snapshot")
                .cloned()
                .ok_or_else(|| ErrorData::invalid_params("缺少参数: snapshot", None))?;
            let report = store
                .import_sessions_snapshot(snapshot)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"report": report})
        }
        SESSION_STATE_LIST_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let state = store
                .list_session_state(&session_key)
                .await
                .map_err(map_store_error)?;
            let values = state.values.clone();
            serde_json::json!({"state": state, "values": values})
        }
        SESSION_STATE_GET_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let key = required_argument_string(&arguments, "key")?;
            let value = store
                .get_session_state_value(&session_key, key)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"key": key, "value": value})
        }
        SESSION_STATE_SET_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let key = required_argument_string(&arguments, "key")?;
            let value = arguments
                .get("value")
                .cloned()
                .ok_or_else(|| ErrorData::invalid_params("缺少参数: value", None))?;
            let state = store
                .set_session_state(&session_key, key, value)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"state": state})
        }
        SESSION_STATE_DELETE_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let key = required_argument_string(&arguments, "key")?;
            let state = store
                .delete_session_state(&session_key, key)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"state": state})
        }
        SESSION_STATE_CLEAR_TOOL => {
            let session_key = resolve_session_state_key(meta, &arguments, default_session_key)?;
            let state = store
                .clear_session_state(&session_key)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"state": state})
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore session_state 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

async fn call_tool_transform_tool(
    store: &MCPStore,
    tool_name: &str,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        TOOL_TRANSFORM_LIST_TOOL => {
            let transforms = store
                .list_tool_transforms()
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"transforms": transforms, "total": transforms.len()})
        }
        TOOL_TRANSFORM_GET_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let tool_name = required_argument_string(&arguments, "tool_name")?;
            let transform = store
                .get_tool_transform(instance_id, tool_name)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"transform": transform})
        }
        TOOL_TRANSFORM_SET_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let tool_name = required_argument_string(&arguments, "tool_name")?.to_string();
            let patch = serde_json::from_value::<ToolTransformPatch>(Value::Object(arguments))
                .map_err(|error| {
                    ErrorData::invalid_params(
                        format!("工具转换规则参数反序列化失败: {error}"),
                        None,
                    )
                })?;
            let transform = store
                .set_tool_transform(instance_id, &tool_name, patch)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"transform": transform})
        }
        TOOL_TRANSFORM_DELETE_TOOL => {
            let instance_id = required_instance_id_argument(&arguments)?;
            let tool_name = required_argument_string(&arguments, "tool_name")?;
            store
                .delete_tool_transform(instance_id, tool_name)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"status": "ok"})
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore tool transform 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

async fn call_openapi_tool(
    store: &MCPStore,
    tool_name: &str,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        OPENAPI_IMPORT_LIST_TOOL => {
            let imports = store
                .list_openapi_imports()
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"imports": imports, "total": imports.len()})
        }
        OPENAPI_IMPORT_GET_TOOL => {
            let name = required_argument_string(&arguments, "name")?;
            let import = store
                .get_openapi_import(name)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"import": import})
        }
        OPENAPI_IMPORT_SET_TOOL => {
            let name = required_argument_string(&arguments, "name")?.to_string();
            let spec_url = required_argument_string(&arguments, "spec_url")?.to_string();
            let options = openapi_import_options_from_arguments(&arguments)?;
            let spec = arguments.get("spec").cloned();
            let spec_text = arguments
                .get("spec_text")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty());
            let import = match (spec, spec_text) {
                (Some(_), Some(_)) => {
                    return Err(ErrorData::invalid_params(
                        "spec and spec_text cannot both be provided",
                        None,
                    ));
                }
                (Some(spec), None) => {
                    store
                        .import_openapi_service_from_spec_with_options(
                            &name, &spec_url, spec, options,
                        )
                        .await
                }
                (None, Some(spec_text)) => {
                    store
                        .import_openapi_service_from_spec_text_with_options(
                            &name, &spec_url, spec_text, options,
                        )
                        .await
                }
                (None, None) => {
                    store
                        .import_openapi_service_with_options(&name, &spec_url, options)
                        .await
                }
            }
            .map_err(map_store_error)?;
            serde_json::json!({"import": import})
        }
        OPENAPI_BUNDLE_TOOL => {
            let spec_url = required_argument_string(&arguments, "spec_url")?.to_string();
            let options = openapi_bundle_options_from_arguments(&arguments)?;
            let spec = arguments.get("spec").cloned();
            let spec_text = arguments
                .get("spec_text")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty());
            let bundle = match (spec, spec_text) {
                (Some(_), Some(_)) => {
                    return Err(ErrorData::invalid_params(
                        "spec and spec_text cannot both be provided",
                        None,
                    ));
                }
                (Some(spec), None) => {
                    store
                        .bundle_openapi_spec_from_value_with_options(&spec_url, spec, options)
                        .await
                }
                (None, Some(spec_text)) => {
                    store
                        .bundle_openapi_spec_from_text_with_options(&spec_url, spec_text, options)
                        .await
                }
                (None, None) => {
                    store
                        .bundle_openapi_spec_with_options(&spec_url, options)
                        .await
                }
            }
            .map_err(map_store_error)?;
            serde_json::json!({"bundle": bundle})
        }
        OPENAPI_BUNDLE_ARTIFACT_TOOL => {
            let spec_url = required_argument_string(&arguments, "spec_url")?.to_string();
            let options = openapi_bundle_options_from_arguments(&arguments)?;
            let spec = arguments.get("spec").cloned();
            let spec_text = arguments
                .get("spec_text")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty());
            let artifact = match (spec, spec_text) {
                (Some(_), Some(_)) => {
                    return Err(ErrorData::invalid_params(
                        "spec and spec_text cannot both be provided",
                        None,
                    ));
                }
                (Some(spec), None) => {
                    store
                        .bundle_openapi_artifact_from_value_with_options(&spec_url, spec, options)
                        .await
                }
                (None, Some(spec_text)) => {
                    store
                        .bundle_openapi_artifact_from_text_with_options(
                            &spec_url, spec_text, options,
                        )
                        .await
                }
                (None, None) => {
                    store
                        .bundle_openapi_artifact_with_options(&spec_url, options)
                        .await
                }
            }
            .map_err(map_store_error)?;
            serde_json::json!({"artifact": artifact})
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore OpenAPI 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

async fn call_cache_tool(
    store: &MCPStore,
    tool_name: &str,
    arguments: Map<String, Value>,
) -> Result<CallToolResult, ErrorData> {
    let result = match tool_name {
        CACHE_HEALTH_TOOL => {
            let health = store.cache_health_check().await.map_err(map_store_error)?;
            serde_json::json!({"health": health})
        }
        CACHE_INSPECT_TOOL => {
            let inspect = store.cache_inspect().await.map_err(map_store_error)?;
            serde_json::json!({"inspect": inspect})
        }
        CACHE_SWITCH_TOOL => {
            let backend = required_argument_string(&arguments, "backend")?;
            let storage = parse_cache_storage_argument(backend)?;
            let backend_label = storage.as_str();
            let redis_url = optional_string_argument(&arguments, "redis_url");
            let namespace = optional_string_argument(&arguments, "namespace");
            let snapshot = store
                .switch_cache_storage(storage, redis_url, namespace)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({
                "backend": backend_label,
                "namespace": store.namespace(),
                "snapshot": snapshot,
            })
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("未知 MCPStore cache 管理工具: {tool_name}"),
                None,
            ));
        }
    };
    Ok(CallToolResult::structured(result))
}

fn resolve_session_state_key(
    meta: Option<&rmcp::model::Meta>,
    arguments: &Map<String, Value>,
    default_session_key: Option<&str>,
) -> Result<String, ErrorData> {
    meta.and_then(|meta| meta.0.get("_mcpstore_session_key"))
        .and_then(Value::as_str)
        .or_else(|| {
            arguments
                .get("_mcpstore_session_key")
                .or_else(|| arguments.get("session_key"))
                .and_then(Value::as_str)
        })
        .or(default_session_key)
        .filter(|session_key| !session_key.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ErrorData::invalid_params("缺少参数: session_key", None))
}

fn required_argument_string<'a>(
    arguments: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, ErrorData> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ErrorData::invalid_params(format!("缺少参数: {field}"), None))
}

fn required_scope_argument(arguments: &Map<String, Value>) -> Result<ScopeRef, ErrorData> {
    let value = arguments
        .get("scope")
        .cloned()
        .ok_or_else(|| ErrorData::invalid_params("缺少参数: scope", None))?;
    serde_json::from_value(value)
        .map_err(|error| ErrorData::invalid_params(format!("scope 参数无效: {error}"), None))
}

fn required_instance_id_argument(arguments: &Map<String, Value>) -> Result<InstanceId, ErrorData> {
    let value = required_argument_string(arguments, "instance_id")?;
    InstanceId::from_str(value)
        .map_err(|error| ErrorData::invalid_params(format!("instance_id 参数无效: {error}"), None))
}

fn optional_string_argument(arguments: &Map<String, Value>, field: &str) -> Option<String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_positive_usize_argument(
    arguments: &Map<String, Value>,
    field: &str,
) -> Result<Option<usize>, ErrorData> {
    let Some(value) = arguments.get(field) else {
        return Ok(None);
    };
    let parsed = match value {
        Value::Null => return Ok(None),
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse::<u64>().ok(),
        _ => None,
    }
    .filter(|value| *value > 0)
    .and_then(|value| usize::try_from(value).ok())
    .ok_or_else(|| {
        ErrorData::invalid_params(format!("{field} must be a positive integer"), None)
    })?;
    Ok(Some(parsed))
}

fn service_config_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<ServerConfig, ErrorData> {
    let config = arguments
        .get("config")
        .cloned()
        .ok_or_else(|| ErrorData::invalid_params("缺少参数: config", None))?;
    serde_json::from_value::<ServerConfig>(config)
        .map_err(|error| ErrorData::invalid_params(format!("服务配置解析失败: {error}"), None))
}

fn service_scope_descriptor_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<ScopeDescriptor, ErrorData> {
    let descriptor = arguments
        .get("descriptor")
        .cloned()
        .ok_or_else(|| ErrorData::invalid_params("缺少参数: descriptor", None))?;
    serde_json::from_value(descriptor).map_err(|error| {
        ErrorData::invalid_params(format!("服务作用域描述解析失败: {error}"), None)
    })
}

fn parse_cache_storage_argument(value: &str) -> Result<CacheStorage, ErrorData> {
    match value {
        "memory" => Ok(CacheStorage::Memory),
        "redis" => Ok(CacheStorage::Redis),
        "openkeyv_memory" => Ok(CacheStorage::OpenKeyvMemory),
        "openkeyv_redis" => Ok(CacheStorage::OpenKeyvRedis),
        other => Err(ErrorData::invalid_params(
            format!("不支持的 cache backend: {other}"),
            None,
        )),
    }
}

fn openapi_import_options_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<OpenApiImportOptions, ErrorData> {
    let headers = match arguments.get("headers") {
        Some(value) => serde_json::from_value(value.clone()).map_err(|err| {
            ErrorData::invalid_params(format!("OpenAPI headers must be a string map: {err}"), None)
        })?,
        None => HashMap::new(),
    };
    let auth = arguments
        .get("auth")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Ok(OpenApiImportOptions {
        headers,
        auth,
        ref_cache: openapi_ref_cache_policy_from_arguments(arguments)?,
        timeout_millis: optional_positive_u64_argument(arguments, "timeout_millis")?
            .unwrap_or_else(OpenApiImportOptions::default_timeout_millis),
        fetch_timeout_millis: optional_positive_u64_argument(arguments, "fetch_timeout_millis")?
            .unwrap_or_else(OpenApiImportOptions::default_fetch_timeout_millis),
    })
}

fn openapi_bundle_options_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<OpenApiBundleOptions, ErrorData> {
    Ok(OpenApiBundleOptions {
        ref_cache: openapi_ref_cache_policy_from_arguments(arguments)?,
        timeout_millis: optional_positive_u64_argument(arguments, "fetch_timeout_millis")?
            .or(optional_positive_u64_argument(arguments, "timeout_millis")?)
            .unwrap_or_else(OpenApiBundleOptions::default_timeout_millis),
    })
}

fn optional_positive_u64_argument(
    arguments: &Map<String, Value>,
    field: &str,
) -> Result<Option<u64>, ErrorData> {
    optional_positive_u64_argument_with_label(arguments, field, "OpenAPI")
}

fn optional_positive_u64_argument_with_label(
    arguments: &Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<Option<u64>, ErrorData> {
    let Some(value) = arguments.get(field) else {
        return Ok(None);
    };
    let parsed = match value {
        Value::Null => return Ok(None),
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse::<u64>().ok(),
        _ => None,
    }
    .filter(|value| *value > 0)
    .ok_or_else(|| {
        ErrorData::invalid_params(format!("{label} {field} must be a positive integer"), None)
    })?;
    Ok(Some(parsed))
}

fn openapi_ref_cache_policy_from_arguments(
    arguments: &Map<String, Value>,
) -> Result<OpenApiRefCachePolicy, ErrorData> {
    match arguments.get("ref_cache") {
        Some(value) => serde_json::from_value(value.clone()).map_err(|err| {
            ErrorData::invalid_params(format!("OpenAPI ref_cache is invalid: {err}"), None)
        }),
        None => Ok(OpenApiRefCachePolicy::default()),
    }
}

fn extract_business_session_key(
    meta: Option<&rmcp::model::Meta>,
    mut arguments: Map<String, Value>,
    default_session_key: Option<&str>,
) -> (Value, Option<String>) {
    let meta_session_key = meta
        .and_then(|meta| meta.0.get("_mcpstore_session_key"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let argument_session_key = arguments
        .remove("_mcpstore_session_key")
        .and_then(|value| value.as_str().map(str::to_string));
    let session_key = meta_session_key
        .or(argument_session_key)
        .or_else(|| default_session_key.map(str::to_string));
    (Value::Object(arguments), session_key)
}

fn read_required_string(payload: &Value, field: &str) -> Result<String, BoxErr> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("工具元数据缺少字符串字段: {field}").into())
}

fn read_required_instance_id(payload: &Value, field: &str) -> Result<InstanceId, BoxErr> {
    let value = payload
        .get(field)
        .ok_or_else(|| format!("工具元数据缺少字段: {field}"))?;
    serde_json::from_value(value.clone())
        .map_err(|error| format!("工具元数据字段 {field} 不是有效 instance_id: {error}").into())
}

fn read_required_object(payload: &Value, field: &str) -> Result<Map<String, Value>, BoxErr> {
    payload
        .get(field)
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| format!("工具元数据缺少对象字段: {field}").into())
}

fn map_store_error(error: StoreError) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}

fn deserialize_items<T>(items: Vec<Value>, label: &str) -> Result<Vec<T>, ErrorData>
where
    T: DeserializeOwned,
{
    items
        .into_iter()
        .map(|item| deserialize_item(item, label))
        .collect()
}

fn deserialize_item<T>(item: Value, label: &str) -> Result<T, ErrorData>
where
    T: DeserializeOwned,
{
    serde_json::from_value(item)
        .map_err(|error| ErrorData::internal_error(format!("{label} 反序列化失败: {error}"), None))
}

async fn run_streamable_http(
    server: McpStoreServer,
    args: &McpServerOptions,
) -> Result<(), BoxErr> {
    let path = normalize_http_path(&args.path);
    let service: StreamableHttpService<McpStoreServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(server.clone()),
            Default::default(),
            StreamableHttpServerConfig::default(),
        );
    let router = axum::Router::new().nest_service(&path, service);
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!("[MCP] Starting streamable-http at http://{addr}{path}");
    axum::serve(listener, router).await?;
    Ok(())
}

fn normalize_http_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/mcp".to_string();
    }
    let normalized = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    normalized.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{events::types::EventKind, CacheStorage, ServiceInstanceKey};
    use rmcp::{
        service::{NotificationContext, RoleClient},
        ClientHandler, ServiceExt,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Default)]
    struct NotificationClient {
        tool_list_changes: Arc<AtomicUsize>,
    }

    impl ClientHandler for NotificationClient {
        async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
            self.tool_list_changes.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn aggregate_server_forwards_scoped_tool_list_changes() {
        let store = MCPStore::setup_with_options(StoreOptions {
            backend: Some(CacheStorage::Memory),
            ..StoreOptions::default()
        })
        .unwrap();
        let instance_id = ServiceInstanceKey::new("aggregate", ScopeRef::Store).instance_id();
        let other_instance_id = ServiceInstanceKey::new("other", ScopeRef::Store).instance_id();
        let server = McpStoreServer {
            store: Arc::clone(&store),
            scope: ScopeRef::Store,
            instance_id: Some(instance_id),
            session_key: None,
            scope_label: "store".to_string(),
            bindings: Arc::new(HashMap::new()),
            session_state_tools: Arc::new(HashMap::new()),
            tool_transform_tools: Arc::new(HashMap::new()),
            openapi_tools: Arc::new(HashMap::new()),
            service_tools: Arc::new(HashMap::new()),
            cache_tools: Arc::new(HashMap::new()),
            event_tools: Arc::new(HashMap::new()),
            tools: Arc::new(Vec::new()),
        };
        let client_handler = NotificationClient::default();
        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server_start =
            tokio::spawn(async move { server.serve(server_transport).await.unwrap() });
        let client = client_handler
            .clone()
            .serve(client_transport)
            .await
            .unwrap();
        let server = server_start.await.unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                store
                    .event_bus
                    .publish(
                        Event::new(
                            EventKind::McpToolsChanged.as_str(),
                            serde_json::json!({"instanceId": instance_id}),
                        ),
                        true,
                    )
                    .await;
                if client_handler.tool_list_changes.load(Ordering::SeqCst) == 1 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        store
            .event_bus
            .publish(
                Event::new(
                    EventKind::McpToolsChanged.as_str(),
                    serde_json::json!({"instanceId": other_instance_id}),
                ),
                true,
            )
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert_eq!(client_handler.tool_list_changes.load(Ordering::SeqCst), 1);

        client.cancel().await.unwrap();
        server.cancel().await.unwrap();
        store
            .event_bus
            .publish(
                Event::new(
                    EventKind::McpToolsChanged.as_str(),
                    serde_json::json!({"instanceId": instance_id}),
                ),
                true,
            )
            .await;
    }

    #[test]
    fn duplicate_prompt_names_are_stably_projected_and_routed() {
        let first = ServiceInstanceKey::new("first service", ScopeRef::Store).instance_id();
        let second = ServiceInstanceKey::new("second", ScopeRef::Store).instance_id();
        let prompts = vec![
            serde_json::json!({
                "name": "review",
                "service_name": "first service",
                "instance_id": first,
            }),
            serde_json::json!({
                "name": "review",
                "service_name": "second",
                "instance_id": second,
            }),
        ];

        let projected = project_prompt_names(prompts.clone()).unwrap();
        let first_name = projected[0]["name"].as_str().unwrap();
        assert_eq!(first_name, format!("first_service__{first}__review"));
        assert_eq!(
            resolve_projected_prompt(&prompts, first_name).unwrap(),
            (first, "review".to_string())
        );
    }

    #[test]
    fn resource_and_template_uris_are_stably_projected_and_routed() {
        let instance_id = ServiceInstanceKey::new("docs service", ScopeRef::Store).instance_id();
        let resource = serde_json::json!({
            "uri": "fixture://docs/readme",
            "service_name": "docs service",
            "instance_id": instance_id,
        });
        let template = serde_json::json!({
            "uri_template": "fixture://docs/{name}",
            "service_name": "docs service",
            "instance_id": instance_id,
        });

        let projected_resource =
            project_catalog_uris(vec![resource.clone()], "uri", false).unwrap();
        let projected_resource_uri = projected_resource[0]["uri"].as_str().unwrap();
        assert_eq!(
            resolve_projected_catalog_uri(&[resource], "uri", false, projected_resource_uri)
                .unwrap(),
            (instance_id, "fixture://docs/readme".to_string())
        );

        let projected_template =
            project_catalog_uris(vec![template.clone()], "uri_template", true).unwrap();
        let projected_template_uri = projected_template[0]["uri_template"].as_str().unwrap();
        assert!(projected_template_uri.contains("/template/"));
        assert_eq!(
            resolve_projected_catalog_uri(
                &[template],
                "uri_template",
                true,
                projected_template_uri,
            )
            .unwrap(),
            (instance_id, "fixture://docs/{name}".to_string())
        );
    }

    #[test]
    fn structured_scope_argument_has_no_name_fallback() {
        let store = Map::from_iter([("scope".to_string(), serde_json::json!({"type": "store"}))]);
        assert_eq!(
            required_scope_argument(&store).expect("store scope"),
            ScopeRef::Store
        );

        let agent = Map::from_iter([(
            "scope".to_string(),
            serde_json::json!({"type": "agent", "agent_id": "agent-a"}),
        )]);
        assert_eq!(
            required_scope_argument(&agent).expect("agent scope"),
            ScopeRef::Agent {
                agent_id: "agent-a".to_string()
            }
        );

        let legacy = Map::from_iter([("scope".to_string(), Value::String("agent-a".to_string()))]);
        assert!(required_scope_argument(&legacy).is_err());
    }

    #[test]
    fn runtime_schema_requires_instance_id() {
        for schema in [instance_id_schema(), service_wait_schema()] {
            assert_eq!(
                schema.get("required"),
                Some(&serde_json::json!(["instance_id"]))
            );
            assert!(schema["properties"].get("name").is_none());
            assert!(schema["properties"].get("service_name").is_none());
        }

        let service_tools = build_service_tools();
        let check_tool = service_tools
            .get(SERVICE_CHECK_TOOL)
            .expect("instance check tool");
        assert_eq!(
            check_tool.input_schema.get("required"),
            Some(&serde_json::json!(["instance_id"]))
        );
    }

    #[test]
    fn config_schema_requires_service_name_and_scope() {
        for schema in [
            service_scope_schema(),
            service_add_schema(),
            service_scope_descriptor_schema(),
        ] {
            let required = schema["required"]
                .as_array()
                .expect("required fields must be an array");
            assert!(required.contains(&Value::String("service_name".to_string())));
            assert!(required.contains(&Value::String("scope".to_string())));
            assert!(schema["properties"].get("instance_id").is_none());
        }
    }

    #[test]
    fn http_path_normalization_is_stable() {
        assert_eq!(normalize_http_path(""), "/mcp");
        assert_eq!(normalize_http_path("custom/"), "/custom");
        assert_eq!(normalize_http_path("/custom/"), "/custom");
    }
}
