use clap::{Parser, Subcommand};

use crate::{bootstrap, commands, BoxErr};

#[derive(Parser)]
#[command(
    name = "mcpstore",
    about = "MCPStore - unified CLI for managing/starting/configuring MCP services",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Version,
    Start(commands::daemon_cmd::StartArgs),
    Stop,
    Api(commands::api::ApiArgs),
    Config {
        #[command(subcommand)]
        action: commands::config::ConfigAction,
    },
    Add(commands::mcp::AddArgs),
    AddJson(commands::mcp::AddJsonArgs),
    Assign(commands::mcp::AssignArgs),
    Unassign(commands::mcp::UnassignArgs),
    List(commands::mcp::ListArgs),
    Get(commands::mcp::GetArgs),
    Remove(commands::mcp::RemoveArgs),
    Connect(commands::mcp::ConnectArgs),
    Disconnect(commands::mcp::DisconnectArgs),
    Restart(commands::mcp::RestartArgs),
    Check(commands::mcp::CheckArgs),
    Wait(commands::mcp::WaitArgs),
    Update(commands::mcp::UpdateArgs),
    Tools(commands::mcp::ToolsArgs),
    Call(commands::mcp::CallToolArgs),
    MigrateBackend(commands::mcp::MigrateBackendArgs),
    #[command(name = "mcp-server", visible_alias = "serve-mcp")]
    McpServer(commands::mcp_server::McpServerArgs),
    #[command(visible_alias = "ui")]
    Web(commands::web::WebArgs),
}

pub fn run() -> Result<(), BoxErr> {
    bootstrap::init_tracing("mcpstore=info");

    let cli = Cli::parse();
    let rt = bootstrap::build_runtime()?;

    rt.block_on(async {
        match cli.command {
            Commands::Version => {
                print_banner();
                Ok(())
            }
            Commands::Start(args) => commands::daemon_cmd::start(args).await,
            Commands::Stop => commands::daemon_cmd::stop().await,
            Commands::Api(args) => commands::api::run(args).await,
            Commands::Config { action } => commands::config::run(action).await,
            Commands::Add(args) => commands::mcp::add(args).await,
            Commands::AddJson(args) => commands::mcp::add_json(args).await,
            Commands::Assign(args) => commands::mcp::assign(args).await,
            Commands::Unassign(args) => commands::mcp::unassign(args).await,
            Commands::List(args) => commands::mcp::list(args).await,
            Commands::Get(args) => commands::mcp::get(args).await,
            Commands::Remove(args) => commands::mcp::remove(args).await,
            Commands::Connect(args) => commands::mcp::connect(args).await,
            Commands::Disconnect(args) => commands::mcp::disconnect(args).await,
            Commands::Restart(args) => commands::mcp::restart(args).await,
            Commands::Check(args) => commands::mcp::check(args).await,
            Commands::Wait(args) => commands::mcp::wait(args).await,
            Commands::Update(args) => commands::mcp::update(args).await,
            Commands::Tools(args) => commands::mcp::tools(args).await,
            Commands::Call(args) => commands::mcp::call_tool(args).await,
            Commands::MigrateBackend(args) => commands::mcp::migrate_backend(args).await,
            Commands::McpServer(args) => commands::mcp_server::run(args).await,
            Commands::Web(args) => commands::web::run(args).await,
        }
    })
}

pub fn print_banner() {
    println!(
        r#"
    ███    ███  ██████  ███████  ██████  ████████  ██████  ██████  ███████
    ████  ████ ██      ██    ██ ██          ██    ██    ██ ██   ██ ██
    ██ ████ ██ ██      ███████  ██████      ██    ██    ██ ██████  █████
    ██  ██  ██ ██      ██           ██      ██    ██    ██ ██  ██  ██
    ██      ██  ██████ ██      ██████       ██     ██████  ██   ██ ███████
    "#
    );
    println!("MCPStore version: {} (Rust)", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_add_with_agent_scope_and_header() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "add",
            "github",
            "https://api.example.com/mcp",
            "--transport",
            "http",
            "--scope",
            "agent",
            "--agent",
            "agent1",
            "--header",
            "Authorization=Bearer token",
            "--env",
            "LOCAL_TOKEN=abc",
        ])
        .unwrap();

        match cli.command {
            Commands::Add(args) => {
                assert_eq!(args.name, "github");
                assert_eq!(
                    args.command_or_url.as_deref(),
                    Some("https://api.example.com/mcp")
                );
                assert_eq!(args.scope, commands::mcp::Scope::Agent);
                assert_eq!(args.agent.as_deref(), Some("agent1"));
                assert_eq!(args.header, vec!["Authorization=Bearer token"]);
                assert_eq!(args.env, vec!["LOCAL_TOKEN=abc"]);
            }
            _ => panic!("Expected to parse as add command"),
        }
    }

    #[test]
    fn parses_stdio_command_after_separator() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "add",
            "filesystem",
            "--transport",
            "stdio",
            "--",
            "npx",
            "-y",
            "@modelcontextprotocol/server-filesystem",
            ".",
        ])
        .unwrap();

        match cli.command {
            Commands::Add(args) => {
                assert_eq!(args.name, "filesystem");
                assert_eq!(args.command_or_url.as_deref(), Some("npx"));
                assert_eq!(
                    args.args,
                    vec!["-y", "@modelcontextprotocol/server-filesystem", "."]
                );
            }
            _ => panic!("Expected to parse as add command"),
        }
    }

    #[test]
    fn parses_top_level_assign() {
        let cli =
            Cli::try_parse_from(["mcpstore", "assign", "github", "--agent", "agent1"]).unwrap();

        match cli.command {
            Commands::Assign(args) => {
                assert_eq!(args.service_name, "github");
                assert_eq!(args.agent, "agent1");
            }
            _ => panic!("Expected to parse as assign command"),
        }
    }

    #[test]
    fn parses_get_with_agent_scope() {
        let cli = Cli::try_parse_from([
            "mcpstore", "get", "github", "--scope", "agent", "--agent", "agent1",
        ])
        .unwrap();

        match cli.command {
            Commands::Get(args) => {
                assert_eq!(args.name, "github");
                assert_eq!(args.scope, commands::mcp::Scope::Agent);
                assert_eq!(args.agent.as_deref(), Some("agent1"));
            }
            _ => panic!("Expected to parse as get command"),
        }
    }

    #[test]
    fn parses_call_command() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "call",
            "gitodo",
            "get_repo_status",
            "--arguments",
            "{}",
        ])
        .unwrap();

        match cli.command {
            Commands::Call(args) => {
                assert_eq!(args.service_name, "gitodo");
                assert_eq!(args.tool_name, "get_repo_status");
                assert_eq!(args.arguments, "{}");
            }
            _ => panic!("Expected to parse as call command"),
        }
    }

    #[test]
    fn parses_mcp_server_command() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "mcp-server",
            "--scope",
            "agent",
            "--agent",
            "agent1",
        ])
        .unwrap();

        match cli.command {
            Commands::McpServer(args) => {
                assert_eq!(args.scope, commands::mcp::Scope::Agent);
                assert_eq!(args.agent.as_deref(), Some("agent1"));
            }
            _ => panic!("Expected to parse as mcp-server command"),
        }
    }

    #[test]
    fn rejects_legacy_call_tool_command() {
        match Cli::try_parse_from(["mcpstore", "call-tool", "gitodo", "get_repo_status"]) {
            Ok(_) => panic!("Legacy call-tool command should no longer be accepted"),
            Err(err) => assert!(err.to_string().contains("unrecognized subcommand")),
        }
    }

    #[test]
    fn rejects_legacy_agent_command() {
        match Cli::try_parse_from(["mcpstore", "agent", "assign", "agent1", "github"]) {
            Ok(_) => panic!("Legacy agent command should no longer be accepted"),
            Err(err) => assert!(err.to_string().contains("unrecognized subcommand")),
        }
    }

    #[test]
    fn parses_db_redis_source_flags() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "list",
            "--source",
            "db",
            "--backend",
            "redis",
            "--redis-url",
            "redis://127.0.0.1:6379/0",
            "--namespace",
            "demo",
        ])
        .unwrap();

        match cli.command {
            Commands::List(args) => {
                assert_eq!(args.store.source, crate::store_args::SourceArg::Db);
                assert_eq!(
                    args.store.backend,
                    Some(crate::store_args::BackendArg::Redis)
                );
                assert_eq!(
                    args.store.redis_url.as_deref(),
                    Some("redis://127.0.0.1:6379/0")
                );
                assert_eq!(args.store.namespace.as_deref(), Some("demo"));
            }
            _ => panic!("Expected to parse as list command"),
        }
    }

    #[test]
    fn parses_migrate_backend_command() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "migrate-backend",
            "--source",
            "local",
            "--target-backend",
            "redis",
            "--target-redis-url",
            "redis://127.0.0.1:6379/0",
        ])
        .unwrap();

        match cli.command {
            Commands::MigrateBackend(args) => {
                assert_eq!(args.store.source, crate::store_args::SourceArg::Local);
                assert_eq!(args.target_backend, crate::store_args::BackendArg::Redis);
                assert_eq!(
                    args.target_redis_url.as_deref(),
                    Some("redis://127.0.0.1:6379/0")
                );
            }
            _ => panic!("Expected to parse as migrate-backend command"),
        }
    }

    #[test]
    fn parses_web_command() {
        let cli = Cli::try_parse_from(["mcpstore", "web", "--port", "9090"]).unwrap();

        match cli.command {
            Commands::Web(args) => assert_eq!(args.port, 9090),
            _ => panic!("Expected to parse as web command"),
        }
    }

    #[test]
    fn parses_api_command() {
        let cli =
            Cli::try_parse_from(["mcpstore", "api", "--port", "9091", "--url-prefix", "/mcp"])
                .unwrap();

        match cli.command {
            Commands::Api(args) => {
                assert_eq!(args.port, 9091);
                assert_eq!(args.url_prefix, "/mcp");
            }
            _ => panic!("Expected to parse as api command"),
        }
    }
}
