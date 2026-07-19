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
    Auth(commands::auth::AuthArgs),
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
    Task(commands::task::TaskArgs),
    Resource(commands::protocol::ResourceArgs),
    Prompt(commands::protocol::PromptArgs),
    Complete(commands::protocol::CompleteArgs),
    MigrateBackend(commands::mcp::MigrateBackendArgs),
    #[command(name = "mcp-server", visible_alias = "serve-mcp")]
    McpServer(commands::mcp_server::McpServerArgs),
    #[command(visible_alias = "ui")]
    Web(commands::web::WebArgs),
    Tui(crate::tui::TuiArgs),
}

pub fn run() -> Result<(), BoxErr> {
    let cli = Cli::parse();

    // TUI runs its own blocking event loop and creates its own runtime,
    // so it must be handled outside the async block to avoid nested runtimes.
    if let Commands::Tui(args) = cli.command {
        return crate::tui::run_from_args(&args);
    }

    if uses_machine_output(&cli.command) {
        bootstrap::init_tracing_silent("mcpstore=info");
    } else {
        bootstrap::init_tracing("mcpstore=info");
    }

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
            Commands::Auth(args) => commands::auth::run(args).await,
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
            Commands::Task(args) => commands::task::run(args).await,
            Commands::Resource(args) => commands::protocol::run_resource(args).await,
            Commands::Prompt(args) => commands::protocol::run_prompt(args).await,
            Commands::Complete(args) => commands::protocol::complete(args).await,
            Commands::MigrateBackend(args) => commands::mcp::migrate_backend(args).await,
            Commands::McpServer(args) => commands::mcp_server::run(args).await,
            Commands::Web(args) => commands::web::run(args).await,
            Commands::Tui(_) => unreachable!("Tui command handled before async block"),
        }
    })
}

fn uses_machine_output(command: &Commands) -> bool {
    match command {
        Commands::Call(args) => args.output != commands::mcp::CallOutputFormat::Human,
        Commands::Task(args) => {
            let output = match &args.action {
                commands::task::TaskAction::Run(args) => args.runtime.output,
                commands::task::TaskAction::List(args) => args.runtime.output,
                commands::task::TaskAction::Status(args)
                | commands::task::TaskAction::Result(args)
                | commands::task::TaskAction::Cancel(args) => args.runtime.output,
            };
            output != commands::task::TaskOutputFormat::Human
        }
        Commands::Resource(args) => match &args.action {
            commands::protocol::ResourceAction::List(args) => {
                args.output.output != commands::protocol::ProtocolOutputFormat::Human
            }
            commands::protocol::ResourceAction::Templates(args) => {
                args.output.output != commands::protocol::ProtocolOutputFormat::Human
            }
            commands::protocol::ResourceAction::Read(args) => {
                args.output.output != commands::protocol::ProtocolOutputFormat::Human
            }
        },
        Commands::Prompt(args) => match &args.action {
            commands::protocol::PromptAction::List(args) => {
                args.output.output != commands::protocol::ProtocolOutputFormat::Human
            }
            commands::protocol::PromptAction::Get(args) => {
                args.output.output != commands::protocol::ProtocolOutputFormat::Human
            }
        },
        Commands::Complete(args) => {
            args.output.output != commands::protocol::ProtocolOutputFormat::Human
        }
        _ => false,
    }
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
    fn parses_auth_status_json_output() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "auth",
            "status",
            "c81af510-755b-55c7-8487-5668ab36e06e",
            "--output",
            "json",
        ])
        .unwrap();

        match cli.command {
            Commands::Auth(commands::auth::AuthArgs {
                action: commands::auth::AuthAction::Status(args),
            }) => {
                assert_eq!(args.output.output, commands::auth::OutputFormat::Json);
            }
            _ => panic!("Expected auth status command"),
        }
    }

    #[test]
    fn parses_auth_login_non_interactive_json_output() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "auth",
            "login",
            "c81af510-755b-55c7-8487-5668ab36e06e",
            "--non-interactive",
            "--output",
            "json",
            "--timeout",
            "17",
        ])
        .unwrap();

        match cli.command {
            Commands::Auth(commands::auth::AuthArgs {
                action: commands::auth::AuthAction::Login(args),
            }) => {
                assert!(args.flow_output.non_interactive);
                assert_eq!(
                    args.flow_output.output.output,
                    commands::auth::OutputFormat::Json
                );
                assert_eq!(args.timeout, 17);
            }
            _ => panic!("Expected auth login command"),
        }
    }

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
    fn parses_get_with_instance_id() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d";
        let cli = Cli::try_parse_from(["mcpstore", "get", instance_id]).unwrap();

        match cli.command {
            Commands::Get(args) => {
                assert_eq!(args.instance_id, instance_id);
            }
            _ => panic!("Expected to parse as get command"),
        }
    }

    #[test]
    fn parses_check_with_instance_id() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d";
        let cli = Cli::try_parse_from(["mcpstore", "check", instance_id]).unwrap();

        match cli.command {
            Commands::Check(args) => assert_eq!(args.instance_id, instance_id),
            _ => panic!("Expected to parse as check command"),
        }
    }

    #[test]
    fn rejects_check_without_instance_id() {
        assert!(Cli::try_parse_from(["mcpstore", "check"]).is_err());
    }

    #[test]
    fn parses_call_command() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "call",
            "c81af510-755b-55c7-8487-5668ab36e06e",
            "get_repo_status",
            "--arguments",
            "{}",
            "--output",
            "jsonl",
            "--timeout",
            "15",
            "--max-total-timeout",
            "60",
            "--non-interactive",
        ])
        .unwrap();

        match cli.command {
            Commands::Call(args) => {
                assert_eq!(args.instance_id, "c81af510-755b-55c7-8487-5668ab36e06e");
                assert_eq!(args.tool_name, "get_repo_status");
                assert_eq!(args.arguments, "{}");
                assert_eq!(args.output, commands::mcp::CallOutputFormat::Jsonl);
                assert_eq!(args.timeout, Some(15));
                assert_eq!(args.max_total_timeout, Some(60));
                assert!(args.non_interactive);
            }
            _ => panic!("Expected to parse as call command"),
        }
    }

    #[test]
    fn machine_output_commands_use_silent_tracing() {
        let call = Cli::try_parse_from([
            "mcpstore",
            "call",
            "c81af510-755b-55c7-8487-5668ab36e06e",
            "get_repo_status",
            "--output",
            "jsonl",
        ])
        .unwrap();
        assert!(uses_machine_output(&call.command));

        let task = Cli::try_parse_from([
            "mcpstore",
            "task",
            "list",
            "127ce370-1ed6-5b00-9713-e88d01b3010d",
            "--output",
            "json",
        ])
        .unwrap();
        assert!(uses_machine_output(&task.command));

        let human = Cli::try_parse_from([
            "mcpstore",
            "call",
            "c81af510-755b-55c7-8487-5668ab36e06e",
            "get_repo_status",
        ])
        .unwrap();
        assert!(!uses_machine_output(&human.command));
    }

    #[test]
    fn parses_task_run_with_jsonl_output() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d";
        let cli = Cli::try_parse_from([
            "mcpstore",
            "task",
            "run",
            instance_id,
            "long_tool",
            "--input",
            r#"{"value":1}"#,
            "--ttl",
            "5000",
            "--timeout",
            "20",
            "--max-total-timeout",
            "90",
            "--output",
            "jsonl",
            "--non-interactive",
        ])
        .unwrap();

        match cli.command {
            Commands::Task(commands::task::TaskArgs {
                action: commands::task::TaskAction::Run(args),
            }) => {
                assert_eq!(args.instance_id.to_string(), instance_id);
                assert_eq!(args.tool_name, "long_tool");
                assert_eq!(args.input, r#"{"value":1}"#);
                assert_eq!(args.ttl, Some(5000));
                assert_eq!(args.timeout, Some(20));
                assert_eq!(args.max_total_timeout, Some(90));
                assert_eq!(args.runtime.output, commands::task::TaskOutputFormat::Jsonl);
                assert!(args.runtime.non_interactive);
            }
            _ => panic!("Expected to parse as task run command"),
        }
    }

    #[test]
    fn parses_task_list_with_json_output() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d";
        let cli =
            Cli::try_parse_from(["mcpstore", "task", "list", instance_id, "--output", "json"])
                .unwrap();

        match cli.command {
            Commands::Task(commands::task::TaskArgs {
                action: commands::task::TaskAction::List(args),
            }) => {
                assert_eq!(args.instance_id.to_string(), instance_id);
                assert_eq!(args.runtime.output, commands::task::TaskOutputFormat::Json);
            }
            _ => panic!("Expected to parse as task list command"),
        }
    }

    #[test]
    fn parses_task_status_result_and_cancel_targets() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d";
        for action in ["status", "result", "cancel"] {
            let cli = Cli::try_parse_from([
                "mcpstore",
                "task",
                action,
                instance_id,
                "task-1",
                "--output",
                "jsonl",
            ])
            .unwrap();

            let target = match cli.command {
                Commands::Task(commands::task::TaskArgs {
                    action: commands::task::TaskAction::Status(args),
                })
                | Commands::Task(commands::task::TaskArgs {
                    action: commands::task::TaskAction::Result(args),
                })
                | Commands::Task(commands::task::TaskArgs {
                    action: commands::task::TaskAction::Cancel(args),
                }) => args,
                _ => panic!("Expected to parse as task target command"),
            };
            assert_eq!(target.instance_id.to_string(), instance_id);
            assert_eq!(target.task_id, "task-1");
            assert_eq!(
                target.runtime.output,
                commands::task::TaskOutputFormat::Jsonl
            );
        }
    }

    #[test]
    fn rejects_task_commands_missing_required_targets() {
        assert!(Cli::try_parse_from(["mcpstore", "task", "run"]).is_err());
        assert!(Cli::try_parse_from(["mcpstore", "task", "status"]).is_err());
        assert!(Cli::try_parse_from([
            "mcpstore",
            "task",
            "result",
            "127ce370-1ed6-5b00-9713-e88d01b3010d",
        ])
        .is_err());
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
                    Some(crate::store_args::CacheStorageArg::Redis)
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
                assert_eq!(
                    args.target_cache_storage,
                    crate::store_args::CacheStorageArg::Redis
                );
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
    fn parses_auth_login_with_local_callback_timeout() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d";
        let cli =
            Cli::try_parse_from(["mcpstore", "auth", "login", instance_id, "--timeout", "120"])
                .unwrap();

        match cli.command {
            Commands::Auth(commands::auth::AuthArgs {
                action: commands::auth::AuthAction::Login(args),
            }) => {
                assert_eq!(args.instance_id.to_string(), instance_id);
                assert_eq!(args.timeout, 120);
            }
            _ => panic!("Expected to parse as auth login command"),
        }
    }

    #[test]
    fn auth_client_secret_is_read_from_stdin_not_command_line() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d";
        assert!(Cli::try_parse_from([
            "mcpstore",
            "auth",
            "set-client-secret",
            instance_id,
            "secret-value",
        ])
        .is_err());
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
                assert!(!args.allow_remote);
            }
            _ => panic!("Expected to parse as api command"),
        }
    }
    #[test]
    fn parses_resource_read_json_output() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "resource",
            "read",
            "c81af510-755b-55c7-8487-5668ab36e06e",
            "repo://mcp/store",
            "--output",
            "json",
        ])
        .unwrap();

        match cli.command {
            Commands::Resource(commands::protocol::ResourceArgs {
                action: commands::protocol::ResourceAction::Read(args),
            }) => {
                assert_eq!(args.instance_id, "c81af510-755b-55c7-8487-5668ab36e06e");
                assert_eq!(args.uri, "repo://mcp/store");
                assert_eq!(
                    args.output.output,
                    commands::protocol::ProtocolOutputFormat::Json
                );
            }
            _ => panic!("Expected resource read command"),
        }
    }

    #[test]
    fn parses_prompt_get_with_arguments() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "prompt",
            "get",
            "c81af510-755b-55c7-8487-5668ab36e06e",
            "review",
            "--arguments",
            r#"{"style":"brief"}"#,
        ])
        .unwrap();

        match cli.command {
            Commands::Prompt(commands::protocol::PromptArgs {
                action: commands::protocol::PromptAction::Get(args),
            }) => {
                assert_eq!(args.prompt_name, "review");
                assert_eq!(args.arguments, r#"{"style":"brief"}"#);
            }
            _ => panic!("Expected prompt get command"),
        }
    }

    #[test]
    fn parses_complete_command() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "complete",
            "c81af510-755b-55c7-8487-5668ab36e06e",
            "--reference-kind",
            "resource",
            "--reference",
            "repo://mcp/{name}",
            "--argument-name",
            "name",
            "--value",
            "re",
            "--context",
            r#"{"other":"x"}"#,
            "--output",
            "jsonl",
        ])
        .unwrap();

        match cli.command {
            Commands::Complete(args) => {
                assert_eq!(
                    args.reference_kind,
                    commands::protocol::CompletionReferenceKind::Resource
                );
                assert_eq!(args.reference, "repo://mcp/{name}");
                assert_eq!(
                    args.output.output,
                    commands::protocol::ProtocolOutputFormat::Jsonl
                );
            }
            _ => panic!("Expected complete command"),
        }
    }
}
