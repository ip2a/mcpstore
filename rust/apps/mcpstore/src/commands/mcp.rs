use clap::{Args, ValueEnum};
use mcpstore::config::{
    McpStoreExtension, RuntimePolicy, ScopeDeclarations, ScopeDescriptor, ServerConfig,
};
use mcpstore::error::{Error, FailureCode};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::Duration;

use crate::daemon::protocol::KernelOperation;
use crate::error::{attach_tool, OutputFormat};

use mcpstore::{
    InstanceId, McpExecutionOptions, McpStoreExecutionUpdate, McpToolExecution, ScopeRef,
    ToolCallResult,
};

use crate::{
    commands::elicitation::{
        handle_elicitation, settle_execution_after_elicitation_error, ElicitationArgs,
        ElicitationCommandError, ElicitationErrorKind,
    },
    store_args::{open_store_access, StoreAccess, StoreSourceArgs},
    BoxErr,
};

/// 命令统一入口：默认 daemon，--embedded/显式 store 参数时本进程冷启动。
pub(crate) async fn open_store(
    store_args: &StoreSourceArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> mcpstore::Result<StoreAccess> {
    open_store_access(store_args, embedded, endpoint)
        .await
        .map_err(|error| {
            Error::new(
                FailureCode::ServiceUnavailable,
                format!("Failed to open store: {error}"),
            )
        })
}

pub(crate) fn mutation_receipt(result: &Value) -> (&str, &str) {
    let status = result["status"].as_str().unwrap_or("applied");
    let request_id = result["request_id"].as_str().unwrap_or("");
    (status, request_id)
}

pub(crate) fn print_mutation(message: &str, result: &Value) {
    let (status, request_id) = mutation_receipt(result);
    if status == "queued" {
        println!("[Queued] {message} request_id={request_id}");
    } else {
        println!("[Success] {message}");
    }
}

#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum HandshakeArg {
    Auto,
    Discover,
    Initialize,
}

impl HandshakeArg {
    pub fn to_mode(&self) -> mcpstore::config::HandshakeMode {
        match self {
            Self::Auto => mcpstore::config::HandshakeMode::Auto,
            Self::Discover => mcpstore::config::HandshakeMode::Discover,
            Self::Initialize => mcpstore::config::HandshakeMode::Initialize,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, ValueEnum, Default)]
pub enum Scope {
    #[default]
    Store,
    Agent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum RuntimeArg {
    Local,
    Daemon,
}

impl RuntimeArg {
    fn to_runtime(self) -> mcpstore::config::Runtime {
        match self {
            Self::Local => mcpstore::config::Runtime::Local,
            Self::Daemon => mcpstore::config::Runtime::Daemon,
        }
    }
}

impl Scope {
    pub fn to_ref(&self, agent: Option<&str>) -> std::result::Result<ScopeRef, BoxErr> {
        match self {
            Self::Store => {
                validate_agent_flag(self, agent)?;
                Ok(ScopeRef::Store)
            }
            Self::Agent => Ok(ScopeRef::Agent {
                agent_id: require_agent(agent)?.to_string(),
            }),
        }
    }
}

#[derive(Args)]
pub struct AddArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[arg(help = "Streamable HTTP URL or stdio command; stdio recommended after --")]
    pub command_or_url: Option<String>,
    #[arg(trailing_var_arg = true, help = "stdio command arguments")]
    pub args: Vec<String>,
    #[arg(long, help = "Transport type: stdio, http, or streamable-http")]
    pub transport: Option<String>,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(
        long,
        short = 'e',
        num_args = 1,
        help = "Process env vars, format KEY=VAL, repeatable"
    )]
    pub env: Vec<String>,
    #[arg(long, num_args = 1, help = "HTTP headers, format KEY=VAL, repeatable")]
    pub header: Vec<String>,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(
        long,
        value_enum,
        help = "Client handshake mode: initialize (default), auto, or discover"
    )]
    pub handshake: Option<HandshakeArg>,
    /// Repeatable host capability required by local execution (for example browser)
    #[arg(long = "require-host-capability", value_name = "CAPABILITY")]
    pub require_host_capability: Vec<String>,
    #[arg(long = "allow-runtime", value_name = "RUNTIME")]
    pub allow_runtime: Vec<mcpstore::config::Runtime>,
    #[arg(long = "allow-daemon", value_name = "ID")]
    pub allow_daemon: Vec<String>,
}

pub async fn add(
    a: AddArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    validate_scope_target(&a.scope, a.agent.as_deref())?;

    let env_map = parse_env(&a.env)?;
    let header_map = parse_headers(&a.header)?;
    let mut config = build_server_config(
        a.command_or_url.as_deref(),
        &a.args,
        a.transport.as_deref(),
        &env_map,
        &header_map,
    )?;
    let transport = config.infer_transport().to_string();
    if let Some(handshake) = a.handshake.as_ref().map(|h| h.to_mode()) {
        let extension = config.mcpstore.get_or_insert_with(Default::default);
        extension.handshake_mode = Some(handshake);
    }
    if let Some(policy) = runtime_policy_from_flags(
        &a.allow_runtime,
        &a.allow_daemon,
        &a.require_host_capability,
    ) {
        config
            .mcpstore
            .get_or_insert_with(Default::default)
            .runtime_policy = Some(policy);
    }
    let scope = a.scope.to_ref(a.agent.as_deref())?;
    if let ScopeRef::Agent { agent_id } = &scope {
        let previous = config.mcpstore.take();
        let mut scopes = ScopeDeclarations::default();
        scopes
            .agents
            .insert(agent_id.clone(), ScopeDescriptor::default());
        config.mcpstore = Some(McpStoreExtension {
            scopes,
            lifecycle: previous
                .as_ref()
                .and_then(|extension| extension.lifecycle.clone()),
            handshake_mode: previous
                .as_ref()
                .and_then(|extension| extension.handshake_mode),
            runtime_policy: previous
                .as_ref()
                .and_then(|extension| extension.runtime_policy.clone()),
            revision: previous
                .as_ref()
                .map(|extension| extension.revision)
                .unwrap_or(1)
                .max(1),
            extra: previous
                .map(|extension| extension.extra)
                .unwrap_or_default(),
        });
    }

    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let result = access
        .request(
            KernelOperation::AddService,
            json!({"name": a.name, "config": config, "scope": scope}),
        )
        .await?;
    print_mutation(
        &format!("Service added: {} (transport={})", a.name, transport),
        &result,
    );
    Ok(())
}

#[derive(Args)]
pub struct AddJsonArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[arg(help = "ServerConfig JSON string")]
    pub json: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
}

pub async fn add_json(
    a: AddJsonArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    validate_scope_target(&a.scope, a.agent.as_deref())?;
    let mut config: ServerConfig = serde_json::from_str(&a.json)?;
    let transport = config.infer_transport().to_string();
    let scope = a.scope.to_ref(a.agent.as_deref())?;
    if let ScopeRef::Agent { agent_id } = &scope {
        let previous = config.mcpstore.take();
        let mut scopes = ScopeDeclarations::default();
        scopes
            .agents
            .insert(agent_id.clone(), ScopeDescriptor::default());
        config.mcpstore = Some(McpStoreExtension {
            scopes,
            lifecycle: previous
                .as_ref()
                .and_then(|extension| extension.lifecycle.clone()),
            handshake_mode: previous
                .as_ref()
                .and_then(|extension| extension.handshake_mode),
            runtime_policy: previous
                .as_ref()
                .and_then(|extension| extension.runtime_policy.clone()),
            revision: previous
                .as_ref()
                .map(|extension| extension.revision)
                .unwrap_or(1)
                .max(1),
            extra: previous
                .map(|extension| extension.extra)
                .unwrap_or_default(),
        });
    }
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let result = access
        .request(
            KernelOperation::AddService,
            json!({"name": a.name, "config": config, "scope": scope}),
        )
        .await?;
    print_mutation(
        &format!("Service added: {} (transport={})", a.name, transport),
        &result,
    );
    Ok(())
}

#[derive(Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Human,
        help = "Output format: human, json, or jsonl"
    )]
    pub output: OutputFormat,
}

pub async fn list(
    a: ListArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = a.scope.to_ref(a.agent.as_deref())?;

    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let result = access
        .request(KernelOperation::ListServices, json!({ "scope": scope }))
        .await?;
    let services = result["services"].as_array().cloned().unwrap_or_default();

    if a.output != OutputFormat::Human {
        let summaries: Vec<Value> = services
            .iter()
            .map(|service| {
                json!({
                    "service_name": service["service_name"],
                    "instance_id": service["instance_id"],
                    "transport": service["transport"],
                    "readiness": service["state"]["readiness"]["status"],
                    "tools_count": service["tools_count"],
                })
            })
            .collect();
        emit_call_value(
            a.output,
            json!({ "services": summaries, "total": summaries.len() }),
        )?;
        return Ok(());
    }

    println!("[List] service_count={}", services.len());

    if services.is_empty() {
        println!("  No services available");
        return Ok(());
    }

    for service in &services {
        let state = &service["state"];
        println!(
            "- {}  instance={}  transport={}  readiness={}  phase={}  health={}  tools={}  capabilities={}",
            service["service_name"].as_str().unwrap_or("?"),
            service["instance_id"].as_str().unwrap_or("?"),
            service["transport"].as_str().unwrap_or("?"),
            state["readiness"]["status"].as_str().unwrap_or("?"),
            state["phase"].as_str().unwrap_or("?"),
            state["health"].as_str().unwrap_or("?"),
            service["tools_count"].as_u64().unwrap_or_default(),
            format_capabilities(service.get("mcp")),
        );
    }
    Ok(())
}

#[derive(Args)]
pub struct GetArgs {
    #[arg(value_name = "SERVICE|INSTANCE", help = "Service name or instance ID")]
    pub target: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, help = "Output format: human, json, or jsonl")]
    pub output: OutputFormat,
}

pub async fn get(
    a: GetArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = a
        .scope
        .to_ref(a.agent.as_deref())
        .map_err(|e| Error::new(FailureCode::InvalidInput, e.to_string()))?;
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let instance_id = resolve_target(&mut access, &scope, &a.target)
        .await
        .map_err(resolve_error)?;
    let payload = access
        .request(
            KernelOperation::GetServiceInfo,
            json!({"instance_id": instance_id.to_string()}),
        )
        .await?;
    match a.output {
        OutputFormat::Human => {
            let json = serde_json::to_string_pretty(&payload)
                .map_err(|e| Error::new(FailureCode::Internal, e.to_string()))?;
            println!("{json}");
        }
        _ => {
            emit_call_value(
                a.output,
                json!({"event": "service.info", "instance_id": instance_id.to_string(), "info": payload}),
            )?;
        }
    }
    Ok(())
}

#[derive(Args)]
pub struct RemoveArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
}

pub async fn remove(
    a: RemoveArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = a.scope.to_ref(a.agent.as_deref())?;
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let result = access
        .request(
            KernelOperation::RemoveServiceScope,
            json!({"service_name": a.name, "scope": scope}),
        )
        .await?;
    print_mutation(&format!("Service scope removed: {}", a.name), &result);
    Ok(())
}

#[derive(Args)]
pub struct ConnectArgs {
    #[arg(value_name = "SERVICE|INSTANCE", help = "Service name or instance ID")]
    pub target: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, help = "Output format: human, json, or jsonl")]
    pub output: OutputFormat,
}

pub async fn connect(
    a: ConnectArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = a
        .scope
        .to_ref(a.agent.as_deref())
        .map_err(|e| Error::new(FailureCode::InvalidInput, e.to_string()))?;
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let instance_id = resolve_target(&mut access, &scope, &a.target)
        .await
        .map_err(resolve_error)?;
    let result = access
        .request(
            KernelOperation::ConnectService,
            json!({"instance_id": instance_id.to_string()}),
        )
        .await?;
    let (status, request_id) = mutation_receipt(&result);
    if status == "queued" {
        if a.output == OutputFormat::Human {
            println!(
                "[Queued] Connected: {} request_id={request_id}",
                instance_id
            );
        } else {
            emit_call_value(
                a.output,
                json!({
                    "event": "service.connect_queued",
                    "instance_id": instance_id.to_string(),
                    "request_id": request_id,
                    "status": status,
                }),
            )?;
        }
        return Ok(());
    }
    let tools = result["tools"].as_array().cloned().unwrap_or_default();
    let tools_count = result["tools_count"]
        .as_u64()
        .unwrap_or_else(|| tools.len() as u64);
    let capabilities = format_capabilities(result.get("mcp"));
    match a.output {
        OutputFormat::Human => {
            println!(
                "[Success] Connected: {} (tools={}, capabilities={})",
                instance_id, tools_count, capabilities
            );
            for tool in &tools {
                println!(
                    "  - {}: {}",
                    tool["name"].as_str().unwrap_or("?"),
                    tool["description"].as_str().unwrap_or("")
                );
            }
        }
        _ => {
            emit_call_value(
                a.output,
                json!({
                    "event": "service.connected",
                    "instance_id": instance_id.to_string(),
                    "request_id": request_id,
                    "tools_count": tools_count,
                    "capabilities": capabilities,
                }),
            )?;
        }
    }
    Ok(())
}

#[derive(Args)]
pub struct DisconnectArgs {
    #[arg(value_name = "SERVICE|INSTANCE", help = "Service name or instance ID")]
    pub target: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, help = "Output format: human, json, or jsonl")]
    pub output: OutputFormat,
}

pub async fn disconnect(
    a: DisconnectArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = a
        .scope
        .to_ref(a.agent.as_deref())
        .map_err(|e| Error::new(FailureCode::InvalidInput, e.to_string()))?;
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let instance_id = resolve_target(&mut access, &scope, &a.target)
        .await
        .map_err(resolve_error)?;
    access
        .request(
            KernelOperation::DisconnectService,
            json!({"instance_id": instance_id.to_string()}),
        )
        .await?;
    match a.output {
        OutputFormat::Human => {
            println!("[Success] Disconnected: {}", instance_id);
        }
        _ => {
            emit_call_value(
                a.output,
                json!({"event": "service.disconnected", "instance_id": instance_id.to_string()}),
            )?;
        }
    }
    Ok(())
}

#[derive(Args)]
pub struct RestartArgs {
    #[arg(value_name = "SERVICE|INSTANCE", help = "Service name or instance ID")]
    pub target: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, help = "Output format: human, json, or jsonl")]
    pub output: OutputFormat,
}

pub async fn restart(
    a: RestartArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = a
        .scope
        .to_ref(a.agent.as_deref())
        .map_err(|e| Error::new(FailureCode::InvalidInput, e.to_string()))?;
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let instance_id = resolve_target(&mut access, &scope, &a.target)
        .await
        .map_err(resolve_error)?;
    access
        .request(
            KernelOperation::RestartService,
            json!({"instance_id": instance_id.to_string()}),
        )
        .await?;
    match a.output {
        OutputFormat::Human => {
            println!("[Success] Restarted: {}", instance_id);
        }
        _ => {
            emit_call_value(
                a.output,
                json!({"event": "service.restarted", "instance_id": instance_id.to_string()}),
            )?;
        }
    }
    Ok(())
}

#[derive(Args)]
pub struct CheckArgs {
    #[arg(value_name = "SERVICE|INSTANCE", help = "Service name or instance ID")]
    pub target: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, help = "Output format: human, json, or jsonl")]
    pub output: OutputFormat,
    #[arg(long, help = "Exit 0 when ready, non-zero otherwise")]
    pub exit_code: bool,
    #[arg(
        long,
        help = "Suppress output; signal readiness only via the exit code"
    )]
    pub quiet: bool,
}

pub async fn check(
    a: CheckArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = a
        .scope
        .to_ref(a.agent.as_deref())
        .map_err(|e| Error::new(FailureCode::InvalidInput, e.to_string()))?;
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let instance_id = resolve_target(&mut access, &scope, &a.target)
        .await
        .map_err(resolve_error)?;
    let result = access
        .request(
            KernelOperation::CheckService,
            json!({"instance_id": instance_id.to_string()}),
        )
        .await?;
    let state = &result["state"];
    let ready = state["readiness"]["status"].as_str() == Some("ready");
    let label = format!(
        "{instance_id} => readiness={} phase={} health={}",
        state["readiness"]["status"].as_str().unwrap_or("?"),
        state["phase"].as_str().unwrap_or("?"),
        state["health"].as_str().unwrap_or("?"),
    );

    if !a.quiet {
        match a.output {
            OutputFormat::Human => {
                println!("[Check] {label}");
            }
            _ => {
                emit_call_value(
                    a.output,
                    json!({
                        "event": "service.checked",
                        "instance_id": instance_id.to_string(),
                        "ready": ready,
                        "label": label,
                    }),
                )?;
            }
        }
    }
    if a.exit_code {
        std::process::exit(i32::from(!ready));
    }
    Ok(())
}

#[derive(Args)]
pub struct WaitArgs {
    #[arg(value_name = "SERVICE|INSTANCE", help = "Service name or instance ID")]
    pub target: String,
    #[arg(long, default_value_t = 30, help = "Wait timeout in seconds")]
    pub timeout: u64,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, help = "Output format: human, json, or jsonl")]
    pub output: OutputFormat,
}

pub async fn wait(
    a: WaitArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = a
        .scope
        .to_ref(a.agent.as_deref())
        .map_err(|e| Error::new(FailureCode::InvalidInput, e.to_string()))?;
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let instance_id = resolve_target(&mut access, &scope, &a.target)
        .await
        .map_err(resolve_error)?;
    // 与既有语义一致：先触发连接，再等待就绪。
    access
        .request(
            KernelOperation::ConnectService,
            json!({"instance_id": instance_id.to_string()}),
        )
        .await?;
    let result = access
        .request(
            KernelOperation::WaitService,
            json!({"instance_id": instance_id.to_string(), "timeout": a.timeout}),
        )
        .await?;
    let state = &result["state"];
    match a.output {
        OutputFormat::Human => {
            println!(
                "[Success] Service ready: {} (readiness={}, health={})",
                instance_id,
                state["readiness"]["status"].as_str().unwrap_or("?"),
                state["health"].as_str().unwrap_or("?"),
            );
        }
        _ => {
            emit_call_value(
                a.output,
                json!({
                    "event": "service.ready",
                    "instance_id": instance_id.to_string(),
                    "readiness": format!(
                        "{} {}",
                        state["readiness"]["status"].as_str().unwrap_or("?"),
                        state["health"].as_str().unwrap_or("?"),
                    ),
                }),
            )?;
        }
    }
    Ok(())
}

#[derive(Args)]
pub struct UpdateArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[arg(help = "Streamable HTTP URL or stdio command; stdio recommended after --")]
    pub command_or_url: Option<String>,
    #[arg(trailing_var_arg = true, help = "stdio command arguments")]
    pub args: Vec<String>,
    #[arg(long, help = "Transport type: stdio, http, or streamable-http")]
    pub transport: Option<String>,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(
        long,
        short = 'e',
        num_args = 1,
        help = "Process env vars, format KEY=VAL, repeatable"
    )]
    pub env: Vec<String>,
    #[arg(long, num_args = 1, help = "HTTP headers, format KEY=VAL, repeatable")]
    pub header: Vec<String>,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    /// Repeatable host capability required by local execution (for example browser)
    #[arg(long = "require-host-capability", value_name = "CAPABILITY")]
    pub require_host_capability: Vec<String>,
    #[arg(long = "allow-runtime", value_name = "RUNTIME")]
    pub allow_runtime: Vec<mcpstore::config::Runtime>,
    #[arg(long = "allow-daemon", value_name = "ID")]
    pub allow_daemon: Vec<String>,
}

pub async fn update(
    a: UpdateArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    validate_scope_target(&a.scope, a.agent.as_deref())?;
    let env_map = parse_env(&a.env)?;
    let header_map = parse_headers(&a.header)?;
    let config = build_server_config(
        a.command_or_url.as_deref(),
        &a.args,
        a.transport.as_deref(),
        &env_map,
        &header_map,
    )?;
    let runtime_policy = runtime_policy_from_flags(
        &a.allow_runtime,
        &a.allow_daemon,
        &a.require_host_capability,
    );
    if a.scope == Scope::Agent && runtime_policy.is_some() {
        return Err("Runtime policy is definition-level; use --scope store".into());
    }
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let receipt = match a.scope.to_ref(a.agent.as_deref())? {
        ScopeRef::Store => {
            access
                .request(
                    KernelOperation::UpdateService,
                    json!({
                        "name": a.name,
                        "config": config,
                        "runtime_policy": runtime_policy,
                    }),
                )
                .await?
        }
        scope @ ScopeRef::Agent { .. } => {
            access
                .request(
                    KernelOperation::DeclareServiceScope,
                    json!({
                        "service_name": a.name,
                        "scope": scope,
                        "descriptor": ScopeDescriptor {
                            config: config.base_config(),
                            lifecycle: None,
                            revision: 0,
                            ..Default::default()
                        },
                    }),
                )
                .await?
        }
    };
    print_mutation(&format!("Service updated: {}", a.name), &receipt);
    Ok(())
}

#[derive(Args)]
pub struct ToolsArgs {
    #[arg(value_name = "SERVICE|INSTANCE", help = "Service name or instance ID")]
    pub target: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Human,
        help = "Output format: human, json, or jsonl"
    )]
    pub output: OutputFormat,
    #[arg(long, help = "Include each tool's input schema")]
    pub schema: bool,
}

pub async fn tools(
    a: ToolsArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = a.scope.to_ref(a.agent.as_deref())?;
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let instance_id = resolve_target(&mut access, &scope, &a.target).await?;
    let result = access
        .request(
            KernelOperation::ListTools,
            json!({"instance_id": instance_id.to_string()}),
        )
        .await?;
    let tool_values = result["tools"].as_array().cloned().unwrap_or_default();
    let entries: Vec<Value> = tool_values
        .iter()
        .map(|tool| tool_summary_value(tool.clone(), a.schema))
        .collect();

    if a.output != OutputFormat::Human {
        emit_call_value(
            a.output,
            json!({ "instance_id": instance_id, "tools": entries, "total": entries.len() }),
        )?;
        return Ok(());
    }
    println!("[Tools] instance={} count={}", instance_id, entries.len());
    for t in &entries {
        println!(
            "  - {}: {}",
            t.get("name").and_then(Value::as_str).unwrap_or("?"),
            t.get("description").and_then(Value::as_str).unwrap_or("")
        );
    }
    Ok(())
}

/// Build a tool summary `{name, description}` plus `schema` when requested, from
/// a value that carries `name`, `description`, and (optionally) `schema`.
fn tool_summary_value(tool: Value, include_schema: bool) -> Value {
    let mut summary = json!({
        "name": tool.get("name").and_then(Value::as_str).unwrap_or(""),
        "description": tool.get("description").and_then(Value::as_str).unwrap_or(""),
    });
    if include_schema {
        if let Some(schema) = tool.get("schema") {
            summary["schema"] = schema.clone();
        }
    }
    summary
}

/// Attach instance/tool context to a store error from the `call` command.
fn call_error_from_store(
    error: mcpstore::Error,
    instance_id: InstanceId,
    tool_name: &str,
) -> mcpstore::Error {
    attach_tool(error, instance_id, tool_name)
}

#[derive(Clone, Debug, Args)]
pub struct RuntimeArgs {
    #[arg(
        long = "runtime",
        value_enum,
        value_name = "RUNTIME",
        help = "Execution runtime: local or daemon (default: auto)"
    )]
    pub runtime: Option<RuntimeArg>,
    #[arg(
        long = "daemon-node",
        value_name = "ID",
        help = "Route execution to a configured daemon node (implies --runtime daemon)"
    )]
    pub daemon_node: Option<String>,
}

impl RuntimeArgs {
    pub(crate) fn resolve(
        &self,
        embedded: bool,
    ) -> mcpstore::Result<mcpstore::config::RuntimeSelection> {
        use mcpstore::config::{Runtime, RuntimeSelection};
        if let Some(node) = &self.daemon_node {
            if self.runtime == Some(RuntimeArg::Local) {
                return Err(Error::new(
                    FailureCode::InvalidInput,
                    "--daemon-node requires --runtime daemon",
                ));
            }
            return Ok(RuntimeSelection::daemon_node(node.clone()));
        }
        let runtime = match self.runtime.map(RuntimeArg::to_runtime) {
            None if embedded => Runtime::Local,
            None => Runtime::Daemon,
            Some(Runtime::Local) if embedded => Runtime::Local,
            Some(Runtime::Daemon) if !embedded => Runtime::Daemon,
            Some(Runtime::Local) => {
                return Err(Error::new(
                    FailureCode::InvalidInput,
                    "local execution is available only with embedded access",
                ))
            }
            Some(Runtime::Daemon) => {
                return Err(Error::new(
                    FailureCode::InvalidInput,
                    "daemon execution is available only with daemon access",
                ))
            }
        };
        Ok(RuntimeSelection::runtime(runtime))
    }
}

#[derive(Clone, Args)]
pub struct CallToolArgs {
    #[arg(value_name = "SERVICE|INSTANCE", help = "Service name or instance ID")]
    pub target: String,
    #[arg(value_name = "TOOL", help = "Tool name")]
    pub tool_name: String,
    #[arg(
        trailing_var_arg = true,
        value_name = "ARGS",
        help = "Tool arguments: key:value | key=value | --key=value (named options must precede trailing ARGS)"
    )]
    pub args: Vec<String>,
    #[arg(
        long,
        default_value = "{}",
        help = "Tool arguments JSON object, merged with ARGS"
    )]
    pub arguments: String,
    #[arg(
        long,
        value_enum,
        default_value_t = Scope::Store,
        help = "Scope used to resolve a service name target"
    )]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Human,
        help = "Output format: human, json, or jsonl"
    )]
    pub output: OutputFormat,
    #[command(flatten)]
    pub execution: RuntimeArgs,
    #[arg(
        long,
        value_name = "SECONDS",
        help = "Idle timeout, reset by matching progress"
    )]
    pub timeout: Option<u64>,
    #[arg(
        long = "max-total-timeout",
        value_name = "SECONDS",
        help = "Maximum total execution time"
    )]
    pub max_total_timeout: Option<u64>,
    #[arg(long, help = "Guarantee that the command does not prompt for input")]
    pub non_interactive: bool,
    #[command(flatten)]
    pub elicitation: ElicitationArgs,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Args)]
pub struct MigrateStoreArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long = "target-store", help = "Target store name")]
    pub target_store: String,
    #[arg(long = "target-config", help = "Target store configuration")]
    pub target_config: Option<String>,
}

pub async fn call_tool(
    a: CallToolArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    execute_call_tool(a, embedded, endpoint.clone())
        .await
        .map_err(|error| Box::new(error) as BoxErr)
}

pub(crate) fn host_capabilities() -> &'static HashSet<&'static str> {
    static CAPABILITIES: OnceLock<HashSet<&'static str>> = OnceLock::new();
    CAPABILITIES.get_or_init(|| {
        let mut capabilities = HashSet::from(["browser"]);
        if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
            capabilities.insert("display");
        }
        capabilities
    })
}

fn ensure_host_capabilities(
    policy: &mcpstore::config::RuntimePolicy,
    runtime: mcpstore::config::Runtime,
) -> mcpstore::Result<()> {
    if runtime != mcpstore::config::Runtime::Local {
        return Ok(());
    }
    let missing: Vec<_> = policy
        .required_host_capabilities
        .iter()
        .filter(|capability| !host_capabilities().contains(capability.as_str()))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(Error::new(
        FailureCode::CapabilityUnsupported,
        format!(
            "local host lacks required capabilities: {}",
            missing
                .iter()
                .map(|capability| capability.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ))
}

pub(crate) fn resolve_declared_runtime(
    info: &Value,
    selection: mcpstore::config::RuntimeSelection,
) -> mcpstore::Result<mcpstore::config::RuntimeSelection> {
    let policy: Option<mcpstore::config::RuntimePolicy> = info
        .get("runtime_policy")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
    if let Some(policy) = policy {
        ensure_host_capabilities(&policy, selection.runtime)?;
        if !policy.allows_runtime(selection.runtime) {
            return Err(Error::new(
                FailureCode::InvalidInput,
                format!(
                    "runtime '{}' not allowed for instance; allowed: {}",
                    selection.runtime,
                    policy
                        .allowed_runtimes
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        if let Some(node) = &selection.daemon_node {
            if !policy.allows_daemon(node) {
                return Err(Error::new(
                    FailureCode::InvalidInput,
                    format!(
                        "daemon node '{node}' not allowed for instance; allowed: {}",
                        policy.allowed_daemons.join(", ")
                    ),
                ));
            }
        }
    }
    Ok(selection)
}

/// Inject the resolved runtime selection into a daemon op payload as the
/// `runtime` (+ optional `daemon_node`) keys.
pub(crate) fn insert_runtime(payload: &mut Value, selection: &mcpstore::config::RuntimeSelection) {
    payload["runtime"] = json!(selection.runtime);
    if let Some(node) = &selection.daemon_node {
        payload["daemon_node"] = json!(node);
    }
}

async fn execute_call_tool(
    a: CallToolArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> mcpstore::Result<()> {
    parse_arguments_json_object(&a.arguments, a.output)?;
    // Explicit store arguments force embedded access in open_store_access.
    let embedded = embedded || a.store.is_explicit();
    let selection = a.execution.resolve(embedded)?;
    let scope = a
        .scope
        .to_ref(a.agent.as_deref())
        .map_err(|error| Error::new(FailureCode::InvalidInput, error.to_string()))?;
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let result = async {
        let instance_id = resolve_target(&mut access, &scope, &a.target)
            .await
            .map_err(resolve_error)?;
        let info = access
            .request(
                KernelOperation::GetServiceInfo,
                json!({"instance_id": instance_id.to_string()}),
            )
            .await
            .map_err(|error| call_error_from_store(error, instance_id, &a.tool_name))?;
        let selection = resolve_declared_runtime(&info, selection)?;
        access
            .request(
                KernelOperation::ConnectService,
                json!({"instance_id": instance_id.to_string()}),
            )
            .await
            .map_err(|error| call_error_from_store(error, instance_id, &a.tool_name))?;
        let schema = load_tool_input_schema(&mut access, instance_id, &a.tool_name).await?;
        let args = build_call_arguments(&a.args, &a.arguments, schema.as_ref(), a.output)?;

        let mut options = McpExecutionOptions::default();
        if let Some(timeout) = a.timeout {
            options = options.with_idle_timeout(Duration::from_secs(timeout));
        }
        if let Some(timeout) = a.max_total_timeout {
            options = options.with_max_total_timeout(Duration::from_secs(timeout));
        }

        match &mut access {
            StoreAccess::Embedded(store) => {
                call_embedded(&store, instance_id, &a, args, options).await
            }
            StoreAccess::Remote(ref mut client) => {
                call_remote(client, instance_id, &a, args, options, selection).await
            }
        }
    };
    let result = result.await;
    access.close().await;
    result
}

/// embedded 流式路径：elicitation 交互与 Ctrl-C 取消全保留。
async fn call_embedded(
    store: &std::sync::Arc<mcpstore::MCPStore>,
    instance_id: InstanceId,
    a: &CallToolArgs,
    args: Value,
    options: McpExecutionOptions,
) -> mcpstore::Result<()> {
    let tool_name = a.tool_name.as_str();
    let mut elicitation = store
        .open_elicitation_session(instance_id, a.elicitation.session_options())
        .await
        .map_err(|error| call_error_from_store(error, instance_id, tool_name))?;
    let mut execution = store
        .start_tool_execution(instance_id, tool_name, args, None, options)
        .await
        .map_err(|error| call_error_from_store(error, instance_id, tool_name))?;
    emit_call_started(a.output, tool_name, &execution)?;

    let mut cancellation_requested = false;
    loop {
        let update = if cancellation_requested {
            execution.next_update().await
        } else {
            tokio::select! {
                biased;
                update = execution.next_update() => update,
                request = async {
                    match elicitation.as_mut() {
                        Some(session) => session.next_request().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match request {
                        Some(request) => {
                            if let Err(error) = handle_elicitation(
                                request,
                                &a.elicitation,
                                a.output,
                                a.non_interactive,
                            )
                            .await
                            {
                                settle_execution_after_elicitation_error(&mut execution).await;
                                return Err(call_elicitation_error(
                                    error,
                                    instance_id,
                                    tool_name,
                                ));
                            }
                        }
                        None => elicitation = None,
                    }
                    continue;
                }
                signal = tokio::signal::ctrl_c() => {
                    signal.map_err(|error| attach_tool(Error::new(FailureCode::Internal, format!("failed to listen for Ctrl+C: {error}")), instance_id, tool_name))?;
                    if execution.cancel("cancelled by user (Ctrl+C)") {
                        cancellation_requested = true;
                        emit_call_cancellation_requested(a.output, instance_id, tool_name)?;
                    }
                    continue;
                }
            }
        };

        match update {
            Some(McpStoreExecutionUpdate::Progress(progress)) => {
                emit_call_progress(a.output, tool_name, &progress)?;
            }
            Some(McpStoreExecutionUpdate::Finished(result)) => {
                let execution =
                    result.map_err(|error| call_error_from_store(error, instance_id, tool_name))?;
                return finish_call_execution(a.output, instance_id, tool_name, execution);
            }
            None => {
                return Err(attach_tool(
                    Error::new(
                        FailureCode::ToolFailed,
                        "tool execution ended without a result",
                    ),
                    instance_id,
                    tool_name,
                ));
            }
        }
    }
}

/// daemon 流式路径：事件透传到本地输出。elicitation 在 daemon 模式不可用
/// （headless 语义，同 --non-interactive）；Ctrl-C 终止 CLI 进程即断开事件流，
/// daemon 侧执行继续（不自动重放）。
async fn call_remote(
    client: &mut crate::daemon::client::KernelClient,
    instance_id: InstanceId,
    a: &CallToolArgs,
    args: Value,
    options: McpExecutionOptions,
    selection: mcpstore::config::RuntimeSelection,
) -> mcpstore::Result<()> {
    let tool_name = a.tool_name.as_str();
    let mut payload = json!({
        "instance_id": instance_id.to_string(),
        "tool_name": tool_name,
        "args": args,
    });
    insert_runtime(&mut payload, &selection);
    if let Some(timeout) = options.idle_timeout {
        payload["idle_timeout"] = json!(timeout.as_millis() as u64);
    }
    if let Some(timeout) = options.max_total_timeout {
        payload["max_total_timeout"] = json!(timeout.as_millis() as u64);
    }

    let output = a.output;
    let mut instance_for_events = instance_id;
    let result = client
        .request_stream(
            KernelOperation::StreamToolExecution,
            payload,
            Duration::from_secs(600),
            |event| match event {
                crate::daemon::protocol::KernelEvent::Started {
                    request_id,
                    instance_id: started_instance,
                    cancellation,
                } => {
                    instance_for_events = started_instance;
                    if output == OutputFormat::Jsonl {
                        let _ = emit_call_value(
                            output,
                            json!({
                                "event": "execution.started",
                                "instance_id": instance_for_events,
                                "tool_name": tool_name,
                                "request_id": request_id,
                                "progress_token": null,
                                "cancellable": cancellation,
                            }),
                        );
                    }
                }
                crate::daemon::protocol::KernelEvent::Progress { progress, .. } => {
                    let _ = emit_call_progress(output, tool_name, &progress);
                }
                crate::daemon::protocol::KernelEvent::Finished { .. } => {
                    unreachable!("finished is the terminal frame")
                }
            },
        )
        .await
        .map_err(|error| call_error_from_store(error, instance_id, tool_name))?;

    let execution: McpToolExecution = if let Ok(execution) = serde_json::from_value(result.clone())
    {
        execution
    } else {
        let error = serde_json::from_value::<crate::daemon::protocol::KernelError>(result.clone())
            .map(|error| error.into_error())
            .unwrap_or_else(|_| {
                Error::new(
                    FailureCode::ToolFailed,
                    "tool execution returned an unreadable result",
                )
            });
        return Err(call_error_from_store(error, instance_id, tool_name));
    };
    finish_call_execution(a.output, instance_for_events, tool_name, execution)
}

fn call_elicitation_error(
    error: ElicitationCommandError,
    instance_id: InstanceId,
    tool_name: &str,
) -> mcpstore::Error {
    let code = match error.kind() {
        ElicitationErrorKind::InputRequired => {
            mcpstore::error::FailureCode::ElicitationInputRequired
        }
        ElicitationErrorKind::Cancelled => mcpstore::error::FailureCode::ElicitationCancelled,
        ElicitationErrorKind::TimedOut => mcpstore::error::FailureCode::ElicitationTimedOut,
        ElicitationErrorKind::InvalidResponse => {
            mcpstore::error::FailureCode::ElicitationInvalidResponse
        }
    };
    attach_tool(
        mcpstore::Error::new(code, error.message()),
        instance_id,
        tool_name,
    )
}

/// Resolve a call target to an instance ID. UUIDs are used directly; any other
/// value is treated as a service name and resolved within the requested scope.
fn scope_label(scope: &ScopeRef) -> &'static str {
    match scope {
        ScopeRef::Store => "store",
        ScopeRef::Agent { .. } => "agent",
    }
}

/// Cache key for the name→instance target cache; agents must not share entries,
/// so agent scopes are keyed by agent id.
fn scope_cache_key(scope: &ScopeRef) -> String {
    match scope {
        ScopeRef::Store => "store".to_string(),
        ScopeRef::Agent { agent_id } => format!("agent:{agent_id}"),
    }
}

#[derive(Debug)]
enum ResolveError {
    NotFound {
        scope_name: &'static str,
        target: String,
    },
    Backend(String),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { scope_name, target } => {
                write!(f, "service not found in {scope_name} scope: {target}")
            }
            Self::Backend(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolve a service name or instance UUID to an `InstanceId`. UUIDs bypass lookup;
/// names are resolved via ListServices（优先命中本地 schema 缓存）.
async fn resolve_target(
    access: &mut StoreAccess,
    scope: &ScopeRef,
    target: &str,
) -> Result<InstanceId, ResolveError> {
    if let Ok(instance_id) = InstanceId::from_str(target) {
        return Ok(instance_id);
    }
    let identity = access
        .cache_identity()
        .await
        .map_err(|e| ResolveError::Backend(e.to_string()))?;
    let scope_name = scope_label(scope);
    let cache_key = scope_cache_key(scope);
    if let Some(cached) = crate::schema_cache::load_target(&identity, &cache_key, target) {
        if let Ok(instance_id) = InstanceId::from_str(&cached) {
            return Ok(instance_id);
        }
    }
    let result = access
        .request(KernelOperation::ListServices, json!({ "scope": scope }))
        .await
        .map_err(|e| ResolveError::Backend(e.to_string()))?;
    let instance_id = result["services"].as_array().and_then(|services| {
        services
            .iter()
            .find(|service| service["service_name"].as_str() == Some(target))
            .and_then(|service| service["instance_id"].as_str())
            .map(str::to_string)
    });
    if let Some(id) = &instance_id {
        crate::schema_cache::save_target(&identity, &cache_key, target, id);
    }
    instance_id
        .and_then(|id| InstanceId::from_str(&id).ok())
        .ok_or_else(|| ResolveError::NotFound {
            scope_name,
            target: target.to_string(),
        })
}

fn resolve_error(e: ResolveError) -> mcpstore::Error {
    let code = match &e {
        ResolveError::NotFound { .. } => mcpstore::error::FailureCode::ServiceNotFound,
        ResolveError::Backend(_) => mcpstore::error::FailureCode::Internal,
    };
    mcpstore::Error::new(code, e.to_string())
}

/// Load the target tool's input schema so arguments can be positionally mapped,
/// defaulted, coerced, and validated. Returns `None` when the tool is not in the
/// available-tool set for the instance.
async fn load_tool_input_schema(
    access: &mut StoreAccess,
    instance_id: InstanceId,
    tool_name: &str,
) -> mcpstore::Result<Option<Value>> {
    let identity = access.cache_identity().await.map_err(|error| {
        call_error_from_store(
            mcpstore::Error::new(mcpstore::FailureCode::ServiceUnavailable, error.to_string()),
            instance_id,
            tool_name,
        )
    })?;
    if let Some(cached) = crate::schema_cache::load(&identity, &instance_id.to_string()) {
        if let Some(schema) = crate::schema_cache::find_schema(&cached, tool_name) {
            return Ok(Some(schema));
        }
    }
    let result = access
        .request(
            KernelOperation::ListTools,
            json!({"instance_id": instance_id.to_string()}),
        )
        .await
        .map_err(|error| call_error_from_store(error, instance_id, tool_name))?;
    let tools = result["tools"].as_array().cloned().unwrap_or_default();
    let tools_json: Vec<Value> = tools
        .iter()
        .map(|tool| json!({ "name": tool["name"], "schema": tool["schema"] }))
        .collect();
    crate::schema_cache::save(&identity, &instance_id.to_string(), &tools_json);
    Ok(tools
        .iter()
        .find(|tool| tool["name"].as_str() == Some(tool_name))
        .map(|tool| tool["schema"].clone()))
}

/// Merge the `--arguments` JSON base with trailing argument tokens, then apply
/// schema-driven positional mapping, defaults, type coercion, and required-field
/// validation when a schema is available.
fn build_call_arguments(
    raw_args: &[String],
    arguments_json: &str,
    schema: Option<&Value>,
    output: OutputFormat,
) -> mcpstore::Result<Value> {
    let mut object = parse_arguments_json_object(arguments_json, output)?;
    let (keyed, positional) = split_argument_tokens(raw_args);
    for (key, raw_value) in keyed {
        object.insert(key, coerce_value(&raw_value));
    }
    if !positional.is_empty() {
        return Err(mcpstore::Error::new(
            mcpstore::error::FailureCode::InvalidInput,
            "positional arguments are not supported; pass them as key:value or key=value",
        ));
    }
    if let Some(schema) = schema {
        apply_tool_schema(&mut object, schema, output)?;
    }
    Ok(Value::Object(object))
}

fn parse_arguments_json_object(
    arguments_json: &str,
    _output: OutputFormat,
) -> mcpstore::Result<Map<String, Value>> {
    let value: Value = serde_json::from_str(arguments_json).map_err(|error| {
        mcpstore::Error::new(
            mcpstore::error::FailureCode::InvalidInput,
            format!("invalid --arguments JSON: {error}"),
        )
    })?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(mcpstore::Error::new(
            mcpstore::error::FailureCode::InvalidInput,
            "--arguments must be a JSON object",
        )),
    }
}

/// Split trailing tokens into keyed (`key:value`, `key=value`, `--key=value`) and
/// positional values. A bare `--flag` without `=` falls through to positional.
fn split_argument_tokens(args: &[String]) -> (Vec<(String, String)>, Vec<String>) {
    let mut keyed = Vec::new();
    let mut positional = Vec::new();
    for raw in args {
        if let Some(rest) = raw.strip_prefix("--") {
            if let Some((key, value)) = rest.split_once('=') {
                keyed.push((key.to_string(), value.to_string()));
                continue;
            }
        }
        if let Some((key, value)) = raw.split_once(':') {
            keyed.push((key.to_string(), value.to_string()));
            continue;
        }
        if let Some((key, value)) = raw.split_once('=') {
            keyed.push((key.to_string(), value.to_string()));
            continue;
        }
        positional.push(raw.clone());
    }
    (keyed, positional)
}

/// Parse a raw token as JSON when it is valid (numbers, booleans, arrays, quoted
/// strings); otherwise keep it as a string.
fn coerce_value(raw: &str) -> Value {
    match serde_json::from_str::<Value>(raw) {
        Ok(value) => value,
        Err(_) => Value::String(raw.to_string()),
    }
}

/// Apply a tool's input schema: fill defaults, validate required fields, and
/// coerce string values to declared primitive types. Property iteration order is
/// not significant because every step is keyed.
fn apply_tool_schema(
    object: &mut Map<String, Value>,
    schema: &Value,
    _output: OutputFormat,
) -> mcpstore::Result<()> {
    let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) else {
        return Ok(());
    };

    for (key, spec) in properties.iter() {
        if !object.contains_key(key) {
            if let Some(default) = spec.get("default") {
                object.insert(key.clone(), default.clone());
            }
        }
    }

    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        let missing: Vec<&str> = required
            .iter()
            .filter_map(Value::as_str)
            .filter(|key| !object.contains_key(*key))
            .collect();
        if !missing.is_empty() {
            return Err(mcpstore::Error::new(
                mcpstore::error::FailureCode::InvalidInput,
                format!("missing required argument(s): {}", missing.join(", ")),
            ));
        }
    }

    for (key, spec) in properties.iter() {
        if let Some(value) = object.get(key) {
            if let Some(coerced) = coerce_value_to_schema(value, spec) {
                object.insert(key.clone(), coerced);
            }
        }
    }

    Ok(())
}

/// Coerce a value that parsed as a string into the schema's primitive type when
/// the schema declares integer, number, or boolean.
fn coerce_value_to_schema(value: &Value, spec: &Value) -> Option<Value> {
    let schema_type = spec.get("type")?.as_str()?;
    let raw = value.as_str()?;
    Some(match schema_type {
        "integer" => Value::from(raw.parse::<i64>().ok()?),
        "number" => Value::Number(serde_json::Number::from_f64(raw.parse::<f64>().ok()?)?),
        "boolean" => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => return None,
        },
        _ => return None,
    })
}

fn emit_call_started(
    output: OutputFormat,
    tool_name: &str,
    execution: &mcpstore::McpStoreToolExecutionHandle<'_>,
) -> mcpstore::Result<()> {
    if output != OutputFormat::Jsonl {
        return Ok(());
    }
    emit_call_value(
        output,
        json!({
            "event": "execution.started",
            "instance_id": execution.instance_id(),
            "tool_name": tool_name,
            "request_id": execution.request_id(),
            "progress_token": execution.progress_token(),
            "cancellable": execution.supports_cancellation(),
        }),
    )
}

fn emit_call_progress(
    output: OutputFormat,
    tool_name: &str,
    progress: &mcpstore::McpExecutionProgress,
) -> mcpstore::Result<()> {
    match output {
        OutputFormat::Human => {
            let amount = progress.total.map_or_else(
                || progress.progress.to_string(),
                |total| format!("{}/{}", progress.progress, total),
            );
            if let Some(message) = &progress.message {
                eprintln!("[Progress] {tool_name}: {amount} {message}");
            } else {
                eprintln!("[Progress] {tool_name}: {amount}");
            }
            Ok(())
        }
        OutputFormat::Json => Ok(()),
        OutputFormat::Jsonl => emit_call_value(
            output,
            json!({
                "event": "execution.progress",
                "instance_id": progress.instance_id,
                "tool_name": tool_name,
                "progress_token": progress.progress_token,
                "progress": progress.progress,
                "total": progress.total,
                "message": progress.message,
            }),
        ),
    }
}

fn emit_call_cancellation_requested(
    output: OutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
) -> mcpstore::Result<()> {
    match output {
        OutputFormat::Human => {
            eprintln!("[Cancellation requested] {tool_name}");
            Ok(())
        }
        OutputFormat::Json => Ok(()),
        OutputFormat::Jsonl => emit_call_value(
            output,
            json!({
                "event": "execution.cancellation_requested",
                "instance_id": instance_id,
                "tool_name": tool_name,
            }),
        ),
    }
}

fn finish_call_execution(
    output: OutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
    execution: McpToolExecution,
) -> mcpstore::Result<()> {
    let McpToolExecution::Immediate { result } = execution else {
        return Err(attach_tool(
            mcpstore::Error::new(
                mcpstore::error::FailureCode::ToolFailed,
                "tool call unexpectedly returned a task",
            ),
            instance_id,
            tool_name,
        ));
    };
    emit_call_result(output, instance_id, tool_name, &result)
}

/// Format a completed tool result for the chosen output format. Shared by the
/// local streaming path (`finish_call_execution`) and the daemon fast path.
fn emit_call_result(
    output: OutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
    result: &ToolCallResult,
) -> mcpstore::Result<()> {
    if result.is_error {
        return Err(attach_tool(
            mcpstore::Error::new(
                mcpstore::error::FailureCode::ToolFailed,
                tool_error_message(result),
            ),
            instance_id,
            tool_name,
        ));
    }
    match output {
        OutputFormat::Human => {
            print_tool_content(result);
            Ok(())
        }
        OutputFormat::Json | OutputFormat::Jsonl => emit_call_value(
            output,
            json!({
                "event": "execution.completed",
                "instance_id": instance_id,
                "tool_name": tool_name,
                "result": result,
            }),
        ),
    }
}

fn print_tool_content(result: &ToolCallResult) {
    for item in &result.content {
        match item {
            mcpstore::transport::ContentItem::Text { text, .. } => println!("{text}"),
            mcpstore::transport::ContentItem::Image { mime_type, .. } => {
                println!("[Image: {mime_type}]")
            }
            mcpstore::transport::ContentItem::Audio { mime_type, .. } => {
                println!("[Audio: {mime_type}]")
            }
            mcpstore::transport::ContentItem::Resource { resource, .. } => {
                println!("[Resource: {resource}]")
            }
            mcpstore::transport::ContentItem::ResourceLink { resource, .. } => {
                println!("[ResourceLink: {resource}]")
            }
        }
    }
}

fn tool_error_message(result: &ToolCallResult) -> String {
    result
        .content
        .iter()
        .find_map(|item| match item {
            mcpstore::transport::ContentItem::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "tool returned an error result".to_string())
}

pub(crate) fn emit_call_value(output: OutputFormat, value: Value) -> mcpstore::Result<()> {
    let encoded = match output {
        OutputFormat::Human => Ok(value.to_string()),
        OutputFormat::Json => serde_json::to_string_pretty(&value),
        OutputFormat::Jsonl => serde_json::to_string(&value),
    }
    .map_err(|error| {
        mcpstore::Error::new(
            mcpstore::error::FailureCode::Internal,
            format!("failed to encode call output: {error}"),
        )
    })?;
    println!("{encoded}");
    Ok(())
}

pub async fn migrate_store(
    a: MigrateStoreArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let mut config = json!({});
    if let Some(target_config) = a.target_config {
        config = json!({"config": target_config});
    }
    let result = access
        .request(
            KernelOperation::SwapStore,
            json!({"store": a.target_store, "config": config}),
        )
        .await?;

    println!(
        "[Success] Cache storage hot migration completed: target={} entries={}",
        result["target_store"].as_str().unwrap_or("?"),
        result["copied"]
    );
    Ok(())
}

#[derive(Args)]
pub struct AssignArgs {
    #[arg(help = "Service name")]
    pub service_name: String,
    #[arg(long, help = "Agent ID")]
    pub agent: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Args)]
pub struct UnassignArgs {
    #[arg(help = "Service name")]
    pub service_name: String,
    #[arg(long, help = "Agent ID")]
    pub agent: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn assign(
    a: AssignArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = ScopeRef::Agent {
        agent_id: a.agent.clone(),
    };
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let result = access
        .request(
            KernelOperation::DeclareServiceScope,
            json!({
                "service_name": a.service_name,
                "scope": scope,
                "descriptor": ScopeDescriptor::default(),
            }),
        )
        .await?;
    print_mutation(
        &format!(
            "Service authorized to Agent: agent={} service={}",
            a.agent, a.service_name
        ),
        &result,
    );
    Ok(())
}

pub async fn unassign(
    a: UnassignArgs,
    embedded: bool,
    endpoint: Option<crate::daemon::client::DaemonEndpoint>,
) -> std::result::Result<(), BoxErr> {
    let scope = ScopeRef::Agent {
        agent_id: a.agent.clone(),
    };
    let mut access = open_store(&a.store, embedded, endpoint.clone()).await?;
    let result = access
        .request(
            KernelOperation::RemoveServiceScope,
            json!({"service_name": a.service_name, "scope": scope}),
        )
        .await?;
    print_mutation(
        &format!(
            "Removed Agent service authorization: agent={} service={}",
            a.agent, a.service_name
        ),
        &result,
    );
    Ok(())
}

fn parse_env(env: &[String]) -> std::result::Result<HashMap<String, String>, BoxErr> {
    parse_key_values(env, "env var")
}

fn parse_headers(headers: &[String]) -> std::result::Result<HashMap<String, String>, BoxErr> {
    parse_key_values(headers, "header")
}

fn parse_key_values(
    items: &[String],
    label: &str,
) -> std::result::Result<HashMap<String, String>, BoxErr> {
    let mut map = HashMap::new();
    for item in items {
        let (k, v) = item
            .split_once('=')
            .ok_or_else(|| format!("{label} format error: {item}"))?;
        if k.is_empty() {
            return Err(format!("{label} key cannot be empty: {item}").into());
        }
        map.insert(k.to_string(), v.to_string());
    }
    Ok(map)
}

fn runtime_policy_from_flags(
    allowed_runtimes: &[mcpstore::config::Runtime],
    allowed_daemons: &[String],
    required_host_capabilities: &[String],
) -> Option<RuntimePolicy> {
    (!allowed_runtimes.is_empty()
        || !allowed_daemons.is_empty()
        || !required_host_capabilities.is_empty())
    .then(|| RuntimePolicy {
        allowed_runtimes: allowed_runtimes.to_vec(),
        allowed_daemons: allowed_daemons.to_vec(),
        required_host_capabilities: required_host_capabilities.to_vec(),
    })
}

fn build_server_config(
    command_or_url: Option<&str>,
    args: &[String],
    transport: Option<&str>,
    env_map: &HashMap<String, String>,
    header_map: &HashMap<String, String>,
) -> std::result::Result<ServerConfig, BoxErr> {
    let command_or_url = command_or_url.ok_or_else(|| {
        "Missing service entry: Streamable HTTP requires URL, stdio requires command".to_string()
    })?;
    let is_url = command_or_url.starts_with("http://") || command_or_url.starts_with("https://");

    let resolved_transport = transport
        .map(|t| match t {
            "http" => "streamable-http",
            other => other,
        })
        .unwrap_or(if is_url { "streamable-http" } else { "stdio" })
        .to_string();

    if resolved_transport == "sse" {
        return Err("Unsupported transport type: sse".into());
    }

    if resolved_transport == "streamable-http" && !is_url {
        return Err(format!(
            "{} service http:// or https:// URL required: {}",
            resolved_transport, command_or_url
        )
        .into());
    }

    if resolved_transport != "stdio" && is_url {
        Ok(ServerConfig {
            url: Some(command_or_url.to_string()),
            command: None,
            args: Vec::new(),
            env: env_map.clone(),
            headers: header_map.clone(),
            auth: Default::default(),
            transport: Some(resolved_transport),
            working_dir: None,
            description: None,
            mcpstore: None,
            extra: Default::default(),
        })
    } else {
        Ok(ServerConfig {
            url: None,
            command: Some(command_or_url.to_string()),
            args: args.to_vec(),
            env: env_map.clone(),
            headers: header_map.clone(),
            auth: Default::default(),
            transport: Some(resolved_transport),
            working_dir: None,
            description: None,
            mcpstore: None,
            extra: Default::default(),
        })
    }
}

pub(crate) fn parse_instance_id(value: &str) -> std::result::Result<InstanceId, BoxErr> {
    Ok(InstanceId::from_str(value)?)
}

fn format_capabilities(metadata: Option<&Value>) -> String {
    let Some(capabilities) = metadata.and_then(|metadata| metadata.get("capabilities")) else {
        return "unknown".to_string();
    };
    let mut enabled = Vec::new();
    for (name, present) in [
        ("tools", capabilities["tools"].as_bool().unwrap_or(false)),
        (
            "tools.list_changed",
            capabilities["tools_list_changed"]
                .as_bool()
                .unwrap_or(false),
        ),
        (
            "resources",
            capabilities["resources"].as_bool().unwrap_or(false),
        ),
        (
            "resources.list_changed",
            capabilities["resources_list_changed"]
                .as_bool()
                .unwrap_or(false),
        ),
        (
            "prompts",
            capabilities["prompts"].as_bool().unwrap_or(false),
        ),
        (
            "prompts.list_changed",
            capabilities["prompts_list_changed"]
                .as_bool()
                .unwrap_or(false),
        ),
        (
            "completions",
            capabilities["completions"].as_bool().unwrap_or(false),
        ),
        ("tasks", capabilities["tasks"].as_bool().unwrap_or(false)),
        (
            "extensions",
            capabilities["extensions"]
                .as_array()
                .is_some_and(|list| !list.is_empty()),
        ),
        (
            "experimental",
            capabilities["experimental"]
                .as_array()
                .is_some_and(|list| !list.is_empty()),
        ),
    ] {
        if present {
            enabled.push(name);
        }
    }
    if enabled.is_empty() {
        "none".to_string()
    } else {
        enabled.join(",")
    }
}

fn require_agent(agent: Option<&str>) -> std::result::Result<&str, BoxErr> {
    agent
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "--agent is required when using --scope agent".into())
}

fn validate_agent_flag(scope: &Scope, agent: Option<&str>) -> std::result::Result<(), BoxErr> {
    if *scope != Scope::Agent && agent.is_some() {
        return Err("--agent can only be used with --scope agent".into());
    }
    Ok(())
}

fn validate_scope_target(scope: &Scope, agent: Option<&str>) -> std::result::Result<(), BoxErr> {
    validate_agent_flag(scope, agent)?;
    if *scope == Scope::Agent {
        require_agent(agent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_execution_rejects_missing_host_capability() {
        let policy = RuntimePolicy {
            allowed_runtimes: Vec::new(),
            allowed_daemons: Vec::new(),
            required_host_capabilities: vec!["definitely-missing-capability".into()],
        };
        let error =
            ensure_host_capabilities(&policy, mcpstore::config::Runtime::Local).unwrap_err();
        assert!(error.to_string().contains("definitely-missing-capability"));
    }

    #[test]
    fn daemon_selection_passes_allowlist() {
        let policy = mcpstore::config::RuntimePolicy {
            allowed_runtimes: vec![mcpstore::config::Runtime::Daemon],
            allowed_daemons: Vec::new(),
            required_host_capabilities: Vec::new(),
        };
        let info = json!({"runtime_policy": policy});
        let selection = resolve_declared_runtime(
            &info,
            mcpstore::config::RuntimeSelection::runtime(mcpstore::config::Runtime::Daemon),
        )
        .unwrap();
        assert_eq!(selection.runtime, mcpstore::config::Runtime::Daemon);
    }

    #[test]
    fn disallowed_daemon_node_is_rejected() {
        let policy = mcpstore::config::RuntimePolicy {
            allowed_runtimes: Vec::new(),
            allowed_daemons: vec!["approved".into()],
            required_host_capabilities: Vec::new(),
        };
        let info = json!({"runtime_policy": policy});
        let error = resolve_declared_runtime(
            &info,
            mcpstore::config::RuntimeSelection::daemon_node("other"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("not allowed"), "{error}");
    }

    #[test]
    fn explicit_disallowed_runtime_is_rejected() {
        let policy = mcpstore::config::RuntimePolicy {
            allowed_runtimes: vec![mcpstore::config::Runtime::Local],
            allowed_daemons: Vec::new(),
            required_host_capabilities: Vec::new(),
        };
        let info = json!({"runtime_policy": policy});
        let error = resolve_declared_runtime(
            &info,
            mcpstore::config::RuntimeSelection::runtime(mcpstore::config::Runtime::Daemon),
        )
        .unwrap_err();
        assert!(error.to_string().contains("not allowed"), "{error}");
    }

    #[tokio::test]
    async fn update_changes_runtime_policy_on_store_scope_only() {
        let path =
            std::env::temp_dir().join(format!("mcpstore-update-policy-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let config_path = path.join("mcp.json");
        let add_args = AddArgs {
            name: "browser".into(),
            command_or_url: Some("echo".into()),
            args: vec!["fixture".into()],
            transport: Some("stdio".into()),
            store: StoreSourceArgs {
                config_path: Some(config_path.to_str().unwrap().into()),
                source: crate::store_args::SourceArg::Local,
                store: None,
                store_config: None,
                namespace: None,
                node_mode: None,
            },
            env: Vec::new(),
            header: Vec::new(),
            scope: Scope::Store,
            agent: None,
            handshake: None,
            require_host_capability: Vec::new(),
            allow_runtime: vec![mcpstore::config::Runtime::Local],
            allow_daemon: Vec::new(),
        };
        add(add_args, true, None).await.unwrap();

        let update_args = UpdateArgs {
            name: "browser".into(),
            command_or_url: Some("echo".into()),
            args: vec!["changed".into()],
            transport: Some("stdio".into()),
            store: StoreSourceArgs {
                config_path: Some(config_path.to_str().unwrap().into()),
                source: crate::store_args::SourceArg::Local,
                store: None,
                store_config: None,
                namespace: None,
                node_mode: None,
            },
            env: Vec::new(),
            header: Vec::new(),
            scope: Scope::Store,
            agent: None,
            require_host_capability: Vec::new(),
            allow_runtime: vec![mcpstore::config::Runtime::Daemon],
            allow_daemon: Vec::new(),
        };
        update(update_args, true, None).await.unwrap();

        let update_args = UpdateArgs {
            name: "browser".into(),
            command_or_url: Some("echo".into()),
            args: vec!["preserved".into()],
            transport: Some("stdio".into()),
            store: StoreSourceArgs {
                config_path: Some(config_path.to_str().unwrap().into()),
                source: crate::store_args::SourceArg::Local,
                store: None,
                store_config: None,
                namespace: None,
                node_mode: None,
            },
            env: Vec::new(),
            header: Vec::new(),
            scope: Scope::Store,
            agent: None,
            require_host_capability: Vec::new(),
            allow_runtime: Vec::new(),
            allow_daemon: Vec::new(),
        };
        update(update_args, true, None).await.unwrap();

        let store = mcpstore::MCPStore::setup(Some(config_path.to_str().unwrap())).unwrap();
        store.load_from_source().await.unwrap();
        let policy = store
            .find_definition("browser")
            .await
            .unwrap()
            .runtime_policy
            .unwrap();
        assert_eq!(
            policy.allowed_runtimes,
            vec![mcpstore::config::Runtime::Daemon]
        );
        std::fs::remove_dir_all(path).ok();
    }

    #[tokio::test]
    async fn update_rejects_runtime_policy_on_agent_scope() {
        let args = UpdateArgs {
            name: "browser".into(),
            command_or_url: Some("echo".into()),
            args: Vec::new(),
            transport: Some("stdio".into()),
            store: StoreSourceArgs {
                config_path: None,
                source: crate::store_args::SourceArg::Local,
                store: None,
                store_config: None,
                namespace: None,
                node_mode: None,
            },
            env: Vec::new(),
            header: Vec::new(),
            scope: Scope::Agent,
            agent: Some("agent".into()),
            require_host_capability: Vec::new(),
            allow_runtime: vec![mcpstore::config::Runtime::Local],
            allow_daemon: Vec::new(),
        };
        let error = update(args, true, None).await.unwrap_err().to_string();
        assert!(error.contains("definition-level"), "{error}");
    }

    #[tokio::test]
    async fn add_declares_runtime_policy() {
        let path = std::env::temp_dir().join(format!("mcpstore-add-policy-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let config_path = path.join("mcp.json");
        let args = AddArgs {
            name: "browser".into(),
            command_or_url: Some("echo".into()),
            args: vec!["fixture".into()],
            transport: Some("stdio".into()),
            store: StoreSourceArgs {
                config_path: Some(config_path.to_str().unwrap().into()),
                source: crate::store_args::SourceArg::Local,
                store: None,
                store_config: None,
                namespace: None,
                node_mode: None,
            },
            env: Vec::new(),
            header: Vec::new(),
            scope: Scope::Store,
            agent: None,
            handshake: None,
            require_host_capability: Vec::new(),
            allow_runtime: vec![mcpstore::config::Runtime::Local],
            allow_daemon: Vec::new(),
        };
        add(args, true, None).await.unwrap();
        let store = mcpstore::MCPStore::setup(Some(config_path.to_str().unwrap())).unwrap();
        store.load_from_source().await.unwrap();
        let definition = store.find_definition("browser").await.unwrap();
        assert_eq!(
            definition
                .runtime_policy
                .map(|policy| policy.allowed_runtimes),
            Some(vec![mcpstore::config::Runtime::Local])
        );
        std::fs::remove_dir_all(path).ok();
    }

    #[test]
    fn capability_summary_reports_protocol_features() {
        let metadata = json!({
            "protocol_version": "2026-07-28",
            "capabilities": {
                "tools": true,
                "tools_list_changed": false,
                "resources": true,
                "resources_list_changed": false,
                "prompts": true,
                "prompts_list_changed": false,
                "completions": true,
                "tasks": false,
                "extensions": [],
                "experimental": [],
            },
        });
        assert_eq!(
            format_capabilities(Some(&metadata)),
            "tools,resources,prompts,completions"
        );
        assert_eq!(format_capabilities(None), "unknown");
    }

    #[test]
    fn call_arguments_require_a_json_object() {
        assert_eq!(
            parse_arguments_json_object(r#"{"value":1}"#, OutputFormat::Human).unwrap()["value"],
            1
        );
        let error = parse_arguments_json_object("[]", OutputFormat::Jsonl).unwrap_err();
        assert_eq!(error.code(), FailureCode::InvalidInput);
        assert_eq!(crate::error::exit_code(error.code()), 2);
        let value: Value =
            serde_json::from_str(&crate::error::render(&error, OutputFormat::Json)).unwrap();
        assert_eq!(value["error"]["code"], "invalid_input");
        assert!(value.get("event").is_none());
    }

    #[test]
    fn call_errors_keep_their_failure_codes() {
        let instance_id: InstanceId = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();
        for (error, code, exit) in [
            (
                Error::new(
                    FailureCode::CallCancelled,
                    "MCP request cancelled: cancelled",
                ),
                FailureCode::CallCancelled,
                44,
            ),
            (
                Error::new(FailureCode::CallTimedOut, "MCP request timed out after 1s"),
                FailureCode::CallTimedOut,
                43,
            ),
            (
                Error::new(
                    FailureCode::CallDisconnected,
                    format!("MCP request disconnected for service instance {instance_id}"),
                ),
                FailureCode::CallDisconnected,
                45,
            ),
        ] {
            let error = attach_tool(error, instance_id, "long_tool");
            assert_eq!(error.code(), code);
            assert_eq!(crate::error::exit_code(error.code()), exit);
            let v: Value =
                serde_json::from_str(&crate::error::render(&error, OutputFormat::Json)).unwrap();
            assert!(v.get("event").is_none());
            assert_eq!(v["error"]["code"], code.as_str());
        }
    }

    #[test]
    fn parse_key_values_rejects_missing_separator() {
        let err = parse_env(&["TOKEN".to_string()]).unwrap_err().to_string();
        assert!(err.contains("env var format error"));
    }

    #[test]
    fn http_config_keeps_env_and_headers_separate() {
        let env = parse_env(&["LOCAL_TOKEN=abc".to_string()]).unwrap();
        let headers = parse_headers(&["Authorization=Bearer token".to_string()]).unwrap();

        let config = build_server_config(
            Some("https://api.example.com/mcp"),
            &[],
            Some("http"),
            &env,
            &headers,
        )
        .unwrap();

        assert_eq!(config.url.as_deref(), Some("https://api.example.com/mcp"));
        assert_eq!(config.transport.as_deref(), Some("streamable-http"));
        assert_eq!(
            config.env.get("LOCAL_TOKEN").map(String::as_str),
            Some("abc")
        );
        assert_eq!(
            config.headers.get("Authorization").map(String::as_str),
            Some("Bearer token")
        );
    }

    #[test]
    fn stdio_config_preserves_command_args_env_and_headers() {
        let env = parse_env(&["TOKEN=abc".to_string()]).unwrap();
        let headers = parse_headers(&["X-Debug=1".to_string()]).unwrap();
        let args = vec!["-y".to_string(), "server".to_string()];

        let config =
            build_server_config(Some("npx"), &args, Some("stdio"), &env, &headers).unwrap();

        assert_eq!(config.command.as_deref(), Some("npx"));
        assert_eq!(config.args, args);
        assert_eq!(config.env.get("TOKEN").map(String::as_str), Some("abc"));
        assert_eq!(config.headers.get("X-Debug").map(String::as_str), Some("1"));
    }

    #[test]
    fn remote_transport_requires_url() {
        let err = build_server_config(
            Some("npx"),
            &[],
            Some("http"),
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("http:// or https:// URL required"));
    }

    #[test]
    fn sse_transport_is_rejected_during_config_building() {
        let err = build_server_config(
            Some("https://api.example.com/sse"),
            &[],
            Some("sse"),
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap_err()
        .to_string();

        assert_eq!(err, "Unsupported transport type: sse");
    }

    #[test]
    fn agent_scope_requires_agent_id() {
        let err = require_agent(None).unwrap_err().to_string();
        assert!(err.contains("--agent is required"));
    }

    #[test]
    fn agent_flag_requires_agent_scope() {
        let err = validate_agent_flag(&Scope::Store, Some("agent1"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("can only be used with --scope agent"));
    }

    #[test]
    fn validate_scope_target_rejects_agent_scope_without_agent() {
        let err = validate_scope_target(&Scope::Agent, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("--agent is required"));
    }

    fn json_schema(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn scope_cache_key_separates_agents() {
        let store = scope_cache_key(&ScopeRef::Store);
        let agent_x = scope_cache_key(&ScopeRef::Agent {
            agent_id: "x".into(),
        });
        let agent_y = scope_cache_key(&ScopeRef::Agent {
            agent_id: "y".into(),
        });
        assert_eq!(store, "store");
        assert_eq!(agent_x, "agent:x");
        assert_ne!(agent_x, agent_y);
    }

    #[test]
    fn split_argument_tokens_classifies_keyed_and_positional() {
        let args: Vec<String> = [
            "owner:ip2a",
            "repo=mcp/store",
            "--draft=true",
            "bare",
            "--flag",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let (keyed, positional) = split_argument_tokens(&args);
        assert_eq!(
            keyed,
            vec![
                ("owner".to_string(), "ip2a".to_string()),
                ("repo".to_string(), "mcp/store".to_string()),
                ("draft".to_string(), "true".to_string()),
            ]
        );
        assert_eq!(positional, vec!["bare".to_string(), "--flag".to_string()]);
    }

    #[test]
    fn coerce_value_parses_json_or_keeps_string() {
        assert_eq!(coerce_value("5"), Value::from(5));
        assert_eq!(coerce_value("true"), Value::Bool(true));
        assert_eq!(coerce_value("ip2a"), Value::String("ip2a".to_string()));
        assert_eq!(
            coerce_value("[1, 2]"),
            Value::Array(vec![Value::from(1), Value::from(2)])
        );
    }

    #[test]
    fn build_call_arguments_without_schema_merges_keyed_and_base() {
        let built = build_call_arguments(
            &["owner:ip2a".to_string()],
            r#"{"repo":"mcpstore"}"#,
            None,
            OutputFormat::Human,
        )
        .unwrap();
        assert_eq!(built["owner"], "ip2a");
        assert_eq!(built["repo"], "mcpstore");
    }

    #[test]
    fn build_call_arguments_fills_defaults_and_coerces() {
        let schema = json_schema(
            r#"{
                "type": "object",
                "properties": {
                    "owner": {"type": "string"},
                    "count": {"type": "integer", "default": 1},
                    "verified": {"type": "boolean"}
                },
                "required": ["owner"]
            }"#,
        );

        // `count` omitted → default 1 applied.
        let defaulted = build_call_arguments(
            &["owner:ip2a".to_string()],
            "{}",
            Some(&schema),
            OutputFormat::Human,
        )
        .unwrap();
        assert_eq!(defaulted["owner"], "ip2a");
        assert_eq!(defaulted["count"], 1);

        // base string values coerced to declared primitive types.
        let coerced = build_call_arguments(
            &[],
            r#"{"owner":"x","count":"42","verified":"true"}"#,
            Some(&schema),
            OutputFormat::Human,
        )
        .unwrap();
        assert_eq!(coerced["count"], 42);
        assert_eq!(coerced["verified"], true);
    }

    #[test]
    fn build_call_arguments_reports_missing_required() {
        let schema = json_schema(
            r#"{"type":"object","properties":{"owner":{"type":"string"}},"required":["owner"]}"#,
        );
        let err = build_call_arguments(&[], "{}", Some(&schema), OutputFormat::Human)
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing required"), "{err}");
    }

    #[test]
    fn build_call_arguments_rejects_positional() {
        let err = build_call_arguments(&["lonely".to_string()], "{}", None, OutputFormat::Human)
            .unwrap_err()
            .to_string();
        assert!(err.contains("positional"), "{err}");
    }

    #[test]
    fn call_error_human_output_includes_hint() {
        let error = mcpstore::Error::new(
            mcpstore::error::FailureCode::ServiceNotFound,
            "service not found: github".to_string(),
        );
        let rendered = crate::error::render(&error, OutputFormat::Human);
        assert!(rendered.contains("service_not_found"), "{rendered}");
        assert!(rendered.contains("hint:"), "{rendered}");
        assert!(rendered.contains("mcpstore list"), "{rendered}");
    }
}
