use clap::{Args, ValueEnum};
use mcpstore::mcp_server::{
    McpServerOptions as CoreMcpServerOptions, McpServerTransport as CoreMcpServerTransport,
    Scope as CoreScope,
};
use mcpstore::{CacheStorage, SourceMode};

use crate::{
    commands::mcp::Scope,
    store_args::{CacheStorageArg, StoreSourceArgs},
    BoxErr,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum McpServerTransport {
    Stdio,
    #[value(name = "streamable-http", alias = "http")]
    StreamableHttp,
}

impl McpServerTransport {
    fn to_core(self) -> CoreMcpServerTransport {
        match self {
            Self::Stdio => CoreMcpServerTransport::Stdio,
            Self::StreamableHttp => CoreMcpServerTransport::StreamableHttp,
        }
    }
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
        help = "Expose MCPStore service lifecycle management tools. Disabled by default."
    )]
    pub expose_service_tools: bool,
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

impl McpServerArgs {
    fn to_core_options(&self) -> CoreMcpServerOptions {
        let backend = self
            .store
            .backend
            .map(CacheStorageArg::as_cache_storage)
            .or_else(|| self.store.redis_url.as_ref().map(|_| CacheStorage::Redis));
        CoreMcpServerOptions {
            config_path: self.store.config_path.clone(),
            source_mode: match self.store.source {
                crate::store_args::SourceArg::Local => SourceMode::Local,
                crate::store_args::SourceArg::Db => SourceMode::Db,
            },
            backend,
            redis_url: self.store.redis_url.clone(),
            namespace: self.store.namespace.clone(),
            scope: match self.scope {
                Scope::Store | Scope::Project => CoreScope::Store,
                Scope::Agent => CoreScope::Agent,
            },
            agent: self.agent.clone(),
            service: self.service.clone(),
            transport: self.transport.to_core(),
            host: self.host.clone(),
            port: self.port,
            path: self.path.clone(),
            session_key: self.session_key.clone(),
            expose_session_state_tools: self.expose_session_state_tools,
            expose_tool_transform_tools: self.expose_tool_transform_tools,
            expose_openapi_tools: self.expose_openapi_tools,
            expose_service_tools: self.expose_service_tools,
            expose_cache_tools: self.expose_cache_tools,
            expose_event_tools: self.expose_event_tools,
        }
    }
}

pub async fn run(args: McpServerArgs) -> Result<(), BoxErr> {
    mcpstore::mcp_server::run(args.to_core_options()).await
}
