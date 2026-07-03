use std::collections::HashMap;
use std::sync::Arc;

use clap::{Args, ValueEnum};
use mcpstore::{
    perspective::GLOBAL_AGENT_STORE, CacheStorage, MCPStore, OpenApiBundleOptions,
    OpenApiImportOptions, OpenApiRefCachePolicy, ToolTransformPatch,
};
use rmcp::{
    model::{
        AnnotateAble, CallToolRequestParams, CallToolResult, Content, GetPromptRequestParams,
        GetPromptResult, Implementation, ListPromptsResult, ListResourceTemplatesResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, Prompt, RawAudioContent,
        RawContent, RawResource, ReadResourceRequestParams, ReadResourceResult, Resource,
        ResourceContents, ResourceTemplate, ServerCapabilities, ServerInfo, Tool, ToolAnnotations,
    },
    serve_server,
    transport::{
        stdio, streamable_http_server::session::local::LocalSessionManager,
        StreamableHttpServerConfig, StreamableHttpService,
    },
    ErrorData, ServerHandler,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::{
    commands::mcp::Scope,
    store_args::{build_store, StoreSourceArgs},
    BoxErr,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum McpServerTransport {
    Stdio,
    #[value(name = "streamable-http", alias = "http")]
    StreamableHttp,
}

#[derive(Args)]
pub struct McpServerArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(
        long,
        help = "Optional service name to expose within the selected store or agent scope"
    )]
    pub service: Option<String>,
    #[arg(
        long,
        value_enum,
        default_value_t = McpServerTransport::Stdio,
        help = "MCP transport: stdio or streamable-http"
    )]
    pub transport: McpServerTransport,
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "绑定地址，仅 streamable-http 使用"
    )]
    pub host: String,
    #[arg(
        long,
        default_value_t = 18300,
        help = "监听端口，仅 streamable-http 使用"
    )]
    pub port: u16,
    #[arg(
        long,
        default_value = "/mcp",
        help = "HTTP 路径，仅 streamable-http 使用"
    )]
    pub path: String,
    #[arg(
        long,
        help = "MCPStore 业务 session key；与 rmcp transport session 分离"
    )]
    pub session_key: Option<String>,
    #[arg(
        long,
        help = "Expose MCPStore session_state management tools. Disabled by default."
    )]
    pub expose_session_state_tools: bool,
    #[arg(
        long,
        help = "Expose MCPStore tool transform management tools. Disabled by default."
    )]
    pub expose_tool_transform_tools: bool,
    #[arg(
        long,
        help = "Expose MCPStore OpenAPI import management tools. Disabled by default."
    )]
    pub expose_openapi_tools: bool,
    #[arg(
        long,
        help = "Expose MCPStore cache backend management tools. Disabled by default."
    )]
    pub expose_cache_tools: bool,
    #[arg(
        long,
        help = "Expose MCPStore event observability tools. Disabled by default."
    )]
    pub expose_event_tools: bool,
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
const CACHE_HEALTH_TOOL: &str = "mcpstore_cache_health";
const CACHE_INSPECT_TOOL: &str = "mcpstore_cache_inspect";
const CACHE_SWITCH_TOOL: &str = "mcpstore_cache_switch";
const EVENT_HISTORY_TOOL: &str = "mcpstore_event_history";
const EVENT_CAPABILITY_REPORT_TOOL: &str = "mcpstore_event_capability_report";

#[derive(Clone)]
struct ToolBinding {
    tool: Tool,
    global_service_name: String,
    original_tool_name: String,
}

#[derive(Clone)]
struct McpStoreServer {
    store: Arc<MCPStore>,
    agent_id: Option<String>,
    service_name: Option<String>,
    session_key: Option<String>,
    scope_label: String,
    bindings: Arc<HashMap<String, ToolBinding>>,
    session_state_tools: Arc<HashMap<String, Tool>>,
    tool_transform_tools: Arc<HashMap<String, Tool>>,
    openapi_tools: Arc<HashMap<String, Tool>>,
    cache_tools: Arc<HashMap<String, Tool>>,
    event_tools: Arc<HashMap<String, Tool>>,
    tools: Arc<Vec<Tool>>,
}

impl McpStoreServer {
    async fn from_store(
        store: Arc<MCPStore>,
        agent_id: Option<String>,
        service_name: Option<String>,
        session_key: Option<String>,
        expose_session_state_tools: bool,
        expose_tool_transform_tools: bool,
        expose_openapi_tools: bool,
        expose_cache_tools: bool,
        expose_event_tools: bool,
    ) -> Result<Self, BoxErr> {
        connect_scoped_services(&store, agent_id.as_deref(), service_name.as_deref()).await?;
        if let Some(session_key) = session_key.as_deref() {
            store.session_by_key(session_key).status().await?;
        }
        let bindings = build_tool_bindings(
            &store,
            agent_id.as_deref(),
            service_name.as_deref(),
            session_key.as_deref(),
        )
        .await?;
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
        tools.extend(cache_tools.values().cloned());
        tools.extend(event_tools.values().cloned());
        tools.sort_by(|left, right| left.name.cmp(&right.name));

        let scope_label = match agent_id.as_deref() {
            Some(agent_id) => format!("agent:{agent_id}"),
            None => "store".to_string(),
        };
        let scope_label = match service_name.as_deref() {
            Some(service_name) => format!("{scope_label} service:{service_name}"),
            None => scope_label,
        };
        let scope_label = match session_key.as_deref() {
            Some(session_key) => format!("{scope_label} session:{session_key}"),
            None => scope_label,
        };

        Ok(Self {
            store,
            agent_id,
            service_name,
            session_key,
            scope_label,
            bindings: Arc::new(bindings),
            session_state_tools: Arc::new(session_state_tools),
            tool_transform_tools: Arc::new(tool_transform_tools),
            openapi_tools: Arc::new(openapi_tools),
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
            self.agent_id.as_deref(),
            self.service_name.as_deref(),
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
        tools.extend(self.cache_tools.values().cloned());
        tools.extend(self.event_tools.values().cloned());
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(tools)
    }
}

impl ServerHandler for McpStoreServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
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
            .or_else(|| self.cache_tools.get(name).cloned())
            .or_else(|| self.event_tools.get(name).cloned())
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let agent_id = self.agent_id.clone();
        let service_name = self.service_name.clone();
        let session_key = self.session_key.clone();
        async move {
            let resources = if let Some(session_key) = session_key.as_deref() {
                store.list_resources_in_session(session_key).await
            } else {
                store
                    .list_resources_scoped(agent_id.as_deref(), service_name.as_deref())
                    .await
            }
            .map_err(map_store_error)?;
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
        let agent_id = self.agent_id.clone();
        let service_name = self.service_name.clone();
        let session_key = self.session_key.clone();
        async move {
            let templates = if let Some(session_key) = session_key.as_deref() {
                store.list_resource_templates_in_session(session_key).await
            } else {
                store
                    .list_resource_templates_scoped(agent_id.as_deref(), service_name.as_deref())
                    .await
            }
            .map_err(map_store_error)?;
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
        let agent_id = self.agent_id.clone();
        let service_name = self.service_name.clone();
        let session_key = self.session_key.clone();
        async move {
            let result = if let Some(session_key) = session_key.as_deref() {
                store
                    .read_resource_in_session(session_key, &request.uri, None)
                    .await
            } else {
                store
                    .read_resource_scoped(
                        agent_id.as_deref(),
                        &request.uri,
                        service_name.as_deref(),
                    )
                    .await
            }
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
        let agent_id = self.agent_id.clone();
        let service_name = self.service_name.clone();
        let session_key = self.session_key.clone();
        async move {
            let prompts = if let Some(session_key) = session_key.as_deref() {
                store.list_prompts_in_session(session_key).await
            } else {
                store
                    .list_prompts_scoped(agent_id.as_deref(), service_name.as_deref())
                    .await
            }
            .map_err(map_store_error)?;
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
        let agent_id = self.agent_id.clone();
        let service_name = self.service_name.clone();
        let session_key = self.session_key.clone();
        async move {
            let arguments = Value::Object(request.arguments.unwrap_or_default());
            let result = if let Some(session_key) = session_key.as_deref() {
                store
                    .get_prompt_in_session(session_key, &request.name, arguments, None)
                    .await
            } else {
                store
                    .get_prompt_scoped(
                        agent_id.as_deref(),
                        &request.name,
                        arguments,
                        service_name.as_deref(),
                    )
                    .await
            }
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
        let is_cache_tool = self.cache_tools.contains_key(tool_name.as_str());
        let is_event_tool = self.event_tools.contains_key(tool_name.as_str());
        let store = Arc::clone(&self.store);
        let agent_id = self.agent_id.clone();
        let service_name = self.service_name.clone();
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
            if is_cache_tool {
                return call_cache_tool(&store, &tool_name, arguments).await;
            }
            if is_event_tool {
                return call_event_tool(&store, &tool_name, arguments).await;
            }

            let binding = match binding {
                Some(binding) => binding,
                None => build_tool_bindings(
                    &store,
                    agent_id.as_deref(),
                    service_name.as_deref(),
                    default_session_key.as_deref(),
                )
                .await
                .map_err(|error| ErrorData::internal_error(error.to_string(), None))?
                .remove(tool_name.as_str())
                .ok_or_else(|| ErrorData::invalid_params(format!("未知工具: {tool_name}"), None))?,
            };

            let (args, session_key) = extract_business_session_key(
                meta.as_ref(),
                arguments,
                default_session_key.as_deref(),
            );
            let result = if let Some(session_key) = session_key.as_deref() {
                store
                    .call_tool_in_session(session_key, &binding.tool.name, args)
                    .await
            } else {
                store
                    .call_tool(
                        &binding.global_service_name,
                        &binding.original_tool_name,
                        args,
                    )
                    .await
            }
            .map_err(map_store_error)?;

            let mut content = Vec::with_capacity(result.content.len());
            for item in result.content {
                match item {
                    mcpstore::transport::ContentItem::Text { text, .. } => {
                        content.push(Content::text(text));
                    }
                    mcpstore::transport::ContentItem::Image {
                        data, mime_type, ..
                    } => {
                        content.push(Content::image(data, mime_type));
                    }
                    mcpstore::transport::ContentItem::Audio {
                        data, mime_type, ..
                    } => {
                        content.push(
                            RawContent::Audio(RawAudioContent { data, mime_type }).no_annotation(),
                        );
                    }
                    mcpstore::transport::ContentItem::Resource { resource, .. } => {
                        content.push(match serde_json::from_value::<ResourceContents>(resource) {
                            Ok(resource) => Content::resource(resource),
                            Err(error) => {
                                Content::text(format!("Failed to decode resource content: {error}"))
                            }
                        });
                    }
                    mcpstore::transport::ContentItem::ResourceLink { resource, .. } => {
                        content.push(match serde_json::from_value::<RawResource>(resource) {
                            Ok(resource) => Content::resource_link(resource),
                            Err(error) => {
                                Content::text(format!("Failed to decode resource link: {error}"))
                            }
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

pub async fn run(args: McpServerArgs) -> Result<(), BoxErr> {
    let agent_id = scope_agent_id(&args.scope, args.agent.clone())?;

    let store = Arc::new(build_store(&args.store)?);
    store.load_from_source().await?;

    let server = McpStoreServer::from_store(
        Arc::clone(&store),
        agent_id,
        args.service.clone(),
        args.session_key.clone(),
        args.expose_session_state_tools,
        args.expose_tool_transform_tools,
        args.expose_openapi_tools,
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

async fn connect_scoped_services(
    store: &MCPStore,
    agent_id: Option<&str>,
    service_name: Option<&str>,
) -> Result<(), BoxErr> {
    let mut service_names = if let Some(service_name) = service_name {
        let service = store.service_info_scoped(agent_id, service_name).await?;
        let global_service_name = service
            .get("global_name")
            .or_else(|| service.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Service metadata missing name: {service_name}"))?;
        vec![global_service_name.to_string()]
    } else if let Some(agent_id) = agent_id {
        store.list_agent_service_names(agent_id).await?
    } else {
        store
            .list_services()
            .await
            .into_iter()
            .map(|service| service.name)
            .collect()
    };
    service_names.sort();
    service_names.dedup();

    for service_name in service_names {
        store.connect_service(&service_name).await?;
    }
    Ok(())
}

async fn build_tool_bindings(
    store: &MCPStore,
    agent_id: Option<&str>,
    service_name: Option<&str>,
    session_key: Option<&str>,
) -> Result<HashMap<String, ToolBinding>, BoxErr> {
    let scope_id = agent_id.unwrap_or(GLOBAL_AGENT_STORE);
    let tool_payloads = if let Some(session_key) = session_key {
        serde_json::to_value(store.list_tools_in_session(session_key).await?)
            .and_then(serde_json::from_value::<Vec<Value>>)
            .map_err(|error| format!("session tool metadata serialization failed: {error}"))?
    } else {
        store.list_tools_scoped(agent_id, service_name).await?
    };
    let mut bindings = HashMap::with_capacity(tool_payloads.len());

    for payload in tool_payloads {
        let tool_name = read_required_string(&payload, "name")?;
        let original_tool_name = read_required_string(&payload, "original_name")?;
        let global_service_name = read_required_string(&payload, "global_service_name")?;
        let description = payload
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);
        let schema = read_required_object(&payload, "input_schema")?;

        let tool = Tool::new_with_raw(
            tool_name.clone(),
            description.clone().map(Into::into),
            Arc::new(schema),
        );
        let existing = bindings.insert(
            tool_name.clone(),
            ToolBinding {
                tool,
                global_service_name,
                original_tool_name,
            },
        );
        if existing.is_some() {
            return Err(format!(
                "重复工具名，无法构建 Rust MCP server: scope={scope_id} tool={tool_name}"
            )
            .into());
        }
    }

    Ok(bindings)
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
            tool_transform_schema(&["service_name", "tool_name"]),
            true,
        ),
        tool_transform_tool(
            TOOL_TRANSFORM_SET_TOOL,
            "Set one Rust-backed MCPStore tool transform rule.",
            tool_transform_schema(&["service_name", "tool_name"]),
            false,
        ),
        tool_transform_tool(
            TOOL_TRANSFORM_DELETE_TOOL,
            "Delete one Rust-backed MCPStore tool transform rule.",
            tool_transform_schema(&["service_name", "tool_name"]),
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
        "service_name".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "MCPStore service name or global service name."
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
            let service_name = required_argument_string(&arguments, "service_name")?;
            let tool_name = required_argument_string(&arguments, "tool_name")?;
            let transform = store
                .get_tool_transform(service_name, tool_name)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"transform": transform})
        }
        TOOL_TRANSFORM_SET_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?.to_string();
            let original_tool_name = required_argument_string(&arguments, "tool_name")?.to_string();
            let patch = serde_json::from_value::<ToolTransformPatch>(Value::Object(arguments))
                .map_err(|error| {
                    ErrorData::invalid_params(
                        format!("工具转换规则参数反序列化失败: {error}"),
                        None,
                    )
                })?;
            let transform = store
                .set_tool_transform(&service_name, &original_tool_name, patch)
                .await
                .map_err(map_store_error)?;
            serde_json::json!({"transform": transform})
        }
        TOOL_TRANSFORM_DELETE_TOOL => {
            let service_name = required_argument_string(&arguments, "service_name")?;
            let tool_name = required_argument_string(&arguments, "tool_name")?;
            store
                .delete_tool_transform(service_name, tool_name)
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
        ErrorData::invalid_params(format!("OpenAPI {field} must be a positive integer"), None)
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

fn read_required_object(payload: &Value, field: &str) -> Result<Map<String, Value>, BoxErr> {
    payload
        .get(field)
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| format!("工具元数据缺少对象字段: {field}").into())
}

fn scope_agent_id(scope: &Scope, agent: Option<String>) -> Result<Option<String>, BoxErr> {
    match scope {
        Scope::Store | Scope::Project => Ok(None),
        Scope::Agent => match agent {
            Some(agent_id) if !agent_id.is_empty() => Ok(Some(agent_id)),
            _ => Err("`mcp-server --scope agent` 需要同时提供 `--agent <id>`".into()),
        },
    }
}

fn map_store_error(error: mcpstore::StoreError) -> ErrorData {
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

async fn run_streamable_http(server: McpStoreServer, args: &McpServerArgs) -> Result<(), BoxErr> {
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
    use mcpstore::{
        cache::models::{
            AgentServiceRelation, ServiceEntity, ServiceRelationItem, ServiceToolRelation,
            ToolEntity, ToolRelationItem,
        },
        CacheStorage, CreateSessionRequest, ServerConfig, SourceMode, StoreOptions,
    };
    use std::time::SystemTime;

    fn unique_namespace() -> String {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        format!("mcp-server-session-test-{nanos}")
    }

    fn stdio_config() -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("echo".to_string()),
            args: vec!["fixture".to_string()],
            env: HashMap::new(),
            headers: HashMap::new(),
            transport: Some("stdio".to_string()),
            working_dir: None,
            description: Some("fixture".to_string()),
        }
    }

    async fn seed_db_service(store: &MCPStore, service_name: &str, tool_name: &str) {
        let config = stdio_config();
        let global_tool_name = format!("{service_name}_{tool_name}");
        let cache = store.cache();
        cache
            .put_entity(
                "services",
                service_name,
                serde_json::to_value(ServiceEntity {
                    service_global_name: service_name.to_string(),
                    service_original_name: service_name.to_string(),
                    source_agent: "global_agent_store".to_string(),
                    config: serde_json::to_value(config).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        cache
            .put_entity(
                "tools",
                &global_tool_name,
                serde_json::to_value(ToolEntity {
                    tool_global_name: global_tool_name.clone(),
                    tool_original_name: tool_name.to_string(),
                    service_global_name: service_name.to_string(),
                    service_original_name: service_name.to_string(),
                    source_agent: "global_agent_store".to_string(),
                    description: format!("{tool_name} tool"),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Original text."},
                            "debug": {"type": "boolean"}
                        },
                        "required": ["text", "debug"]
                    }),
                    created_time: 111,
                    tool_hash: format!("{service_name}:{tool_name}:fixture"),
                })
                .unwrap(),
            )
            .await
            .unwrap();
        cache
            .put_relation(
                "service_tools",
                service_name,
                serde_json::to_value(ServiceToolRelation {
                    service_global_name: service_name.to_string(),
                    service_original_name: service_name.to_string(),
                    source_agent: "global_agent_store".to_string(),
                    tools: vec![ToolRelationItem {
                        tool_global_name: global_tool_name,
                        tool_original_name: tool_name.to_string(),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();
    }

    async fn seed_global_agent_relation(store: &MCPStore, services: &[&str]) {
        store
            .cache()
            .put_relation(
                "agent_services",
                "global_agent_store",
                serde_json::to_value(AgentServiceRelation {
                    services: services
                        .iter()
                        .map(|service_name| ServiceRelationItem {
                            service_original_name: (*service_name).to_string(),
                            service_global_name: (*service_name).to_string(),
                            client_id: (*service_name).to_string(),
                            established_time: 111,
                            last_access: None,
                        })
                        .collect(),
                })
                .unwrap(),
            )
            .await
            .unwrap();
    }

    #[test]
    fn agent_scope_requires_agent_flag() {
        let error = scope_agent_id(&Scope::Agent, None).expect_err("expected error");
        assert!(error.to_string().contains("--agent"));
    }

    #[test]
    fn store_scope_ignores_agent_flag() {
        let agent_id = scope_agent_id(&Scope::Store, Some("agent-a".to_string())).unwrap();
        assert!(agent_id.is_none());
    }

    #[test]
    fn normalize_http_path_defaults_to_mcp() {
        assert_eq!(normalize_http_path(""), "/mcp");
        assert_eq!(normalize_http_path("/"), "/mcp");
        assert_eq!(normalize_http_path("mcp"), "/mcp");
        assert_eq!(normalize_http_path("/mcp/"), "/mcp");
    }

    #[test]
    fn business_session_key_prefers_meta_and_strips_argument_control_field() {
        let mut meta = rmcp::model::Meta::new();
        meta.0.insert(
            "_mcpstore_session_key".to_string(),
            Value::String("store:global:from-meta".to_string()),
        );
        let mut args = Map::new();
        args.insert("name".to_string(), Value::String("Ada".to_string()));
        args.insert(
            "_mcpstore_session_key".to_string(),
            Value::String("store:global:from-args".to_string()),
        );

        let (forwarded, session_key) =
            extract_business_session_key(Some(&meta), args, Some("store:global:default"));

        assert_eq!(session_key.as_deref(), Some("store:global:from-meta"));
        assert_eq!(forwarded["name"], "Ada");
        assert!(forwarded.get("_mcpstore_session_key").is_none());
    }

    #[tokio::test]
    async fn mcp_server_uses_rust_core_session_state_for_tool_bindings() {
        let store = Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: None,
                source_mode: SourceMode::Db,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some(unique_namespace()),
            })
            .unwrap(),
        );
        seed_db_service(&store, "alpha", "echo").await;
        seed_db_service(&store, "beta", "search").await;
        seed_global_agent_relation(&store, &["alpha", "beta"]).await;

        let session = store
            .create_session(CreateSessionRequest::store("mcp-core-session"))
            .await
            .unwrap();
        store
            .bind_service_to_session(&session.session_key, "alpha")
            .await
            .unwrap();

        let server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            Some(session.session_key.clone()),
            false,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        let tool_names = server
            .tools
            .iter()
            .map(|tool| tool.name.as_ref().to_string())
            .collect::<Vec<_>>();

        assert_eq!(tool_names, vec!["alpha_echo"]);
        assert!(server.bindings.contains_key("alpha_echo"));
        assert!(!server.bindings.contains_key("beta_search"));

        store
            .close_session(&session.session_key, Some("done".to_string()))
            .await
            .unwrap();
        let error = match McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            Some(session.session_key.clone()),
            false,
            false,
            false,
            false,
            false,
        )
        .await
        {
            Ok(_) => panic!("closed business session must not start an active MCP server view"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("Session is not active"));
    }

    #[tokio::test]
    async fn mcp_server_can_expose_single_service_scope() {
        let store = Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: None,
                source_mode: SourceMode::Db,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some(unique_namespace()),
            })
            .unwrap(),
        );
        seed_db_service(&store, "alpha", "echo").await;
        seed_db_service(&store, "beta", "search").await;
        seed_global_agent_relation(&store, &["alpha", "beta"]).await;

        let server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            Some("alpha".to_string()),
            None,
            false,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        let tool_names = server
            .tools
            .iter()
            .map(|tool| tool.name.as_ref().to_string())
            .collect::<Vec<_>>();

        assert_eq!(tool_names, vec!["echo"]);
        assert!(server.bindings.contains_key("echo"));
        assert!(!server.bindings.contains_key("beta_search"));
        assert!(server.instructions().contains("service:alpha"));
    }

    #[tokio::test]
    async fn mcp_server_session_state_tools_are_explicit_and_use_rust_core() {
        let store = Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: None,
                source_mode: SourceMode::Db,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some(unique_namespace()),
            })
            .unwrap(),
        );
        seed_db_service(&store, "alpha", "echo").await;
        seed_global_agent_relation(&store, &["alpha"]).await;
        let session = store
            .create_session(CreateSessionRequest::store("mcp-state-tools"))
            .await
            .unwrap();

        let default_server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            Some(session.session_key.clone()),
            false,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(!default_server
            .tools
            .iter()
            .any(|tool| tool.name.as_ref() == SESSION_STATE_SET_TOOL));

        let server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            Some(session.session_key.clone()),
            true,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        let tool_names = server
            .tools
            .iter()
            .map(|tool| tool.name.as_ref().to_string())
            .collect::<Vec<_>>();
        assert!(tool_names.contains(&SESSION_STATE_LIST_TOOL.to_string()));
        assert!(tool_names.contains(&SESSION_STATE_GET_TOOL.to_string()));
        assert!(tool_names.contains(&SESSION_STATE_SET_TOOL.to_string()));
        assert!(tool_names.contains(&SESSION_STATE_DELETE_TOOL.to_string()));
        assert!(tool_names.contains(&SESSION_STATE_CLEAR_TOOL.to_string()));
        assert!(tool_names.contains(&SESSION_SNAPSHOT_EXPORT_TOOL.to_string()));
        assert!(tool_names.contains(&SESSION_SNAPSHOT_IMPORT_TOOL.to_string()));

        let set_args = serde_json::json!({
            "key": "cursor",
            "value": {"page": 2},
        })
        .as_object()
        .cloned()
        .unwrap();
        let set_result = call_session_state_tool(
            &store,
            SESSION_STATE_SET_TOOL,
            None,
            set_args,
            Some(&session.session_key),
        )
        .await
        .unwrap();
        assert_eq!(
            set_result.structured_content.as_ref().unwrap()["state"]["values"]["cursor"]["page"],
            2
        );

        let get_args = serde_json::json!({"key": "cursor"})
            .as_object()
            .cloned()
            .unwrap();
        let get_result = call_session_state_tool(
            &store,
            SESSION_STATE_GET_TOOL,
            None,
            get_args,
            Some(&session.session_key),
        )
        .await
        .unwrap();
        assert_eq!(
            get_result.structured_content.as_ref().unwrap()["value"]["page"],
            2
        );

        let delete_args =
            serde_json::json!({"session_key": session.session_key.clone(), "key": "cursor"})
                .as_object()
                .cloned()
                .unwrap();
        let delete_result =
            call_session_state_tool(&store, SESSION_STATE_DELETE_TOOL, None, delete_args, None)
                .await
                .unwrap();
        assert!(
            delete_result.structured_content.as_ref().unwrap()["state"]["values"]
                .as_object()
                .unwrap()
                .is_empty()
        );

        let mut meta = rmcp::model::Meta::new();
        meta.0.insert(
            "_mcpstore_session_key".to_string(),
            Value::String(session.session_key.clone()),
        );
        let clear_result = call_session_state_tool(
            &store,
            SESSION_STATE_CLEAR_TOOL,
            Some(&meta),
            Map::new(),
            None,
        )
        .await
        .unwrap();
        assert!(
            clear_result.structured_content.as_ref().unwrap()["state"]["values"]
                .as_object()
                .unwrap()
                .is_empty()
        );

        let set_snapshot_args = serde_json::json!({
            "session_key": session.session_key,
            "key": "cursor",
            "value": {"page": 5},
        })
        .as_object()
        .cloned()
        .unwrap();
        call_session_state_tool(
            &store,
            SESSION_STATE_SET_TOOL,
            None,
            set_snapshot_args,
            None,
        )
        .await
        .unwrap();
        let export_result =
            call_session_state_tool(&store, SESSION_SNAPSHOT_EXPORT_TOOL, None, Map::new(), None)
                .await
                .unwrap();
        let snapshot = export_result.structured_content.as_ref().unwrap()["snapshot"].clone();
        assert_eq!(
            snapshot["states"]["session_state"][&session.session_key]["values"]["cursor"]["page"],
            5
        );

        let target_store = Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: None,
                source_mode: SourceMode::Db,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some(unique_namespace()),
            })
            .unwrap(),
        );
        seed_db_service(&target_store, "alpha", "echo").await;
        seed_global_agent_relation(&target_store, &["alpha"]).await;
        let import_args = serde_json::json!({"snapshot": snapshot})
            .as_object()
            .cloned()
            .unwrap();
        let import_result = call_session_state_tool(
            &target_store,
            SESSION_SNAPSHOT_IMPORT_TOOL,
            None,
            import_args.clone(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            import_result.structured_content.as_ref().unwrap()["report"]["sessions_imported"],
            1
        );
        assert_eq!(
            target_store
                .get_session_state_value(&session.session_key, "cursor")
                .await
                .unwrap()
                .unwrap()["page"],
            5
        );

        let import_again = call_session_state_tool(
            &target_store,
            SESSION_SNAPSHOT_IMPORT_TOOL,
            None,
            import_args,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            import_again.structured_content.as_ref().unwrap()["report"]["sessions_unchanged"],
            1
        );
    }

    #[tokio::test]
    async fn mcp_server_tool_transform_tools_are_explicit_and_use_rust_core() {
        let store = Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: None,
                source_mode: SourceMode::Db,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some(unique_namespace()),
            })
            .unwrap(),
        );
        seed_db_service(&store, "alpha", "echo").await;
        seed_global_agent_relation(&store, &["alpha"]).await;

        let default_server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(!default_server
            .tools
            .iter()
            .any(|tool| tool.name.as_ref() == TOOL_TRANSFORM_SET_TOOL));

        let server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            true,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        let tool_names = server
            .tools
            .iter()
            .map(|tool| tool.name.as_ref().to_string())
            .collect::<Vec<_>>();
        assert!(tool_names.contains(&TOOL_TRANSFORM_LIST_TOOL.to_string()));
        assert!(tool_names.contains(&TOOL_TRANSFORM_GET_TOOL.to_string()));
        assert!(tool_names.contains(&TOOL_TRANSFORM_SET_TOOL.to_string()));
        assert!(tool_names.contains(&TOOL_TRANSFORM_DELETE_TOOL.to_string()));

        let set_args = serde_json::json!({
            "service_name": "alpha",
            "tool_name": "echo",
            "display_name": "say",
            "description": "Say text with a stable hidden debug flag.",
            "arguments": [
                {
                    "original_name": "text",
                    "new_name": "message",
                    "hidden": false,
                    "description": "Message to echo."
                },
                {
                    "original_name": "debug",
                    "hidden": true,
                    "default_value": false
                }
            ],
            "tags": ["compat"],
            "enabled": true
        })
        .as_object()
        .cloned()
        .unwrap();
        let set_result = call_tool_transform_tool(&store, TOOL_TRANSFORM_SET_TOOL, set_args)
            .await
            .unwrap();
        assert_eq!(
            set_result.structured_content.as_ref().unwrap()["transform"]["display_name"],
            "say"
        );

        let transformed_server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            true,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        let transformed_tool = transformed_server.bindings.get("say").unwrap();
        let schema = transformed_tool.tool.input_schema.as_ref();
        assert_eq!(
            schema["properties"]["message"]["description"],
            "Message to echo."
        );
        assert!(schema["properties"].get("debug").is_none());
        assert_eq!(schema["required"], serde_json::json!(["message"]));

        let list_result = call_tool_transform_tool(&store, TOOL_TRANSFORM_LIST_TOOL, Map::new())
            .await
            .unwrap();
        assert_eq!(list_result.structured_content.as_ref().unwrap()["total"], 1);

        let get_args = serde_json::json!({"service_name": "alpha", "tool_name": "say"})
            .as_object()
            .cloned()
            .unwrap();
        let get_result = call_tool_transform_tool(&store, TOOL_TRANSFORM_GET_TOOL, get_args)
            .await
            .unwrap();
        assert_eq!(
            get_result.structured_content.as_ref().unwrap()["transform"]["original_tool_name"],
            "echo"
        );

        let delete_args = serde_json::json!({"service_name": "alpha", "tool_name": "say"})
            .as_object()
            .cloned()
            .unwrap();
        let delete_result =
            call_tool_transform_tool(&store, TOOL_TRANSFORM_DELETE_TOOL, delete_args)
                .await
                .unwrap();
        assert_eq!(
            delete_result.structured_content.as_ref().unwrap()["status"],
            "ok"
        );

        let restored_server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            true,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(restored_server.bindings.contains_key("alpha_echo"));
    }

    #[tokio::test]
    async fn mcp_server_cache_tools_are_explicit_and_use_rust_core() {
        let store = Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: None,
                source_mode: SourceMode::Db,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some(unique_namespace()),
            })
            .unwrap(),
        );
        seed_db_service(&store, "alpha", "echo").await;
        seed_global_agent_relation(&store, &["alpha"]).await;

        let default_server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(!default_server
            .tools
            .iter()
            .any(|tool| tool.name.as_ref() == CACHE_SWITCH_TOOL));
        assert!(!default_server
            .tools
            .iter()
            .any(|tool| tool.name.as_ref() == CACHE_HEALTH_TOOL));
        assert!(!default_server
            .tools
            .iter()
            .any(|tool| tool.name.as_ref() == CACHE_INSPECT_TOOL));

        let server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            false,
            false,
            true,
            false,
        )
        .await
        .unwrap();
        let tool_names = server
            .tools
            .iter()
            .map(|tool| tool.name.as_ref().to_string())
            .collect::<Vec<_>>();
        assert!(tool_names.contains(&CACHE_HEALTH_TOOL.to_string()));
        assert!(tool_names.contains(&CACHE_INSPECT_TOOL.to_string()));
        assert!(tool_names.contains(&CACHE_SWITCH_TOOL.to_string()));
        let cache_tool = server.get_tool(CACHE_SWITCH_TOOL).unwrap();
        assert!(cache_tool.input_schema["properties"]
            .as_object()
            .unwrap()
            .contains_key("namespace"));

        let health_result = call_cache_tool(&store, CACHE_HEALTH_TOOL, Map::new())
            .await
            .unwrap();
        assert_eq!(
            health_result.structured_content.as_ref().unwrap()["health"]["backend"],
            "memory"
        );

        let inspect_result = call_cache_tool(&store, CACHE_INSPECT_TOOL, Map::new())
            .await
            .unwrap();
        assert_eq!(
            inspect_result.structured_content.as_ref().unwrap()["inspect"]["backend"],
            "memory"
        );
        assert!(
            inspect_result.structured_content.as_ref().unwrap()["inspect"]["counts"]["entities"]
                .as_object()
                .unwrap()
                .contains_key("services")
        );

        let switch_args = serde_json::json!({
            "backend": "openkeyv_memory",
            "namespace": unique_namespace(),
        })
        .as_object()
        .cloned()
        .unwrap();
        let switch_result = call_cache_tool(&store, CACHE_SWITCH_TOOL, switch_args)
            .await
            .unwrap();
        assert_eq!(
            store.current_cache_storage().await,
            CacheStorage::OpenKeyvMemory
        );
        assert_eq!(
            switch_result.structured_content.as_ref().unwrap()["backend"],
            "openkeyv_memory"
        );
        assert!(
            switch_result.structured_content.as_ref().unwrap()["snapshot"]["entities"]["services"]
                .as_object()
                .unwrap()
                .contains_key("alpha")
        );
        assert!(store.service_info_scoped(None, "alpha").await.is_ok());

        let invalid_args = serde_json::json!({"backend": "sqlite"})
            .as_object()
            .cloned()
            .unwrap();
        let invalid = call_cache_tool(&store, CACHE_SWITCH_TOOL, invalid_args)
            .await
            .unwrap_err();
        assert!(invalid.message.contains("不支持的 cache backend"));
    }

    #[tokio::test]
    async fn mcp_server_event_tools_are_explicit_and_use_rust_core() {
        let store = Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: None,
                source_mode: SourceMode::Db,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some(unique_namespace()),
            })
            .unwrap(),
        );
        seed_db_service(&store, "alpha", "echo").await;
        seed_global_agent_relation(&store, &["alpha"]).await;

        let default_server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(!default_server
            .tools
            .iter()
            .any(|tool| tool.name.as_ref() == EVENT_HISTORY_TOOL));
        assert!(!default_server
            .tools
            .iter()
            .any(|tool| tool.name.as_ref() == EVENT_CAPABILITY_REPORT_TOOL));

        let server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            false,
            false,
            false,
            true,
        )
        .await
        .unwrap();
        let tool_names = server
            .tools
            .iter()
            .map(|tool| tool.name.as_ref().to_string())
            .collect::<Vec<_>>();
        assert!(tool_names.contains(&EVENT_HISTORY_TOOL.to_string()));
        assert!(tool_names.contains(&EVENT_CAPABILITY_REPORT_TOOL.to_string()));
        let history_tool = server.get_tool(EVENT_HISTORY_TOOL).unwrap();
        assert!(history_tool.input_schema["properties"]
            .as_object()
            .unwrap()
            .contains_key("count"));

        store
            .publish_event("MCP_EVENT_TOOL_TEST", serde_json::json!({"ok": true}), true)
            .await
            .unwrap();
        let history_args = serde_json::json!({"count": 1})
            .as_object()
            .cloned()
            .unwrap();
        let history_result = call_event_tool(&store, EVENT_HISTORY_TOOL, history_args)
            .await
            .unwrap();
        assert_eq!(
            history_result.structured_content.as_ref().unwrap()["total"],
            1
        );
        assert_eq!(
            history_result.structured_content.as_ref().unwrap()["events"][0]["event_type"],
            "MCP_EVENT_TOOL_TEST"
        );

        let report_result = call_event_tool(&store, EVENT_CAPABILITY_REPORT_TOOL, Map::new())
            .await
            .unwrap();
        assert_eq!(
            report_result.structured_content.as_ref().unwrap()["report"]["event_bus"],
            true
        );

        let invalid_args = serde_json::json!({"count": 0})
            .as_object()
            .cloned()
            .unwrap();
        let invalid = call_event_tool(&store, EVENT_HISTORY_TOOL, invalid_args)
            .await
            .unwrap_err();
        assert!(invalid.message.contains("count must be a positive integer"));
    }

    #[tokio::test]
    async fn mcp_server_openapi_tools_are_explicit_and_use_rust_core() {
        let store = Arc::new(
            MCPStore::setup_with_options(StoreOptions {
                config_path: None,
                source_mode: SourceMode::Db,
                backend: Some(CacheStorage::Memory),
                redis_url: None,
                namespace: Some(unique_namespace()),
            })
            .unwrap(),
        );
        seed_db_service(&store, "alpha", "echo").await;
        seed_global_agent_relation(&store, &["alpha"]).await;

        let default_server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(!default_server
            .tools
            .iter()
            .any(|tool| tool.name.as_ref() == OPENAPI_IMPORT_SET_TOOL));

        let server = McpStoreServer::from_store(
            Arc::clone(&store),
            None,
            None,
            None,
            false,
            false,
            true,
            false,
            false,
        )
        .await
        .unwrap();
        let tool_names = server
            .tools
            .iter()
            .map(|tool| tool.name.as_ref().to_string())
            .collect::<Vec<_>>();
        assert!(tool_names.contains(&OPENAPI_IMPORT_LIST_TOOL.to_string()));
        assert!(tool_names.contains(&OPENAPI_IMPORT_GET_TOOL.to_string()));
        assert!(tool_names.contains(&OPENAPI_IMPORT_SET_TOOL.to_string()));
        assert!(tool_names.contains(&OPENAPI_BUNDLE_TOOL.to_string()));
        assert!(tool_names.contains(&OPENAPI_BUNDLE_ARTIFACT_TOOL.to_string()));
        let bundle_tool = server
            .tools
            .iter()
            .find(|tool| tool.name.as_ref() == OPENAPI_BUNDLE_ARTIFACT_TOOL)
            .unwrap();
        assert!(bundle_tool.input_schema["properties"]
            .as_object()
            .unwrap()
            .contains_key("ref_cache"));
        assert!(bundle_tool.input_schema["properties"]
            .as_object()
            .unwrap()
            .contains_key("fetch_timeout_millis"));
        assert!(bundle_tool.input_schema["properties"]
            .as_object()
            .unwrap()
            .contains_key("timeout_millis"));

        let fixture_dir = std::env::temp_dir().join(format!("mcpstore-mcp-{}", unique_namespace()));
        std::fs::create_dir_all(&fixture_dir).unwrap();
        let schemas_path = fixture_dir.join("schemas.json");
        std::fs::write(
            &schemas_path,
            serde_json::to_vec(&serde_json::json!({
                "Item": {
                    "type": "object",
                    "properties": {"id": {"type": "string"}},
                    "required": ["id"]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let bundle_args = serde_json::json!({
            "spec_url": fixture_dir.join("openapi.json").to_string_lossy(),
            "ref_cache": {"enabled": false},
            "spec": {
                "openapi": "3.0.0",
                "info": {"title": "Inventory", "version": "1.0.0"},
                "paths": {
                    "/items": {
                        "get": {
                            "operationId": "listItems",
                            "responses": {
                                "200": {
                                    "description": "ok",
                                    "content": {
                                        "application/json": {
                                            "schema": {"$ref": "./schemas.json#/Item"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        })
        .as_object()
        .cloned()
        .unwrap();
        let bundle_result = call_openapi_tool(&store, OPENAPI_BUNDLE_TOOL, bundle_args)
            .await
            .unwrap();
        assert_eq!(
            bundle_result.structured_content.as_ref().unwrap()["bundle"]["paths"]["/items"]["get"]
                ["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["id"]
                ["type"],
            "string"
        );
        assert!(store.list_openapi_imports().await.unwrap().is_empty());

        let artifact_args = serde_json::json!({
            "spec_url": fixture_dir.join("openapi.json").to_string_lossy(),
            "ref_cache": {"ttl_seconds": 19},
            "spec": {
                "openapi": "3.0.0",
                "info": {"title": "Inventory", "version": "1.0.0"},
                "paths": {
                    "/items": {
                        "get": {
                            "operationId": "listItems",
                            "responses": {
                                "200": {
                                    "description": "ok",
                                    "content": {
                                        "application/json": {
                                            "schema": {"$ref": "./schemas.json#/Item"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        })
        .as_object()
        .cloned()
        .unwrap();
        let artifact_result =
            call_openapi_tool(&store, OPENAPI_BUNDLE_ARTIFACT_TOOL, artifact_args)
                .await
                .unwrap();
        let artifact = &artifact_result.structured_content.as_ref().unwrap()["artifact"];
        assert_eq!(
            artifact["bundle"]["paths"]["/items"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["properties"]["id"]["type"],
            "string"
        );
        assert_eq!(artifact["documents"].as_array().unwrap().len(), 2);
        assert_eq!(artifact["dependencies"].as_array().unwrap().len(), 1);
        assert_eq!(
            artifact["dependencies"][0]["source_ref"],
            "./schemas.json#/Item"
        );
        assert_eq!(artifact["diagnostics"].as_array().unwrap().len(), 0);
        let ref_cache_states = store
            .cache()
            .get_all_states_async("openapi_ref_documents")
            .await
            .unwrap();
        assert_eq!(ref_cache_states.len(), 1);
        let cached = ref_cache_states.values().next().unwrap();
        assert_eq!(cached["ttl_seconds"], serde_json::json!(19));
        assert!(store.list_openapi_imports().await.unwrap().is_empty());

        let spec = serde_json::json!({
            "openapi": "3.0.0",
            "info": {"title": "Inventory", "version": "1.0.0"},
            "components": {
                "securitySchemes": {
                    "ApiKeyAuth": {"type": "apiKey", "in": "header", "name": "x-api-key"}
                }
            },
            "security": [{"ApiKeyAuth": []}],
            "paths": {
                "/items": {
                    "get": {"operationId": "listItems", "responses": {"200": {"description": "ok"}}},
                    "post": {"operationId": "createItem", "responses": {"201": {"description": "created"}}}
                }
            }
        });
        let import_args = serde_json::json!({
            "name": "inventory",
            "spec_url": "memory://inventory",
            "spec": spec,
            "timeout_millis": 4300,
            "fetch_timeout_millis": "4400",
            "auth": {"ApiKeyAuth": "secret"}
        })
        .as_object()
        .cloned()
        .unwrap();
        let import_result = call_openapi_tool(&store, OPENAPI_IMPORT_SET_TOOL, import_args)
            .await
            .unwrap();
        assert_eq!(
            import_result.structured_content.as_ref().unwrap()["import"]["service_name"],
            "inventory"
        );
        assert_eq!(
            import_result.structured_content.as_ref().unwrap()["import"]["runtime_executable"],
            true
        );
        assert_eq!(
            import_result.structured_content.as_ref().unwrap()["import"]["security_schemes"]
                ["ApiKeyAuth"]["name"],
            "x-api-key"
        );
        let service = store.service_info_scoped(None, "inventory").await.unwrap();
        assert_eq!(service["config"]["openapi_timeout_millis"], 4300);
        assert_eq!(service["config"]["openapi_fetch_timeout_millis"], 4400);

        let invalid_timeout_args = serde_json::json!({
            "spec_url": "memory://invalid-timeout",
            "spec": {"openapi": "3.0.0", "info": {"title": "Invalid", "version": "1.0.0"}, "paths": {}},
            "fetch_timeout_millis": 0
        })
        .as_object()
        .cloned()
        .unwrap();
        let invalid_timeout = call_openapi_tool(&store, OPENAPI_BUNDLE_TOOL, invalid_timeout_args)
            .await
            .unwrap_err();
        assert!(invalid_timeout
            .message
            .contains("fetch_timeout_millis must be a positive integer"));

        let list_result = call_openapi_tool(&store, OPENAPI_IMPORT_LIST_TOOL, Map::new())
            .await
            .unwrap();
        assert_eq!(list_result.structured_content.as_ref().unwrap()["total"], 1);

        let get_args = serde_json::json!({"name": "inventory"})
            .as_object()
            .cloned()
            .unwrap();
        let get_result = call_openapi_tool(&store, OPENAPI_IMPORT_GET_TOOL, get_args)
            .await
            .unwrap();
        assert_eq!(
            get_result.structured_content.as_ref().unwrap()["import"]["spec_info"]["title"],
            "Inventory"
        );

        std::fs::remove_dir_all(&fixture_dir).unwrap();
    }
}
