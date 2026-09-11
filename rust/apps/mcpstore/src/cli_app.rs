use clap::{Parser, Subcommand};
use mcpstore::ConfigManager;

use crate::error::OutputFormat;
use crate::{bootstrap, commands, BoxErr};

#[derive(Parser)]
#[command(
    name = "mcpstore",
    about = "MCPStore - unified CLI for managing/starting/configuring MCP services",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct Cli {
    /// 本进程内嵌 kernel 冷启动，不连也不拉 daemon
    #[arg(long, global = true)]
    pub embedded: bool,
    /// Remote daemon Kernel RPC endpoint, e.g. 10.0.0.2:1840
    #[arg(long, global = true)]
    pub daemon_endpoint: Option<String>,
    /// Namespace on --daemon-endpoint
    #[arg(long, global = true)]
    pub daemon_namespace: Option<String>,
    /// Shared token for --daemon-endpoint
    #[arg(long, global = true)]
    pub daemon_token: Option<String>,
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    fn daemon_endpoint(&self) -> Option<crate::daemon::client::DaemonEndpoint> {
        crate::daemon::client::DaemonEndpoint::from_args(
            self.daemon_endpoint.clone(),
            self.daemon_namespace.clone(),
            self.daemon_token.clone(),
        )
    }
}

#[derive(Subcommand)]
pub enum Commands {
    Version,
    Start(commands::daemon_cmd::StartArgs),
    Stop,
    Status {
        #[arg(long)]
        json: bool,
    },
    Daemon {
        #[command(subcommand)]
        action: commands::daemon_cmd::DaemonAction,
    },
    Api {
        #[arg(long)]
        json: bool,
    },
    Auth(commands::auth::AuthArgs),
    Config {
        #[command(subcommand)]
        action: Option<commands::config::ConfigAction>,
        #[command(flatten)]
        edits: commands::config::ConfigEdits,
        #[arg(long)]
        json: bool,
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
    Request(commands::request::RequestArgs),
    MigrateStore(commands::mcp::MigrateStoreArgs),
    #[command(name = "mcp")]
    McpServer(commands::mcp_server::McpServerArgs),
    #[command(visible_alias = "ui")]
    Web {
        #[arg(long)]
        json: bool,
    },
    Tui(crate::tui::TuiArgs),
}

pub fn run() -> Result<(), BoxErr> {
    let cli = Cli::parse();

    // TUI runs its own blocking event loop and creates its own runtime,
    // so it must be handled outside the async block to avoid nested runtimes.
    if let Commands::Tui(mut args) = cli.command {
        args.embedded = args.embedded || cli.embedded;
        return crate::tui::run_from_args(&args);
    }

    let app_config = ConfigManager::new().load_app_config_or_default().ok();
    let output_format = output_format(&cli.command);
    if output_format.is_machine() {
        bootstrap::init_tracing_silent("mcpstore=info");
    } else {
        bootstrap::init_tracing_from_config(app_config.as_ref());
    }

    let endpoint = cli.daemon_endpoint();
    let embedded = cli.embedded;
    let rt = bootstrap::build_runtime()?;

    let result = rt.block_on(async move {
        match cli.command {
            Commands::Version => {
                print_banner();
                Ok(())
            }
            Commands::Start(args) => commands::daemon_cmd::start(args).await,
            Commands::Stop => commands::daemon_cmd::stop().await,
            Commands::Status { json } => commands::daemon_cmd::status(json).await,
            Commands::Daemon { action } => commands::daemon_cmd::run_daemon(action).await,
            Commands::Api { json } => commands::daemon_cmd::face_view("core", json).await,
            Commands::Auth(args) => commands::auth::run(args, embedded, endpoint.clone()).await,
            Commands::Config {
                action,
                edits,
                json,
            } => commands::config::run(action, edits, json).await,
            Commands::Add(args) => commands::mcp::add(args, embedded, endpoint.clone()).await,
            Commands::AddJson(args) => {
                commands::mcp::add_json(args, embedded, endpoint.clone()).await
            }
            Commands::Assign(args) => commands::mcp::assign(args, embedded, endpoint.clone()).await,
            Commands::Unassign(args) => {
                commands::mcp::unassign(args, embedded, endpoint.clone()).await
            }
            Commands::List(args) => commands::mcp::list(args, embedded, endpoint.clone()).await,
            Commands::Get(args) => commands::mcp::get(args, embedded, endpoint.clone()).await,
            Commands::Remove(args) => commands::mcp::remove(args, embedded, endpoint.clone()).await,
            Commands::Connect(args) => {
                commands::mcp::connect(args, embedded, endpoint.clone()).await
            }
            Commands::Disconnect(args) => {
                commands::mcp::disconnect(args, embedded, endpoint.clone()).await
            }
            Commands::Restart(args) => {
                commands::mcp::restart(args, embedded, endpoint.clone()).await
            }
            Commands::Check(args) => commands::mcp::check(args, embedded, endpoint.clone()).await,
            Commands::Wait(args) => commands::mcp::wait(args, embedded, endpoint.clone()).await,
            Commands::Update(args) => commands::mcp::update(args, embedded, endpoint.clone()).await,
            Commands::Tools(args) => commands::mcp::tools(args, embedded, endpoint.clone()).await,
            Commands::Call(ref args) => {
                let endpoint = execution_endpoint(
                    endpoint.clone(),
                    app_config.as_ref(),
                    args.execution.daemon_node.as_deref(),
                )?;
                commands::mcp::call_tool(args.clone(), embedded, endpoint).await
            }
            Commands::Task(args) => {
                let endpoint = task_endpoint(app_config.as_ref(), &args, endpoint.clone())?;
                commands::task::run(args, embedded, endpoint).await
            }
            Commands::Resource(args) => {
                let endpoint = protocol_endpoint(app_config.as_ref(), &args, endpoint.clone())?;
                commands::protocol::run_resource(args, embedded, endpoint).await
            }
            Commands::Prompt(args) => {
                let endpoint = protocol_endpoint(app_config.as_ref(), &args, endpoint.clone())?;
                commands::protocol::run_prompt(args, embedded, endpoint).await
            }
            Commands::Complete(ref args) => {
                let endpoint = execution_endpoint(
                    endpoint.clone(),
                    app_config.as_ref(),
                    args.execution.daemon_node.as_deref(),
                )?;
                commands::protocol::complete(args.clone(), embedded, endpoint).await
            }
            Commands::Request(args) => {
                commands::request::run(args, embedded, endpoint.clone()).await
            }
            Commands::MigrateStore(args) => {
                commands::mcp::migrate_store(args, embedded, endpoint.clone()).await
            }
            Commands::McpServer(args) => commands::mcp_server::run(args).await,
            Commands::Web { json } => commands::daemon_cmd::face_view("web", json).await,
            Commands::Tui(_) => unreachable!("Tui command handled before async block"),
        }
    });

    if let Err(error) = &result {
        if let Some(error) = error.downcast_ref::<mcpstore::Error>() {
            eprintln!("{}", crate::error::render(error, output_format));
            std::process::exit(crate::error::exit_code(error.code()));
        }
    }
    result
}

fn execution_endpoint(
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
    app_config: Option<&mcpstore::AppConfig>,
    daemon_node: Option<&str>,
) -> mcpstore::Result<Option<crate::daemon::client::DaemonEndpoint>> {
    if endpoint.is_some() {
        return Ok(endpoint);
    }
    let Some(node_id) = daemon_node else {
        return Ok(None);
    };
    let endpoint = app_config
        .and_then(|config| config.daemons.get(node_id))
        .map(crate::daemon::client::DaemonEndpoint::from_node);
    if let Some(endpoint) = endpoint {
        return Ok(Some(endpoint));
    }
    Err(mcpstore::Error::new(
        mcpstore::FailureCode::ConfigInvalid,
        format!("unknown execution node '{node_id}'"),
    ))
}

fn task_endpoint(
    app_config: Option<&mcpstore::AppConfig>,
    args: &commands::task::TaskArgs,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> mcpstore::Result<Option<crate::daemon::client::DaemonEndpoint>> {
    let daemon_node = match &args.action {
        commands::task::TaskAction::Run(args) => args.execution.daemon_node.as_deref(),
        _ => None,
    };
    execution_endpoint(endpoint, app_config, daemon_node)
}

fn protocol_endpoint<T: ProtocolExecutionArgs>(
    app_config: Option<&mcpstore::AppConfig>,
    args: &T,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> mcpstore::Result<Option<crate::daemon::client::DaemonEndpoint>> {
    execution_endpoint(endpoint, app_config, args.daemon_node())
}

trait ProtocolExecutionArgs {
    fn daemon_node(&self) -> Option<&str>;
}

impl ProtocolExecutionArgs for commands::protocol::ResourceArgs {
    fn daemon_node(&self) -> Option<&str> {
        match &self.action {
            commands::protocol::ResourceAction::List(args) => args.execution.daemon_node.as_deref(),
            commands::protocol::ResourceAction::Templates(args) => {
                args.execution.daemon_node.as_deref()
            }
            commands::protocol::ResourceAction::Read(args) => args.execution.daemon_node.as_deref(),
        }
    }
}

impl ProtocolExecutionArgs for commands::protocol::PromptArgs {
    fn daemon_node(&self) -> Option<&str> {
        match &self.action {
            commands::protocol::PromptAction::List(args) => args.execution.daemon_node.as_deref(),
            commands::protocol::PromptAction::Get(args) => args.execution.daemon_node.as_deref(),
        }
    }
}

fn output_format(command: &Commands) -> crate::error::OutputFormat {
    match command {
        Commands::Call(args) => args.output,
        Commands::Task(args) => match &args.action {
            commands::task::TaskAction::Run(args) => args.runtime.output,
            commands::task::TaskAction::List(args) => args.runtime.output,
            commands::task::TaskAction::Status(args)
            | commands::task::TaskAction::Result(args)
            | commands::task::TaskAction::Cancel(args) => args.runtime.output,
        },
        Commands::Resource(args) => match &args.action {
            commands::protocol::ResourceAction::List(args) => args.output.output,
            commands::protocol::ResourceAction::Templates(args) => args.output.output,
            commands::protocol::ResourceAction::Read(args) => args.output.output,
        },
        Commands::Prompt(args) => match &args.action {
            commands::protocol::PromptAction::List(args) => args.output.output,
            commands::protocol::PromptAction::Get(args) => args.output.output,
        },
        Commands::Complete(args) => args.output.output,
        Commands::Request(args) => match &args.action {
            commands::request::RequestAction::List(args) => args.output,
            commands::request::RequestAction::Get(args) => args.output,
            commands::request::RequestAction::Wait(args) => args.output,
        },
        Commands::List(args) => args.output,
        Commands::Tools(args) => args.output,
        Commands::Get(args) => args.output,
        Commands::Connect(args) => args.output,
        Commands::Disconnect(args) => args.output,
        Commands::Restart(args) => args.output,
        Commands::Check(args) => args.output,
        Commands::Wait(args) => args.output,
        _ => OutputFormat::Human,
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
    fn named_execution_node_resolves_endpoint_from_config() {
        let mut config = mcpstore::AppConfig::default();
        config.daemons.insert(
            "remote".into(),
            mcpstore::DaemonNodeSettings {
                endpoint: "127.0.0.1:1840".into(),
                namespace: Some("ns".into()),
                token: Some("secret".into()),
            },
        );
        let cli = Cli::try_parse_from([
            "mcpstore",
            "call",
            "svc",
            "noop",
            "--runtime",
            "daemon",
            "--daemon-node",
            "remote",
        ])
        .unwrap();
        let Commands::Call(args) = &cli.command else {
            panic!("expected call command");
        };
        let endpoint =
            execution_endpoint(None, Some(&config), args.execution.daemon_node.as_deref())
                .unwrap()
                .unwrap();
        assert_eq!(endpoint.address, "127.0.0.1:1840");
        assert_eq!(endpoint.namespace, "ns");
        assert_eq!(endpoint.token.as_deref(), Some("secret"));
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
                assert_eq!(args.target, instance_id);
            }
            _ => panic!("Expected to parse as get command"),
        }
    }

    #[test]
    fn parses_check_with_instance_id() {
        let instance_id = "127ce370-1ed6-5b00-9713-e88d01b3010d";
        let cli = Cli::try_parse_from(["mcpstore", "check", instance_id]).unwrap();

        match cli.command {
            Commands::Check(args) => assert_eq!(args.target, instance_id),
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
            Commands::Call(ref args) => {
                assert_eq!(args.target, "c81af510-755b-55c7-8487-5668ab36e06e");
                assert_eq!(args.tool_name, "get_repo_status");
                assert_eq!(args.arguments, "{}");
                assert_eq!(args.output, crate::error::OutputFormat::Jsonl);
                assert_eq!(args.timeout, Some(15));
                assert_eq!(args.max_total_timeout, Some(60));
                assert!(args.non_interactive);
            }
            _ => panic!("Expected to parse as call command"),
        }
    }

    #[test]
    fn parses_call_runtime_and_daemon_node() {
        for (value, expected) in [
            ("local", commands::mcp::RuntimeArg::Local),
            ("daemon", commands::mcp::RuntimeArg::Daemon),
        ] {
            let cli =
                Cli::try_parse_from(["mcpstore", "call", "service", "tool", "--runtime", value])
                    .unwrap();
            match cli.command {
                Commands::Call(args) => assert_eq!(args.execution.runtime, Some(expected)),
                _ => panic!("Expected to parse as call command"),
            }
        }

        let cli = Cli::try_parse_from([
            "mcpstore",
            "call",
            "service",
            "tool",
            "--runtime",
            "daemon",
            "--daemon-node",
            "browser-host",
        ])
        .unwrap();
        match cli.command {
            Commands::Call(args) => {
                assert_eq!(
                    args.execution.runtime,
                    Some(commands::mcp::RuntimeArg::Daemon)
                );
                assert_eq!(args.execution.daemon_node.as_deref(), Some("browser-host"));
            }
            _ => panic!("Expected to parse as call command"),
        }
    }

    #[test]
    fn parses_call_with_service_name_and_tool_args() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "call",
            "--scope",
            "agent",
            "--agent",
            "bot",
            "github",
            "get_repo",
            "owner:ip2a",
            "repo:mcp/store",
        ])
        .unwrap();

        match cli.command {
            Commands::Call(ref args) => {
                assert_eq!(args.target, "github");
                assert_eq!(args.tool_name, "get_repo");
                assert_eq!(args.args, vec!["owner:ip2a", "repo:mcp/store"]);
                assert_eq!(args.scope, commands::mcp::Scope::Agent);
                assert_eq!(args.agent.as_deref(), Some("bot"));
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
        assert!(output_format(&call.command).is_machine());

        let task = Cli::try_parse_from([
            "mcpstore",
            "task",
            "list",
            "127ce370-1ed6-5b00-9713-e88d01b3010d",
            "--output",
            "json",
        ])
        .unwrap();
        assert!(output_format(&task.command).is_machine());

        let list_json = Cli::try_parse_from(["mcpstore", "list", "--output", "json"]).unwrap();
        assert!(output_format(&list_json.command).is_machine());

        let tools_json = Cli::try_parse_from([
            "mcpstore",
            "tools",
            "127ce370-1ed6-5b00-9713-e88d01b3010d",
            "--output",
            "json",
        ])
        .unwrap();
        assert!(output_format(&tools_json.command).is_machine());

        let list_human = Cli::try_parse_from(["mcpstore", "list"]).unwrap();
        assert!(!output_format(&list_human.command).is_machine());

        let human = Cli::try_parse_from([
            "mcpstore",
            "call",
            "c81af510-755b-55c7-8487-5668ab36e06e",
            "get_repo_status",
        ])
        .unwrap();
        assert!(!output_format(&human.command).is_machine());
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
            "--timeout",
            "20",
            "--max-total-timeout",
            "90",
            "--output",
            "jsonl",
            "--non-interactive",
            "--runtime",
            "daemon",
        ])
        .unwrap();

        match cli.command {
            Commands::Task(commands::task::TaskArgs {
                action: commands::task::TaskAction::Run(args),
            }) => {
                assert_eq!(args.instance_id.to_string(), instance_id);
                assert_eq!(args.tool_name, "long_tool");
                assert_eq!(args.input, r#"{"value":1}"#);
                assert_eq!(args.timeout, Some(20));
                assert_eq!(args.max_total_timeout, Some(90));
                assert_eq!(args.runtime.output, crate::error::OutputFormat::Jsonl);
                assert!(args.runtime.non_interactive);
                assert_eq!(
                    args.execution.runtime,
                    Some(crate::commands::mcp::RuntimeArg::Daemon)
                );
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
                assert_eq!(args.runtime.output, crate::error::OutputFormat::Json);
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
            assert_eq!(target.runtime.output, crate::error::OutputFormat::Jsonl);
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
    fn parses_mcp_command() {
        let cli = Cli::try_parse_from(["mcpstore", "mcp", "--scope", "agent", "--agent", "agent1"])
            .unwrap();

        match cli.command {
            Commands::McpServer(args) => {
                assert_eq!(args.scope, commands::mcp::Scope::Agent);
                assert_eq!(args.agent.as_deref(), Some("agent1"));
                assert_eq!(args.transport, None);
                assert_eq!(args.port, None);
            }
            _ => panic!("Expected to parse as mcp command"),
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
            "--store",
            "redis",
            "--store-config",
            r#"{"url":"redis://127.0.0.1:6379/0"}"#,
            "--namespace",
            "demo",
        ])
        .unwrap();

        match cli.command {
            Commands::List(args) => {
                assert_eq!(args.store.source, crate::store_args::SourceArg::Db);
                assert_eq!(args.store.store.as_deref(), Some("redis"));
                assert_eq!(
                    args.store.store_config.as_deref(),
                    Some(r#"{"url":"redis://127.0.0.1:6379/0"}"#)
                );
                assert_eq!(args.store.namespace.as_deref(), Some("demo"));
            }
            _ => panic!("Expected to parse as list command"),
        }
    }

    #[test]
    fn parses_data_plane_node_mode() {
        let cli = Cli::try_parse_from([
            "mcpstore", "list", "--source", "db", "--store", "redis", "--plane", "data",
        ])
        .unwrap();

        match cli.command {
            Commands::List(args) => {
                assert_eq!(
                    args.store.node_mode,
                    Some(crate::store_args::NodeModeArg::Data)
                );
                assert!(args.store.is_explicit());
            }
            _ => panic!("Expected to parse as list command"),
        }
    }

    #[test]
    fn parses_request_get_and_wait_commands() {
        let cli = Cli::try_parse_from(["mcpstore", "request", "get", "req-1", "--output", "json"])
            .unwrap();
        match cli.command {
            Commands::Request(args) => match args.action {
                commands::request::RequestAction::Get(args) => {
                    assert_eq!(args.request_id, "req-1");
                    assert_eq!(args.output, OutputFormat::Json);
                }
                _ => panic!("Expected request get"),
            },
            _ => panic!("Expected request command"),
        }

        let cli = Cli::try_parse_from(["mcpstore", "request", "wait", "req-1", "--timeout", "7"])
            .unwrap();
        match cli.command {
            Commands::Request(args) => match args.action {
                commands::request::RequestAction::Wait(args) => {
                    assert_eq!(args.request_id, "req-1");
                    assert_eq!(args.timeout, 7);
                }
                _ => panic!("Expected request wait"),
            },
            _ => panic!("Expected request command"),
        }
    }

    #[test]
    fn parses_migrate_store_command() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "migrate-store",
            "--source",
            "local",
            "--target-store",
            "redis",
            "--target-config",
            "redis://127.0.0.1:6379/0",
        ])
        .unwrap();

        match cli.command {
            Commands::MigrateStore(args) => {
                assert_eq!(args.store.source, crate::store_args::SourceArg::Local);
                assert_eq!(args.target_store, "redis");
                assert_eq!(
                    args.target_config.as_deref(),
                    Some("redis://127.0.0.1:6379/0")
                );
            }
            _ => panic!("Expected to parse as migrate-store command"),
        }
    }

    #[test]
    fn parses_web_command() {
        let cli = Cli::try_parse_from(["mcpstore", "web", "--json"]).unwrap();
        match cli.command {
            Commands::Web { json } => assert!(json),
            _ => panic!("Expected to parse as web view command"),
        }
        // 启动语义已删除：旧 flag 必须解析失败
        assert!(Cli::try_parse_from(["mcpstore", "web", "--port", "9090"]).is_err());
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
        let cli = Cli::try_parse_from(["mcpstore", "api", "--json"]).unwrap();
        match cli.command {
            Commands::Api { json } => assert!(json),
            _ => panic!("Expected to parse as api view command"),
        }
        // 启动语义已删除：旧 flag 必须解析失败
        assert!(
            Cli::try_parse_from(["mcpstore", "api", "--port", "9091", "--url-prefix", "/mcp"])
                .is_err()
        );
    }

    #[test]
    fn parses_config_edit_flags() {
        let cli =
            Cli::try_parse_from(["mcpstore", "config", "--web-port", "1829", "--core", "off"])
                .unwrap();
        match cli.command {
            Commands::Config { action, edits, .. } => {
                assert!(action.is_none());
                assert_eq!(edits.web_port, Some(1829));
                assert_eq!(edits.core.as_deref(), Some("off"));
            }
            _ => panic!("Expected to parse as config edit command"),
        }
    }

    #[test]
    fn parses_daemon_restart() {
        let cli = Cli::try_parse_from(["mcpstore", "daemon", "restart"]).unwrap();
        match cli.command {
            Commands::Daemon { action } => {
                assert!(matches!(
                    action,
                    commands::daemon_cmd::DaemonAction::Restart
                ));
            }
            _ => panic!("Expected to parse as daemon restart"),
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
            "--runtime",
            "local",
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
                assert_eq!(args.output.output, crate::error::OutputFormat::Json);
                assert_eq!(
                    args.execution.runtime,
                    Some(crate::commands::mcp::RuntimeArg::Local)
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
            Commands::Complete(ref args) => {
                assert_eq!(
                    args.reference_kind,
                    commands::protocol::CompletionReferenceKind::Resource
                );
                assert_eq!(args.reference, "repo://mcp/{name}");
                assert_eq!(args.output.output, crate::error::OutputFormat::Jsonl);
            }
            _ => panic!("Expected complete command"),
        }
    }

    #[test]
    fn parses_list_and_tools_machine_output_flags() {
        let list_cli = Cli::try_parse_from(["mcpstore", "list", "--output", "json"]).unwrap();
        match list_cli.command {
            Commands::List(args) => {
                assert_eq!(args.output, crate::error::OutputFormat::Json);
            }
            _ => panic!("Expected list command"),
        }

        let tools_cli = Cli::try_parse_from([
            "mcpstore",
            "tools",
            "127ce370-1ed6-5b00-9713-e88d01b3010d",
            "--output",
            "jsonl",
            "--schema",
        ])
        .unwrap();
        match tools_cli.command {
            Commands::Tools(args) => {
                assert_eq!(args.output, crate::error::OutputFormat::Jsonl);
                assert!(args.schema);
            }
            _ => panic!("Expected tools command"),
        }
    }

    #[test]
    fn parses_check_exit_code_flags() {
        let cli = Cli::try_parse_from([
            "mcpstore",
            "check",
            "127ce370-1ed6-5b00-9713-e88d01b3010d",
            "--exit-code",
            "--quiet",
        ])
        .unwrap();
        match cli.command {
            Commands::Check(args) => {
                assert!(args.exit_code);
                assert!(args.quiet);
            }
            _ => panic!("Expected check command"),
        }
    }
}
