use clap::{Args, ValueEnum};
use mcpstore::config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig};
use mcpstore::transport::TransportError;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use mcpstore::{
    InstanceId, McpExecutionOptions, McpServerCapabilities, McpServerMetadata, MCPStore,
    McpStoreExecutionUpdate, McpToolExecution, ScopeRef, StoreError, ToolCallResult,
};

use crate::{
    commands::elicitation::{
        handle_elicitation, settle_execution_after_elicitation_error, ElicitationArgs,
        ElicitationCommandError, ElicitationErrorKind, ElicitationOutputFormat,
    },
    store_args::{build_store, CacheStorageArg, StoreSourceArgs},
    BoxErr,
};

#[derive(Clone, Debug, Eq, PartialEq, ValueEnum, Default)]
pub enum Scope {
    #[default]
    Store,
    Agent,
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
}

pub async fn add(a: AddArgs) -> std::result::Result<(), BoxErr> {
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

    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({
            "name": a.name,
            "config": config,
            "scope": scope,
        });
        crate::daemon::client::call_daemon("add_service", params).await?;
        println!(
            "[Success] Service added: {} (transport={})",
            a.name, transport
        );
        return Ok(());
    }

    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let definition_exists = store.get_definition_config(&a.name).await?.is_some();
    if definition_exists {
        let lifecycle = config
            .mcpstore
            .as_ref()
            .and_then(|extension| extension.lifecycle.clone());
        store
            .declare_service_scope(
                &a.name,
                &scope,
                ScopeDescriptor {
                    config: config.base_config(),
                    lifecycle,
                    revision: 0,
                },
            )
            .await?;
    } else {
        store.add_service(&a.name, config).await?;
    }
    println!(
        "[Success] Service added: {} (transport={})",
        a.name, transport
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

pub async fn add_json(a: AddJsonArgs) -> std::result::Result<(), BoxErr> {
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
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
    let definition_exists = store.get_definition_config(&a.name).await?.is_some();
    if definition_exists {
        let lifecycle = config
            .mcpstore
            .as_ref()
            .and_then(|extension| extension.lifecycle.clone());
        store
            .declare_service_scope(
                &a.name,
                &scope,
                ScopeDescriptor {
                    config: config.base_config(),
                    lifecycle,
                    revision: 0,
                },
            )
            .await?;
    } else {
        store.add_service(&a.name, config).await?;
    }
    println!(
        "[Success] Service added: {} (transport={})",
        a.name, transport
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
    #[arg(long, help = "Emit machine-readable JSON")]
    pub json: bool,
}

/// Collect service summaries (name, instance, transport, readiness, tool count)
/// from the daemon when running, otherwise from a local store. Used by the
/// machine-readable `list --json` view.
async fn load_service_summaries(
    store_args: &StoreSourceArgs,
    scope: &ScopeRef,
) -> std::result::Result<Vec<Value>, BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let result =
            crate::daemon::client::call_daemon("list_services", json!({ "scope": scope })).await?;
        return Ok(result
            .get("services")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|svc| {
                json!({
                    "service_name": svc.get("service_name").and_then(Value::as_str).unwrap_or(""),
                    "instance_id": svc.get("instance_id").and_then(Value::as_str).unwrap_or(""),
                    "transport": svc.get("transport").and_then(Value::as_str).unwrap_or(""),
                    "readiness": svc.pointer("/state/readiness/status").and_then(Value::as_str).unwrap_or(""),
                    "tools_count": svc.get("tools_count").and_then(|v| v.as_u64()).unwrap_or(0),
                })
            })
            .collect());
    }
    let store = build_store(store_args)?;
    store.load_from_source().await?;
    let services = store.list_scope_instances(scope).await?;
    let mut out = Vec::with_capacity(services.len());
    for svc in services {
        let state = store.service_state_entry(svc.instance_id).await?;
        out.push(json!({
            "service_name": svc.service_name,
            "instance_id": svc.instance_id,
            "transport": svc.transport,
            "readiness": state.readiness.status,
            "tools_count": svc.tools.len(),
        }));
    }
    Ok(out)
}

pub async fn list(a: ListArgs) -> std::result::Result<(), BoxErr> {
    let scope = a.scope.to_ref(a.agent.as_deref())?;

    if a.json {
        let services = load_service_summaries(&a.store, &scope).await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "services": services, "total": services.len() }))?
        );
        return Ok(());
    }

    if crate::daemon::client::daemon_socket_exists() {
        let result = crate::daemon::client::call_daemon(
            "list_services",
            serde_json::json!({"scope": scope}),
        )
        .await?;
        let services = result
            .get("services")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        println!("[List] service_count={}", services.len());
        if services.is_empty() {
            println!("  No services available");
            return Ok(());
        }
        for svc in services {
            let name = svc
                .get("service_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let instance_id = svc
                .get("instance_id")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let transport = svc.get("transport").and_then(|v| v.as_str()).unwrap_or("?");
            let readiness = svc
                .pointer("/state/readiness/status")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let tools_count = svc.get("tools_count").and_then(|v| v.as_u64()).unwrap_or(0);
            let capabilities = svc
                .get("mcp")
                .cloned()
                .and_then(|value| serde_json::from_value::<Option<McpServerMetadata>>(value).ok())
                .flatten();
            println!(
                "- {}  instance={}  transport={}  readiness={}  tools={}  capabilities={}",
                name,
                instance_id,
                transport,
                readiness,
                tools_count,
                format_capabilities(capabilities.as_ref())
            );
        }
        return Ok(());
    }

    let store = build_store(&a.store)?;
    store.load_from_source().await?;

    let services = store.list_scope_instances(&scope).await?;
    println!("[List] service_count={}", services.len());

    if services.is_empty() {
        println!("  No services available");
        return Ok(());
    }

    for svc in &services {
        let state = store.service_state_entry(svc.instance_id).await?;
        let metadata = store.mcp_server_metadata(svc.instance_id).await?;
        println!(
            "- {}  instance={}  transport={}  readiness={:?}  phase={:?}  health={:?}  tools={}  capabilities={}",
            svc.service_name,
            svc.instance_id,
            svc.transport,
            state.readiness.status,
            state.phase,
            state.health,
            svc.tools.len(),
            format_capabilities(metadata.as_ref())
        );
    }
    Ok(())
}

#[derive(Args)]
pub struct GetArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn get(a: GetArgs) -> std::result::Result<(), BoxErr> {
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let payload = store
        .service_info_scoped(parse_instance_id(&a.instance_id)?)
        .await?;
    let json = serde_json::to_string_pretty(&payload)?;
    println!("{json}");
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

pub async fn remove(a: RemoveArgs) -> std::result::Result<(), BoxErr> {
    let scope = a.scope.to_ref(a.agent.as_deref())?;
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"service_name": a.name, "scope": scope});
        crate::daemon::client::call_daemon("remove_service_scope", params).await?;
        println!("[Success] Service scope removed: {}", a.name);
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store.remove_service_scope(&a.name, &scope).await?;
    println!("[Success] Service scope removed: {}", a.name);
    Ok(())
}

#[derive(Args)]
pub struct ConnectArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn connect(a: ConnectArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"instance_id": a.instance_id});
        let result = crate::daemon::client::call_daemon("connect_service", params).await?;
        let tools_count = result
            .get("tools_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let capabilities = result
            .get("mcp")
            .cloned()
            .and_then(|value| serde_json::from_value::<Option<McpServerMetadata>>(value).ok())
            .flatten();
        println!(
            "[Success] Connected: {} (tools={}, capabilities={})",
            a.instance_id,
            tools_count,
            format_capabilities(capabilities.as_ref())
        );
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let instance_id = parse_instance_id(&a.instance_id)?;
    store.connect_service(instance_id).await?;

    let tools = store
        .list_tool_entries_for_instance_with_filter(
            instance_id,
            mcpstore::ToolVisibilityFilter::Available,
        )
        .await
        .unwrap_or_default();
    let metadata = store.mcp_server_metadata(instance_id).await?;
    println!(
        "[Success] Connected: {} (tools={}, capabilities={})",
        instance_id,
        tools.len(),
        format_capabilities(metadata.as_ref())
    );
    for t in &tools {
        println!("  - {}: {}", t.name, t.description);
    }
    Ok(())
}

#[derive(Args)]
pub struct DisconnectArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn disconnect(a: DisconnectArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"instance_id": a.instance_id});
        crate::daemon::client::call_daemon("disconnect_service", params).await?;
        println!("[Success] Disconnected: {}", a.instance_id);
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let instance_id = parse_instance_id(&a.instance_id)?;
    store.disconnect_service(instance_id).await?;
    println!("[Success] Disconnected: {}", instance_id);
    Ok(())
}

#[derive(Args)]
pub struct RestartArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn restart(a: RestartArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"instance_id": a.instance_id});
        crate::daemon::client::call_daemon("restart_service", params).await?;
        println!("[Success] Restarted: {}", a.instance_id);
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let instance_id = parse_instance_id(&a.instance_id)?;
    store.restart_service(instance_id).await?;
    println!("[Success] Restarted: {}", instance_id);
    Ok(())
}

#[derive(Args)]
pub struct CheckArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, help = "Exit 0 when ready, non-zero otherwise")]
    pub exit_code: bool,
    #[arg(long, help = "Suppress output; signal readiness only via the exit code")]
    pub quiet: bool,
}

pub async fn check(a: CheckArgs) -> std::result::Result<(), BoxErr> {
    let instance_id = parse_instance_id(&a.instance_id)?;
    let (ready, label) = if crate::daemon::client::daemon_socket_exists() {
        let result = crate::daemon::client::call_daemon(
            "check_service",
            json!({ "instance_id": instance_id }),
        )
        .await?;
        let readiness = result
            .pointer("/state/readiness/status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let phase = result
            .pointer("/state/phase")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        (
            readiness == "ready",
            format!("{instance_id} => readiness={readiness} phase={phase}"),
        )
    } else {
        let store = build_store(&a.store)?;
        store.load_from_source().await?;
        let status = store.service_state_entry(instance_id).await?;
        (
            status.readiness.status == mcpstore::ReadinessStatus::Ready,
            format!(
                "{instance_id} => readiness={:?} phase={:?} health={:?}",
                status.readiness.status, status.phase, status.health
            ),
        )
    };

    if !a.quiet {
        println!("[Check] {label}");
    }
    if a.exit_code {
        std::process::exit(i32::from(!ready));
    }
    Ok(())
}

#[derive(Args)]
pub struct WaitArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[arg(long, default_value_t = 30, help = "Wait timeout in seconds")]
    pub timeout: u64,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn wait(a: WaitArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"instance_id": a.instance_id, "timeout": a.timeout});
        let result = crate::daemon::client::call_daemon("wait_service", params).await?;
        let readiness = result
            .pointer("/state/readiness/status")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        println!("[Success] Service ready: {} ({})", a.instance_id, readiness);
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let instance_id = parse_instance_id(&a.instance_id)?;
    store.connect_service(instance_id).await?;
    let status = store
        .wait_instance_ready(instance_id, std::time::Duration::from_secs(a.timeout))
        .await?;
    println!(
        "[Success] Service ready: {} (readiness={:?}, health={:?})",
        instance_id, status.readiness.status, status.health
    );
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
}

pub async fn update(a: UpdateArgs) -> std::result::Result<(), BoxErr> {
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
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
    match a.scope.to_ref(a.agent.as_deref())? {
        ScopeRef::Store => store.update_service(&a.name, config).await?,
        scope @ ScopeRef::Agent { .. } => {
            store
                .declare_service_scope(
                    &a.name,
                    &scope,
                    ScopeDescriptor {
                        config: config.base_config(),
                        lifecycle: None,
                        revision: 0,
                    },
                )
                .await?;
        }
    }
    println!("[Success] Service updated: {}", a.name);
    Ok(())
}

#[derive(Args)]
pub struct ToolsArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, help = "Emit machine-readable JSON")]
    pub json: bool,
    #[arg(long, help = "Include each tool's input schema")]
    pub schema: bool,
}

pub async fn tools(a: ToolsArgs) -> std::result::Result<(), BoxErr> {
    let instance_id = parse_instance_id(&a.instance_id)?;
    let entries: Vec<Value> = if crate::daemon::client::daemon_socket_exists() {
        let result = crate::daemon::client::call_daemon(
            "list_tools",
            json!({ "instance_id": instance_id }),
        )
        .await?;
        result
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|t| tool_summary_value(t, a.schema))
            .collect()
    } else {
        let store = build_store(&a.store)?;
        store.load_from_source().await?;
        store.connect_service(instance_id).await?;
        let tools = store
            .list_tool_entries_for_instance_with_filter(
                instance_id,
                mcpstore::ToolVisibilityFilter::Available,
            )
            .await?;
        tools
            .iter()
            .map(|t| tool_summary_value(json!({ "name": t.name, "description": t.description, "schema": t.input_schema }), a.schema))
            .collect()
    };

    if a.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "instance_id": instance_id,
                "tools": entries,
                "total": entries.len(),
            }))?
        );
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

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, ValueEnum)]
pub enum CallOutputFormat {
    #[default]
    Human,
    Json,
    Jsonl,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CallErrorCode {
    InvalidInput,
    ServiceNotFound,
    ConnectionFailed,
    AuthenticationRequired,
    CapabilityUnsupported,
    Cancelled,
    TimedOut,
    Disconnected,
    ToolFailed,
    ProtocolFailed,
    ElicitationInputRequired,
    ElicitationCancelled,
    ElicitationTimedOut,
    ElicitationInvalidResponse,
    CommandFailed,
}

impl CallErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::ServiceNotFound => "service_not_found",
            Self::ConnectionFailed => "connection_failed",
            Self::AuthenticationRequired => "authentication_required",
            Self::CapabilityUnsupported => "capability_unsupported",
            Self::Cancelled => "execution_cancelled",
            Self::TimedOut => "execution_timed_out",
            Self::Disconnected => "execution_disconnected",
            Self::ToolFailed => "tool_failed",
            Self::ProtocolFailed => "protocol_failed",
            Self::ElicitationInputRequired => "input_required",
            Self::ElicitationCancelled => "elicitation_cancelled",
            Self::ElicitationTimedOut => "elicitation_timed_out",
            Self::ElicitationInvalidResponse => "elicitation_invalid_response",
            Self::CommandFailed => "call_command_failed",
        }
    }

    fn exit_code(self) -> i32 {
        match self {
            Self::InvalidInput => 2,
            Self::ServiceNotFound => 10,
            Self::ConnectionFailed => 11,
            Self::AuthenticationRequired => 12,
            Self::CapabilityUnsupported => 20,
            Self::Cancelled => 30,
            Self::TimedOut => 31,
            Self::Disconnected => 32,
            Self::ToolFailed => 33,
            Self::ProtocolFailed => 34,
            Self::ElicitationInputRequired => 35,
            Self::ElicitationCancelled => 36,
            Self::ElicitationTimedOut => 37,
            Self::ElicitationInvalidResponse => 38,
            Self::CommandFailed => 1,
        }
    }

    fn event(self) -> &'static str {
        match self {
            Self::Cancelled => "execution.cancelled",
            Self::TimedOut => "execution.timed_out",
            Self::ElicitationInputRequired => "elicitation.input_required",
            Self::ElicitationCancelled => "elicitation.cancelled",
            Self::ElicitationTimedOut => "elicitation.timed_out",
            Self::ElicitationInvalidResponse => "elicitation.invalid_response",
            _ => "execution.failed",
        }
    }

    /// A brief human-facing next-step suggestion, when one is useful.
    fn hint(self) -> Option<&'static str> {
        match self {
            Self::InvalidInput => Some("check the tool schema with `mcpstore tools <instance> --schema`"),
            Self::ServiceNotFound => Some("run `mcpstore list` to see configured services"),
            Self::ConnectionFailed => Some("run `mcpstore check <instance>` or `mcpstore restart <instance>`"),
            Self::AuthenticationRequired => Some("run `mcpstore auth login <instance>`"),
            Self::TimedOut => Some("retry, or raise --timeout / --max-total-timeout"),
            Self::ElicitationInputRequired => Some("re-run without --non-interactive to answer the prompt"),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct CallCommandError {
    format: CallOutputFormat,
    code: CallErrorCode,
    message: String,
    instance_id: Option<InstanceId>,
    tool_name: Option<String>,
}

impl CallCommandError {
    fn new(format: CallOutputFormat, code: CallErrorCode, message: impl Into<String>) -> Self {
        Self {
            format,
            code,
            message: message.into(),
            instance_id: None,
            tool_name: None,
        }
    }

    fn for_call(
        format: CallOutputFormat,
        code: CallErrorCode,
        message: impl Into<String>,
        instance_id: InstanceId,
        tool_name: impl Into<String>,
    ) -> Self {
        Self {
            format,
            code,
            message: message.into(),
            instance_id: Some(instance_id),
            tool_name: Some(tool_name.into()),
        }
    }

    fn from_store(
        error: StoreError,
        format: CallOutputFormat,
        instance_id: InstanceId,
        tool_name: &str,
    ) -> Self {
        let code = match &error {
            StoreError::ToolNotAvailable { .. } => CallErrorCode::InvalidInput,
            StoreError::ServiceNotFound(_) => CallErrorCode::ServiceNotFound,
            StoreError::Auth(_) => CallErrorCode::AuthenticationRequired,
            StoreError::Transport(error) => match error {
                TransportError::InvalidInput(_) => CallErrorCode::InvalidInput,
                TransportError::AuthRequired(_) | TransportError::InsufficientScope { .. } => {
                    CallErrorCode::AuthenticationRequired
                }
                TransportError::CapabilityUnsupported { .. } => {
                    CallErrorCode::CapabilityUnsupported
                }
                TransportError::RequestCancelled { .. } => CallErrorCode::Cancelled,
                TransportError::RequestTimedOut { .. } => CallErrorCode::TimedOut,
                TransportError::RequestDisconnected { .. } => CallErrorCode::Disconnected,
                TransportError::ConnectionFailed(_)
                | TransportError::NotConnected(_)
                | TransportError::Io(_) => CallErrorCode::ConnectionFailed,
                TransportError::ToolCallFailed(_) => CallErrorCode::ToolFailed,
                TransportError::Protocol(_) => CallErrorCode::ProtocolFailed,
                TransportError::ElicitationSessionActive { .. } => {
                    CallErrorCode::ElicitationInvalidResponse
                }
                TransportError::TaskNotFound { .. } | TransportError::TaskState(_) => {
                    CallErrorCode::CommandFailed
                }
            },
            StoreError::Cache(_)
            | StoreError::Config(_)
            | StoreError::State(_)
            | StoreError::Other(_) => CallErrorCode::CommandFailed,
        };
        Self::for_call(format, code, error.to_string(), instance_id, tool_name)
    }

    pub fn exit_code(&self) -> i32 {
        self.code.exit_code()
    }

    fn json_value(&self) -> Value {
        json!({
            "event": self.code.event(),
            "error": {
                "code": self.code.as_str(),
                "message": self.message,
            },
            "instance_id": self.instance_id,
            "tool_name": self.tool_name,
        })
    }
}

impl std::fmt::Display for CallCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.format {
            CallOutputFormat::Human => match self.code.hint() {
                Some(hint) => write!(
                    formatter,
                    "{}: {}\n  hint: {}",
                    self.code.as_str(),
                    self.message,
                    hint
                ),
                None => write!(formatter, "{}: {}", self.code.as_str(), self.message),
            },
            CallOutputFormat::Json | CallOutputFormat::Jsonl => self.json_value().fmt(formatter),
        }
    }
}

impl std::error::Error for CallCommandError {}

#[derive(Args)]
pub struct CallToolArgs {
    #[arg(
        value_name = "SERVICE|INSTANCE",
        help = "Service name or instance ID"
    )]
    pub target: String,
    #[arg(value_name = "TOOL", help = "Tool name")]
    pub tool_name: String,
    #[arg(
        trailing_var_arg = true,
        value_name = "ARGS",
        help = "Tool arguments: key:value | key=value | --key=value (named options must precede trailing ARGS)"
    )]
    pub args: Vec<String>,
    #[arg(long, default_value = "{}", help = "Tool arguments JSON object, merged with ARGS")]
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
        default_value_t = CallOutputFormat::Human,
        help = "Output format: human, json, or jsonl"
    )]
    pub output: CallOutputFormat,
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
pub struct MigrateBackendArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(
        long = "target-backend",
        value_enum,
        help = "Target cache storage: memory or redis"
    )]
    pub target_cache_storage: CacheStorageArg,
    #[arg(
        long,
        help = "Target Redis URL; used when target cache storage is redis"
    )]
    pub target_redis_url: Option<String>,
}

pub async fn call_tool(a: CallToolArgs) -> std::result::Result<(), BoxErr> {
    execute_call_tool(a)
        .await
        .map_err(|error| Box::new(error) as BoxErr)
}

async fn execute_call_tool(a: CallToolArgs) -> Result<(), CallCommandError> {
    if crate::daemon::client::daemon_socket_exists() {
        return run_call_via_daemon(a).await;
    }
    let scope = a.scope.to_ref(a.agent.as_deref()).map_err(|error| {
        CallCommandError::new(a.output, CallErrorCode::InvalidInput, error.to_string())
    })?;
    let store = build_store(&a.store)
        .map_err(|error| CallCommandError::new(a.output, CallErrorCode::CommandFailed, error.to_string()))?;
    store
        .load_from_source()
        .await
        .map_err(|error| CallCommandError::new(a.output, CallErrorCode::CommandFailed, error.to_string()))?;
    let instance_id = resolve_call_target(&store, &scope, &a.target, a.output).await?;

    store.connect_service(instance_id).await.map_err(|error| {
        CallCommandError::from_store(error, a.output, instance_id, &a.tool_name)
    })?;
    let schema = load_tool_input_schema(&store, instance_id, &a.tool_name, a.output).await?;
    let args = build_call_arguments(&a.args, &a.arguments, schema.as_ref(), a.output)?;

    let mut options = McpExecutionOptions::default();
    if let Some(timeout) = a.timeout {
        options = options.with_idle_timeout(Duration::from_secs(timeout));
    }
    if let Some(timeout) = a.max_total_timeout {
        options = options.with_max_total_timeout(Duration::from_secs(timeout));
    }

    let mut elicitation = store
        .open_elicitation_session(instance_id, a.elicitation.session_options())
        .await
        .map_err(|error| {
            CallCommandError::from_store(error, a.output, instance_id, &a.tool_name)
        })?;
    let mut execution = store
        .start_tool_execution(instance_id, &a.tool_name, args, options)
        .await
        .map_err(|error| {
            CallCommandError::from_store(error, a.output, instance_id, &a.tool_name)
        })?;
    emit_call_started(a.output, &a.tool_name, &execution)?;

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
                                call_elicitation_output(a.output),
                                a.non_interactive,
                            )
                            .await
                            {
                                settle_execution_after_elicitation_error(&mut execution).await;
                                return Err(call_elicitation_error(
                                    error,
                                    a.output,
                                    instance_id,
                                    &a.tool_name,
                                ));
                            }
                        }
                        None => elicitation = None,
                    }
                    continue;
                }
                signal = tokio::signal::ctrl_c() => {
                    signal.map_err(|error| CallCommandError::for_call(
                        a.output,
                        CallErrorCode::CommandFailed,
                        format!("failed to listen for Ctrl+C: {error}"),
                        instance_id,
                        &a.tool_name,
                    ))?;
                    if execution.cancel("cancelled by user (Ctrl+C)") {
                        cancellation_requested = true;
                        emit_call_cancellation_requested(a.output, instance_id, &a.tool_name)?;
                    }
                    continue;
                }
            }
        };

        match update {
            Some(McpStoreExecutionUpdate::Progress(progress)) => {
                emit_call_progress(a.output, &a.tool_name, &progress)?;
            }
            Some(McpStoreExecutionUpdate::Finished(result)) => {
                let execution = result.map_err(|error| {
                    CallCommandError::from_store(error, a.output, instance_id, &a.tool_name)
                })?;
                return finish_call_execution(a.output, instance_id, &a.tool_name, execution);
            }
            None => {
                return Err(CallCommandError::for_call(
                    a.output,
                    CallErrorCode::ProtocolFailed,
                    "tool execution ended without a result",
                    instance_id,
                    &a.tool_name,
                ));
            }
        }
    }
}

fn call_elicitation_output(output: CallOutputFormat) -> ElicitationOutputFormat {
    match output {
        CallOutputFormat::Human => ElicitationOutputFormat::Human,
        CallOutputFormat::Json => ElicitationOutputFormat::Json,
        CallOutputFormat::Jsonl => ElicitationOutputFormat::Jsonl,
    }
}

fn call_elicitation_error(
    error: ElicitationCommandError,
    output: CallOutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
) -> CallCommandError {
    let code = match error.kind() {
        ElicitationErrorKind::InputRequired => CallErrorCode::ElicitationInputRequired,
        ElicitationErrorKind::Cancelled => CallErrorCode::ElicitationCancelled,
        ElicitationErrorKind::TimedOut => CallErrorCode::ElicitationTimedOut,
        ElicitationErrorKind::InvalidResponse => CallErrorCode::ElicitationInvalidResponse,
    };
    CallCommandError::for_call(output, code, error.message(), instance_id, tool_name)
}

/// Resolve a call target to an instance ID. UUIDs are used directly; any other
/// value is treated as a service name and resolved within the requested scope.
async fn resolve_call_target(
    store: &MCPStore,
    scope: &ScopeRef,
    target: &str,
    output: CallOutputFormat,
) -> Result<InstanceId, CallCommandError> {
    if let Ok(instance_id) = InstanceId::from_str(target) {
        return Ok(instance_id);
    }
    let instances = store
        .list_scope_instances(scope)
        .await
        .map_err(|error| CallCommandError::new(output, CallErrorCode::CommandFailed, error.to_string()))?;
    match instances.iter().find(|instance| instance.service_name == target) {
        Some(instance) => Ok(instance.instance_id),
        None => Err(CallCommandError::new(
            output,
            CallErrorCode::ServiceNotFound,
            format!("service not found in {} scope: {target}", scope_label(scope)),
        )),
    }
}

fn scope_label(scope: &ScopeRef) -> &'static str {
    match scope {
        ScopeRef::Store => "store",
        ScopeRef::Agent { .. } => "agent",
    }
}

/// Resolve a call target through the running daemon. UUIDs are used directly;
/// service names are resolved via the daemon's `list_services`.
async fn resolve_call_target_daemon(
    scope: &ScopeRef,
    target: &str,
    output: CallOutputFormat,
) -> Result<InstanceId, CallCommandError> {
    if let Ok(instance_id) = InstanceId::from_str(target) {
        return Ok(instance_id);
    }
    let response = crate::daemon::client::call_daemon("list_services", json!({ "scope": scope }))
        .await
        .map_err(|error| CallCommandError::new(output, CallErrorCode::CommandFailed, error))?;
    let instance = response
        .get("services")
        .and_then(Value::as_array)
        .and_then(|services| {
            services
                .iter()
                .find(|svc| svc.get("service_name").and_then(Value::as_str) == Some(target))
        })
        .and_then(|svc| svc.get("instance_id"))
        .and_then(Value::as_str)
        .and_then(|value| InstanceId::from_str(value).ok());
    instance.ok_or_else(|| {
        CallCommandError::new(
            output,
            CallErrorCode::ServiceNotFound,
            format!("service not found in {} scope: {target}", scope_label(scope)),
        )
    })
}

/// Load the target tool's input schema through the daemon's `list_tools`.
async fn load_tool_input_schema_daemon(
    instance_id: InstanceId,
    tool_name: &str,
    output: CallOutputFormat,
) -> Result<Option<Value>, CallCommandError> {
    let response = crate::daemon::client::call_daemon(
        "list_tools",
        json!({ "instance_id": instance_id }),
    )
    .await
    .map_err(|error| CallCommandError::new(output, CallErrorCode::CommandFailed, error))?;
    Ok(response
        .get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools
                .iter()
                .find(|t| t.get("name").and_then(Value::as_str) == Some(tool_name))
        })
        .and_then(|t| t.get("schema").cloned()))
}

/// Daemon fast path: reuse the daemon's long-lived MCP server connections across
/// CLI invocations. The daemon speaks request/response, so streaming progress,
/// elicitation, per-call timeouts, and cancellation apply only to the local path
/// used when no daemon is running.
async fn run_call_via_daemon(a: CallToolArgs) -> Result<(), CallCommandError> {
    let scope = a
        .scope
        .to_ref(a.agent.as_deref())
        .map_err(|error| CallCommandError::new(a.output, CallErrorCode::InvalidInput, error.to_string()))?;
    let instance_id = resolve_call_target_daemon(&scope, &a.target, a.output).await?;
    let schema = load_tool_input_schema_daemon(instance_id, &a.tool_name, a.output).await?;
    let args = build_call_arguments(&a.args, &a.arguments, schema.as_ref(), a.output)?;
    let value = crate::daemon::client::call_daemon(
        "call_tool",
        json!({ "instance_id": instance_id, "tool_name": a.tool_name, "args": args }),
    )
    .await
    .map_err(|error| CallCommandError::new(a.output, CallErrorCode::CommandFailed, error))?;
    let result: ToolCallResult = serde_json::from_value(value).map_err(|error| {
        CallCommandError::new(
            a.output,
            CallErrorCode::ProtocolFailed,
            format!("daemon returned a malformed tool result: {error}"),
        )
    })?;
    emit_call_result(a.output, instance_id, &a.tool_name, &result)
}

/// Load the target tool's input schema so arguments can be positionally mapped,
/// defaulted, coerced, and validated. Returns `None` when the tool is not in the
/// available-tool set for the instance.
async fn load_tool_input_schema(
    store: &MCPStore,
    instance_id: InstanceId,
    tool_name: &str,
    output: CallOutputFormat,
) -> Result<Option<Value>, CallCommandError> {
    let entries = store
        .list_tool_entries_for_instance_with_filter(
            instance_id,
            mcpstore::ToolVisibilityFilter::Available,
        )
        .await
        .map_err(|error| CallCommandError::from_store(error, output, instance_id, tool_name))?;
    Ok(entries
        .iter()
        .find(|entry| entry.tool_name == tool_name)
        .map(|entry| entry.input_schema.clone()))
}

/// Merge the `--arguments` JSON base with trailing argument tokens, then apply
/// schema-driven positional mapping, defaults, type coercion, and required-field
/// validation when a schema is available.
fn build_call_arguments(
    raw_args: &[String],
    arguments_json: &str,
    schema: Option<&Value>,
    output: CallOutputFormat,
) -> Result<Value, CallCommandError> {
    let mut object = parse_arguments_json_object(arguments_json, output)?;
    let (keyed, positional) = split_argument_tokens(raw_args);
    for (key, raw_value) in keyed {
        object.insert(key, coerce_value(&raw_value));
    }
    if !positional.is_empty() {
        return Err(CallCommandError::new(
            output,
            CallErrorCode::InvalidInput,
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
    output: CallOutputFormat,
) -> Result<Map<String, Value>, CallCommandError> {
    let value: Value = serde_json::from_str(arguments_json).map_err(|error| {
        CallCommandError::new(
            output,
            CallErrorCode::InvalidInput,
            format!("invalid --arguments JSON: {error}"),
        )
    })?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(CallCommandError::new(
            output,
            CallErrorCode::InvalidInput,
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
    output: CallOutputFormat,
) -> Result<(), CallCommandError> {
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
            return Err(CallCommandError::new(
                output,
                CallErrorCode::InvalidInput,
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
    output: CallOutputFormat,
    tool_name: &str,
    execution: &mcpstore::McpStoreToolExecutionHandle<'_>,
) -> Result<(), CallCommandError> {
    if output != CallOutputFormat::Jsonl {
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
    output: CallOutputFormat,
    tool_name: &str,
    progress: &mcpstore::McpExecutionProgress,
) -> Result<(), CallCommandError> {
    match output {
        CallOutputFormat::Human => {
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
        CallOutputFormat::Json => Ok(()),
        CallOutputFormat::Jsonl => emit_call_value(
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
    output: CallOutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
) -> Result<(), CallCommandError> {
    match output {
        CallOutputFormat::Human => {
            eprintln!("[Cancellation requested] {tool_name}");
            Ok(())
        }
        CallOutputFormat::Json => Ok(()),
        CallOutputFormat::Jsonl => emit_call_value(
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
    output: CallOutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
    execution: McpToolExecution,
) -> Result<(), CallCommandError> {
    let McpToolExecution::Immediate { result } = execution else {
        return Err(CallCommandError::for_call(
            output,
            CallErrorCode::ProtocolFailed,
            "tool call unexpectedly returned a task",
            instance_id,
            tool_name,
        ));
    };
    emit_call_result(output, instance_id, tool_name, &result)
}

/// Format a completed tool result for the chosen output format. Shared by the
/// local streaming path (`finish_call_execution`) and the daemon fast path.
fn emit_call_result(
    output: CallOutputFormat,
    instance_id: InstanceId,
    tool_name: &str,
    result: &ToolCallResult,
) -> Result<(), CallCommandError> {
    if result.is_error {
        return Err(CallCommandError::for_call(
            output,
            CallErrorCode::ToolFailed,
            tool_error_message(result),
            instance_id,
            tool_name,
        ));
    }
    match output {
        CallOutputFormat::Human => {
            print_tool_content(result);
            Ok(())
        }
        CallOutputFormat::Json | CallOutputFormat::Jsonl => emit_call_value(
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

fn emit_call_value(output: CallOutputFormat, value: Value) -> Result<(), CallCommandError> {
    let encoded = match output {
        CallOutputFormat::Human => Ok(value.to_string()),
        CallOutputFormat::Json => serde_json::to_string_pretty(&value),
        CallOutputFormat::Jsonl => serde_json::to_string(&value),
    }
    .map_err(|error| {
        CallCommandError::new(
            output,
            CallErrorCode::CommandFailed,
            format!("failed to encode call output: {error}"),
        )
    })?;
    println!("{encoded}");
    Ok(())
}

pub async fn migrate_backend(a: MigrateBackendArgs) -> std::result::Result<(), BoxErr> {
    let store = build_store(&a.store)?;
    store.load_from_source().await?;

    let target_cache_storage = a.target_cache_storage.as_cache_storage();
    let snapshot = store
        .switch_cache_storage(target_cache_storage.clone(), a.target_redis_url, None)
        .await?;
    let total_entries: usize = snapshot.entities.values().map(HashMap::len).sum::<usize>()
        + snapshot.relations.values().map(HashMap::len).sum::<usize>()
        + snapshot.states.values().map(HashMap::len).sum::<usize>()
        + snapshot.events.values().map(HashMap::len).sum::<usize>();

    println!(
        "[Success] Cache storage hot migration completed: target={:?} entries={}",
        target_cache_storage, total_entries
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

pub async fn assign(a: AssignArgs) -> std::result::Result<(), BoxErr> {
    let scope = ScopeRef::Agent {
        agent_id: a.agent.clone(),
    };
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({
            "service_name": a.service_name,
            "scope": scope,
            "descriptor": ScopeDescriptor::default(),
        });
        crate::daemon::client::call_daemon("declare_service_scope", params).await?;
        println!(
            "[Success] Service authorized to Agent: agent={} service={}",
            a.agent, a.service_name
        );
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store
        .declare_service_scope(&a.service_name, &scope, ScopeDescriptor::default())
        .await?;
    println!(
        "[Success] Service authorized to Agent: agent={} service={}",
        a.agent, a.service_name
    );
    Ok(())
}

pub async fn unassign(a: UnassignArgs) -> std::result::Result<(), BoxErr> {
    let scope = ScopeRef::Agent {
        agent_id: a.agent.clone(),
    };
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"service_name": a.service_name, "scope": scope});
        crate::daemon::client::call_daemon("remove_service_scope", params).await?;
        println!(
            "[Success] Removed Agent service authorization: agent={} service={}",
            a.agent, a.service_name
        );
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store.remove_service_scope(&a.service_name, &scope).await?;
    println!(
        "[Success] Removed Agent service authorization: agent={} service={}",
        a.agent, a.service_name
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

fn format_capabilities(metadata: Option<&McpServerMetadata>) -> String {
    let Some(metadata) = metadata else {
        return "unknown".to_string();
    };
    let McpServerCapabilities {
        tools,
        tools_list_changed,
        resources,
        resources_subscribe,
        resources_list_changed,
        prompts,
        prompts_list_changed,
        completions,
        logging,
        tasks,
        task_list,
        task_cancel,
        task_tool_calls,
        extensions,
        experimental,
        ..
    } = &metadata.capabilities;
    let mut enabled = Vec::new();
    for (name, present) in [
        ("tools", *tools),
        ("tools.list_changed", *tools_list_changed),
        ("resources", *resources),
        ("resources.subscribe", *resources_subscribe),
        ("resources.list_changed", *resources_list_changed),
        ("prompts", *prompts),
        ("prompts.list_changed", *prompts_list_changed),
        ("completions", *completions),
        ("logging", *logging),
        ("tasks", *tasks),
        ("tasks.list", *task_list),
        ("tasks.cancel", *task_cancel),
        ("tasks.tool_calls", *task_tool_calls),
        ("extensions", !extensions.is_empty()),
        ("experimental", !experimental.is_empty()),
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
    fn capability_summary_reports_protocol_features() {
        let metadata = McpServerMetadata {
            protocol_version: "2025-06-18".to_string(),
            server_info: mcpstore::McpServerImplementation {
                name: "fixture".to_string(),
                title: None,
                version: "1.0.0".to_string(),
                description: None,
                website_url: None,
            },
            instructions: None,
            capabilities: McpServerCapabilities {
                tools: true,
                tools_list_changed: false,
                resources: true,
                resources_subscribe: true,
                resources_list_changed: false,
                prompts: true,
                prompts_list_changed: false,
                completions: true,
                logging: false,
                tasks: false,
                task_list: false,
                task_cancel: false,
                task_tool_calls: false,
                extensions: Default::default(),
                experimental: Default::default(),
            },
        };
        assert_eq!(
            format_capabilities(Some(&metadata)),
            "tools,resources,resources.subscribe,prompts,completions"
        );
        assert_eq!(format_capabilities(None), "unknown");
    }

    #[test]
    fn call_arguments_require_a_json_object() {
        assert_eq!(
            parse_arguments_json_object(r#"{"value":1}"#, CallOutputFormat::Human).unwrap()["value"],
            1
        );
        let error = parse_arguments_json_object("[]", CallOutputFormat::Jsonl).unwrap_err();
        assert_eq!(error.code, CallErrorCode::InvalidInput);
        assert_eq!(error.exit_code(), 2);
        let value: Value = serde_json::from_str(&error.to_string()).unwrap();
        assert_eq!(value["event"], "execution.failed");
        assert_eq!(value["error"]["code"], "invalid_input");
    }

    #[test]
    fn execution_store_errors_have_stable_codes_and_events() {
        let instance_id: InstanceId = "127ce370-1ed6-5b00-9713-e88d01b3010d".parse().unwrap();
        for (error, code, exit_code, event) in [
            (
                TransportError::RequestCancelled {
                    reason: Some("cancelled".to_string()),
                },
                CallErrorCode::Cancelled,
                30,
                "execution.cancelled",
            ),
            (
                TransportError::RequestTimedOut {
                    timeout: Duration::from_secs(1),
                },
                CallErrorCode::TimedOut,
                31,
                "execution.timed_out",
            ),
            (
                TransportError::RequestDisconnected { instance_id },
                CallErrorCode::Disconnected,
                32,
                "execution.failed",
            ),
        ] {
            let error = CallCommandError::from_store(
                StoreError::Transport(error),
                CallOutputFormat::Jsonl,
                instance_id,
                "long_tool",
            );
            assert_eq!(error.code, code);
            assert_eq!(error.exit_code(), exit_code);
            assert_eq!(error.json_value()["event"], event);
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
    fn split_argument_tokens_classifies_keyed_and_positional() {
        let args: Vec<String> = ["owner:ip2a", "repo=mcp/store", "--draft=true", "bare", "--flag"]
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
            CallOutputFormat::Human,
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
            CallOutputFormat::Human,
        )
        .unwrap();
        assert_eq!(defaulted["owner"], "ip2a");
        assert_eq!(defaulted["count"], 1);

        // base string values coerced to declared primitive types.
        let coerced = build_call_arguments(
            &[],
            r#"{"owner":"x","count":"42","verified":"true"}"#,
            Some(&schema),
            CallOutputFormat::Human,
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
        let err = build_call_arguments(&[], "{}", Some(&schema), CallOutputFormat::Human)
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing required"), "{err}");
    }

    #[test]
    fn build_call_arguments_rejects_positional() {
        let err = build_call_arguments(&["lonely".to_string()], "{}", None, CallOutputFormat::Human)
            .unwrap_err()
            .to_string();
        assert!(err.contains("positional"), "{err}");
    }

    #[test]
    fn call_error_human_output_includes_hint() {
        let error = CallCommandError::new(
            CallOutputFormat::Human,
            CallErrorCode::ServiceNotFound,
            "service not found: github".to_string(),
        );
        let rendered = error.to_string();
        assert!(rendered.contains("service_not_found"), "{rendered}");
        assert!(rendered.contains("hint:"), "{rendered}");
        assert!(rendered.contains("mcpstore list"), "{rendered}");
    }
}
