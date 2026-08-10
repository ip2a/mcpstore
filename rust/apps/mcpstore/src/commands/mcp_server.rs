use clap::{Args, ValueEnum};
use mcpstore::{ConfigManager, InstanceId, JsonStoreConfig, ScopeRef, SourceMode};

use crate::mcp_server::{
    McpServerOptions as CoreMcpServerOptions, McpServerTransport as CoreMcpServerTransport,
};

use crate::{commands::mcp::Scope, store_args::StoreSourceArgs, BoxErr};

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
    #[arg(
        long,
        visible_alias = "agent-id",
        required_if_eq("scope", "agent"),
        help = "Agent ID, required with --scope agent"
    )]
    pub agent: Option<String>,
    #[arg(
        long,
        conflicts_with = "session_key",
        help = "Optional service instance ID to expose"
    )]
    pub instance_id: Option<InstanceId>,
    #[arg(
        long,
        value_enum,
        help = "MCP transport: stdio or streamable-http; defaults to app config"
    )]
    pub transport: Option<McpServerTransport>,
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "绑定地址，仅 streamable-http 使用"
    )]
    pub host: String,
    #[arg(
        long,
        help = "监听端口，仅 streamable-http 使用；默认读取 mcp_aggregate.port"
    )]
    pub port: Option<u16>,
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
        help = "Expose MCPStore tool override management tools. Disabled by default."
    )]
    pub expose_tool_override_tools: bool,
    #[arg(
        long,
        help = "Expose MCPStore prompt override management tools. Disabled by default."
    )]
    pub expose_prompt_override_tools: bool,
    #[arg(
        long,
        help = "Expose MCPStore resource and resource-template override management tools. Disabled by default."
    )]
    pub expose_resource_override_tools: bool,
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
        help = "Expose MCPStore cache management tools. Disabled by default."
    )]
    pub expose_cache_tools: bool,
    #[arg(
        long,
        help = "Expose the mcpstore_search_tools BM25 meta-tool for searching the visible tool catalog. Disabled by default."
    )]
    pub expose_search_tools: bool,
}

impl McpServerArgs {
    fn to_core_options(
        &self,
        app_config: &mcpstore::AppConfig,
    ) -> Result<CoreMcpServerOptions, BoxErr> {
        let scope = match (&self.scope, self.agent.as_deref()) {
            (Scope::Store, None) => ScopeRef::Store,
            (Scope::Store, Some(_)) => {
                return Err("--agent is only valid with --scope agent".into())
            }
            (Scope::Agent, Some(agent_id)) if !agent_id.trim().is_empty() => ScopeRef::Agent {
                agent_id: agent_id.to_string(),
            },
            (Scope::Agent, _) => return Err("--agent is required with --scope agent".into()),
        };
        if self.instance_id.is_some() && self.session_key.is_some() {
            return Err("--instance-id cannot be combined with --session-key".into());
        }

        let transport = match self.transport {
            Some(transport) => transport,
            None => match app_config.mcp_aggregate.transport.as_str() {
                "stdio" => McpServerTransport::Stdio,
                "streamable-http" => McpServerTransport::StreamableHttp,
                value => return Err(format!("invalid mcp_aggregate.transport: {value}").into()),
            },
        };
        let port = self.port.unwrap_or(app_config.mcp_aggregate.port);

        let store = self
            .store
            .store
            .as_ref()
            .map(|store| JsonStoreConfig::new(store, serde_json::json!({})))
            .or_else(|| {
                self.store.store_config.as_ref().map(|config| {
                    JsonStoreConfig::new("redis", serde_json::json!({"config": config}))
                })
            });
        Ok(CoreMcpServerOptions {
            config_path: self.store.config_path.clone(),
            source_mode: match self.store.source {
                crate::store_args::SourceArg::Local => SourceMode::Local,
                crate::store_args::SourceArg::Db => SourceMode::Db,
            },
            store,
            namespace: self.store.namespace.clone(),
            scope,
            instance_id: self.instance_id,
            transport: transport.to_core(),
            host: self.host.clone(),
            port,
            path: self.path.clone(),
            session_key: self.session_key.clone(),
            expose_session_state_tools: self.expose_session_state_tools,
            expose_tool_override_tools: self.expose_tool_override_tools,
            expose_prompt_override_tools: self.expose_prompt_override_tools,
            expose_resource_override_tools: self.expose_resource_override_tools,
            expose_openapi_tools: self.expose_openapi_tools,
            expose_service_tools: self.expose_service_tools,
            expose_cache_tools: self.expose_cache_tools,
            expose_search_tools: self.expose_search_tools,
        })
    }
}

pub async fn run(args: McpServerArgs) -> Result<(), BoxErr> {
    let config_manager = match args.store.config_path.as_deref() {
        Some(path) => ConfigManager::with_path(path),
        None => ConfigManager::new(),
    };
    let app_config = config_manager.load_app_config_or_default()?;
    crate::mcp_server::run(args.to_core_options(&app_config)?).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store_args::SourceArg;

    fn default_args() -> McpServerArgs {
        McpServerArgs {
            store: StoreSourceArgs {
                config_path: None,
                source: SourceArg::Local,
                store: None,
                store_config: None,
                namespace: None,
            },
            scope: Scope::Store,
            agent: None,
            instance_id: None,
            transport: None,
            host: "127.0.0.1".to_string(),
            port: None,
            path: "/mcp".to_string(),
            session_key: None,
            expose_session_state_tools: false,
            expose_tool_override_tools: false,
            expose_prompt_override_tools: false,
            expose_resource_override_tools: false,
            expose_openapi_tools: false,
            expose_service_tools: false,
            expose_cache_tools: false,
            expose_search_tools: false,
        }
    }

    #[test]
    fn app_config_controls_mcp_defaults_and_cli_values_override_it() {
        let mut config = mcpstore::AppConfig::default();
        config.mcp_aggregate.transport = "streamable-http".to_string();
        config.mcp_aggregate.port = 19400;

        let args = default_args();
        let options = args.to_core_options(&config).unwrap();
        assert_eq!(options.transport, CoreMcpServerTransport::StreamableHttp);
        assert_eq!(options.port, 19400);

        let mut args = default_args();
        args.transport = Some(McpServerTransport::Stdio);
        args.port = Some(19500);
        let options = args.to_core_options(&config).unwrap();
        assert_eq!(options.transport, CoreMcpServerTransport::Stdio);
        assert_eq!(options.port, 19500);
    }
}
