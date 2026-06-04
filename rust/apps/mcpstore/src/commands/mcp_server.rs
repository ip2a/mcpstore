use std::collections::HashMap;
use std::sync::Arc;

use clap::{Args, ValueEnum};
use mcpstore::{perspective::GLOBAL_AGENT_STORE, MCPStore};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, Content, GetPromptRequestParams, GetPromptResult,
        Implementation, ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult,
        ListToolsResult, PaginatedRequestParams, Prompt, ReadResourceRequestParams,
        ReadResourceResult, Resource, ResourceTemplate, ServerCapabilities, ServerInfo, Tool,
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
}

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
    scope_label: String,
    bindings: Arc<HashMap<String, ToolBinding>>,
    tools: Arc<Vec<Tool>>,
}

impl McpStoreServer {
    async fn from_store(store: Arc<MCPStore>, agent_id: Option<String>) -> Result<Self, BoxErr> {
        connect_scoped_services(&store, agent_id.as_deref()).await?;
        let bindings = build_tool_bindings(&store, agent_id.as_deref()).await?;
        let mut tools = bindings
            .values()
            .map(|binding| binding.tool.clone())
            .collect::<Vec<_>>();
        tools.sort_by(|left, right| left.name.cmp(&right.name));

        let scope_label = match agent_id.as_deref() {
            Some(agent_id) => format!("agent:{agent_id}"),
            None => "store".to_string(),
        };

        Ok(Self {
            store,
            agent_id,
            scope_label,
            bindings: Arc::new(bindings),
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
        let tools = self.tools.as_ref().clone();
        async move { Ok(ListToolsResult::with_all_items(tools)) }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.bindings.get(name).map(|binding| binding.tool.clone())
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let agent_id = self.agent_id.clone();
        async move {
            let resources = store
                .list_resources_scoped(agent_id.as_deref(), None)
                .await
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
        async move {
            let templates = store
                .list_resource_templates_scoped(agent_id.as_deref(), None)
                .await
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
        async move {
            let result = store
                .read_resource_scoped(agent_id.as_deref(), &request.uri, None)
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
        let agent_id = self.agent_id.clone();
        async move {
            let prompts = store
                .list_prompts_scoped(agent_id.as_deref(), None)
                .await
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
        async move {
            let arguments = Value::Object(request.arguments.unwrap_or_default());
            let result = store
                .get_prompt_scoped(agent_id.as_deref(), &request.name, arguments, None)
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
        let binding = self.bindings.get(request.name.as_ref()).cloned();
        let store = Arc::clone(&self.store);
        async move {
            let binding = binding.ok_or_else(|| {
                ErrorData::invalid_params(format!("未知工具: {}", request.name), None)
            })?;

            let args = Value::Object(request.arguments.unwrap_or_default());
            let result = store
                .call_tool(
                    &binding.global_service_name,
                    &binding.original_tool_name,
                    args,
                )
                .await
                .map_err(map_store_error)?;

            let mut content = Vec::with_capacity(result.content.len());
            for item in result.content {
                match item {
                    mcpstore::transport::ContentItem::Text { text } => {
                        content.push(Content::text(text));
                    }
                    mcpstore::transport::ContentItem::Image { data, mime_type } => {
                        content.push(Content::image(data, mime_type));
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

    let server = McpStoreServer::from_store(Arc::clone(&store), agent_id).await?;
    match args.transport {
        McpServerTransport::Stdio => {
            let running = serve_server(server, stdio()).await?;
            running.waiting().await?;
            Ok(())
        }
        McpServerTransport::StreamableHttp => run_streamable_http(server, &args).await,
    }
}

async fn connect_scoped_services(store: &MCPStore, agent_id: Option<&str>) -> Result<(), BoxErr> {
    let mut service_names = if let Some(agent_id) = agent_id {
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
) -> Result<HashMap<String, ToolBinding>, BoxErr> {
    let scope_id = agent_id.unwrap_or(GLOBAL_AGENT_STORE);
    let tool_payloads = store.list_tools_scoped(agent_id, None).await?;
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
}
