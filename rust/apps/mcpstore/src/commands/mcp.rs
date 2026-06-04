use clap::{Args, ValueEnum};
use mcpstore::config::ServerConfig;
use std::collections::HashMap;

use mcpstore::MCPStore;

use crate::{
    store_args::{build_store, BackendArg, StoreSourceArgs},
    BoxErr,
};

#[derive(Clone, Debug, Eq, PartialEq, ValueEnum, Default)]
pub enum Scope {
    #[default]
    Store,
    Agent,
    Project,
}

#[derive(Args)]
pub struct AddArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[arg(help = "HTTP/SSE URL or stdio command; stdio recommended after --")]
    pub command_or_url: Option<String>,
    #[arg(trailing_var_arg = true, help = "stdio command arguments")]
    pub args: Vec<String>,
    #[arg(long, help = "Transport type: stdio, http, streamable-http, or sse")]
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
    #[arg(
        long,
        num_args = 1,
        help = "HTTP/SSE headers, format KEY=VAL, repeatable"
    )]
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
    let config = build_server_config(
        a.command_or_url.as_deref(),
        &a.args,
        a.transport.as_deref(),
        &env_map,
        &header_map,
    )?;
    let transport = config.infer_transport().to_string();

    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({
            "name": a.name,
            "config": config,
        });
        crate::daemon::client::call_daemon("add_service", params).await?;
        if a.scope == Scope::Agent {
            let agent_id = require_agent(a.agent.as_deref())?;
            let assign_params = serde_json::json!({
                "agent_id": agent_id,
                "service_name": a.name,
            });
            crate::daemon::client::call_daemon("assign_service", assign_params).await?;
        }
        println!(
            "[Success] Service added: {} (transport={})",
            a.name, transport
        );
        return Ok(());
    }

    let store = build_store(&a.store)?;
    store.add_service(&a.name, config).await?;
    apply_scope_after_service_write(&store, &a.scope, a.agent.as_deref(), &a.name).await?;
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
    validate_scope_target(&a.scope, a.agent.as_deref())?;
    let config: ServerConfig = serde_json::from_str(&a.json)?;
    let transport = config.infer_transport().to_string();
    store.add_service(&a.name, config).await?;
    apply_scope_after_service_write(&store, &a.scope, a.agent.as_deref(), &a.name).await?;
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
}

pub async fn list(a: ListArgs) -> std::result::Result<(), BoxErr> {
    validate_agent_flag(&a.scope, a.agent.as_deref())?;

    if crate::daemon::client::daemon_socket_exists() {
        if a.scope == Scope::Agent {
            let agent_id = require_agent(a.agent.as_deref())?;
            let result = crate::daemon::client::call_daemon("list_agents", serde_json::json!({})).await?;
            println!("[Agent] {} service_count={}", agent_id, result.as_array().map(|v| v.len()).unwrap_or(0));
            if let Some(arr) = result.as_array() {
                for item in arr {
                    println!("- {}", item);
                }
            }
            return Ok(());
        }
        let result = crate::daemon::client::call_daemon("list_services", serde_json::json!({})).await?;
        let services = result.get("services").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        println!("[List] service_count={}", services.len());
        if services.is_empty() {
            println!("  No services available");
            return Ok(());
        }
        for svc in services {
            let name = svc.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let transport = svc.get("transport").and_then(|v| v.as_str()).unwrap_or("?");
            let status = svc.get("status").and_then(|v| v.as_str()).unwrap_or("?");
            let tools_count = svc.get("tools_count").and_then(|v| v.as_u64()).unwrap_or(0);
            println!("- {}  transport={}  status={}  tools={}", name, transport, status, tools_count);
        }
        return Ok(());
    }

    let store = build_store(&a.store)?;
    store.load_from_source().await?;

    if a.scope == Scope::Agent {
        let agent_id = require_agent(a.agent.as_deref())?;
        return list_agent_services(&store, agent_id).await;
    }

    let services = store.list_services().await;
    println!("[List] service_count={}", services.len());

    if services.is_empty() {
        println!("  No services available");
        return Ok(());
    }

    for svc in &services {
        println!(
            "- {}  transport={}  status={:?}  tools={}",
            svc.name,
            svc.transport,
            svc.status,
            svc.tools.len()
        );
    }
    Ok(())
}

#[derive(Args)]
pub struct GetArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = Scope::Store, help = "Operation scope")]
    pub scope: Scope,
    #[arg(long, help = "Agent ID, only used with --scope agent")]
    pub agent: Option<String>,
}

pub async fn get(a: GetArgs) -> std::result::Result<(), BoxErr> {
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    validate_scope_target(&a.scope, a.agent.as_deref())?;

    let payload = match a.scope {
        Scope::Agent => {
            let agent_id = require_agent(a.agent.as_deref())?;
            store.service_info_scoped(Some(agent_id), &a.name).await?
        }
        Scope::Store | Scope::Project => store.service_info_scoped(None, &a.name).await?,
    };
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
    validate_agent_flag(&a.scope, a.agent.as_deref())?;
    if a.scope == Scope::Agent {
        let agent_id = require_agent(a.agent.as_deref())?;
        if crate::daemon::client::daemon_socket_exists() {
            let params = serde_json::json!({"agent_id": agent_id, "service_name": a.name});
            crate::daemon::client::call_daemon("unassign_service", params).await?;
            println!(
                "[Success] Removed service authorization from Agent: agent={} service={}",
                agent_id, a.name
            );
            return Ok(());
        }
        let store = build_store(&a.store)?;
        store.load_from_source().await?;
        unassign_service(&store, agent_id, &a.name).await?;
        println!(
            "[Success] Removed service authorization from Agent: agent={} service={}",
            agent_id, a.name
        );
        return Ok(());
    }
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"name": a.name});
        crate::daemon::client::call_daemon("remove_service", params).await?;
        println!("[Success] Service removed: {}", a.name);
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store.remove_service(&a.name).await?;
    println!("[Success] Service removed: {}", a.name);
    Ok(())
}

#[derive(Args)]
pub struct ConnectArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn connect(a: ConnectArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"name": a.name});
        let result = crate::daemon::client::call_daemon("connect_service", params).await?;
        let tools_count = result.get("tools_count").and_then(|v| v.as_u64()).unwrap_or(0);
        println!("[Success] Connected: {} (tools={})", a.name, tools_count);
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store.connect_service(&a.name).await?;

    let tools = store.list_tools(&a.name).await.unwrap_or_default();
    println!("[Success] Connected: {} (tools={})", a.name, tools.len());
    for t in &tools {
        println!("  - {}: {}", t.name, t.description);
    }
    Ok(())
}

#[derive(Args)]
pub struct DisconnectArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn disconnect(a: DisconnectArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"name": a.name});
        crate::daemon::client::call_daemon("disconnect_service", params).await?;
        println!("[Success] Disconnected: {}", a.name);
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store.disconnect_service(&a.name).await?;
    println!("[Success] Disconnected: {}", a.name);
    Ok(())
}

#[derive(Args)]
pub struct RestartArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn restart(a: RestartArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"name": a.name});
        crate::daemon::client::call_daemon("restart_service", params).await?;
        println!("[Success] Restarted: {}", a.name);
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store.restart_service(&a.name).await?;
    println!("[Success] Restarted: {}", a.name);
    Ok(())
}

#[derive(Args)]
pub struct CheckArgs {
    #[arg(help = "Service name; check all services if omitted")]
    pub name: Option<String>,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn check(a: CheckArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = if let Some(ref name) = a.name {
            serde_json::json!({"name": name})
        } else {
            serde_json::json!({})
        };
        let result = crate::daemon::client::call_daemon("check_service", params).await?;
        if let Some(obj) = result.as_object() {
            if obj.len() == 1 && a.name.is_some() {
                for (k, v) in obj {
                    println!("[Check] {} => {}", k, v.as_str().unwrap_or("?"));
                }
            } else {
                for (k, v) in obj {
                    println!("[Check] {} => {}", k, v.as_str().unwrap_or("?"));
                }
            }
        }
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;

    if let Some(name) = a.name {
        let status = store
            .cached_service_status(&name)
            .await?
            .unwrap_or(store.health_check(&name).await?);
        println!("[Check] {} => {:?}", name, status.health_status);
    } else {
        for svc in store.list_services().await {
            let status = store
                .cached_service_status(&svc.name)
                .await?
                .unwrap_or(store.health_check(&svc.name).await?);
            println!("[Check] {} => {:?}", svc.name, status.health_status);
        }
    }
    Ok(())
}

#[derive(Args)]
pub struct WaitArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[arg(long, default_value_t = 30, help = "Wait timeout in seconds")]
    pub timeout: u64,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn wait(a: WaitArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"name": a.name, "timeout": a.timeout});
        let result = crate::daemon::client::call_daemon("wait_service", params).await?;
        let status = result.get("health_status").and_then(|v| v.as_str()).unwrap_or("?");
        println!(
            "[Success] Service ready: {} ({})",
            a.name, status
        );
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store.connect_service(&a.name).await?;
    let status = store.wait_service_ready(&a.name, a.timeout).await?;
    println!(
        "[Success] Service ready: {} ({:?})",
        a.name, status.health_status
    );
    Ok(())
}

#[derive(Args)]
pub struct UpdateArgs {
    #[arg(help = "Service name")]
    pub name: String,
    #[arg(help = "HTTP/SSE URL or stdio command; stdio recommended after --")]
    pub command_or_url: Option<String>,
    #[arg(trailing_var_arg = true, help = "stdio command arguments")]
    pub args: Vec<String>,
    #[arg(long, help = "Transport type: stdio, http, streamable-http, or sse")]
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
    #[arg(
        long,
        num_args = 1,
        help = "HTTP/SSE headers, format KEY=VAL, repeatable"
    )]
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
    store.remove_service(&a.name).await.ok();
    store.add_service(&a.name, config).await?;
    apply_scope_after_service_write(&store, &a.scope, a.agent.as_deref(), &a.name).await?;
    println!("[Success] Service updated: {}", a.name);
    Ok(())
}

#[derive(Args)]
pub struct ToolsArgs {
    #[arg(help = "Service name")]
    pub service_name: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn tools(a: ToolsArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"service_name": a.service_name});
        let result = crate::daemon::client::call_daemon("list_tools", params).await?;
        let tools = result.get("tools").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        println!("[Tools] service={} count={}", a.service_name, tools.len());
        for t in tools {
            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let desc = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
            println!("  - {}: {}", name, desc);
        }
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store.connect_service(&a.service_name).await?;

    let tools = store.list_tools(&a.service_name).await?;
    println!("[Tools] service={} count={}", a.service_name, tools.len());
    for t in &tools {
        println!("  - {}: {}", t.name, t.description);
    }
    Ok(())
}

#[derive(Args)]
pub struct CallToolArgs {
    #[arg(help = "Service name")]
    pub service_name: String,
    #[arg(help = "Tool name")]
    pub tool_name: String,
    #[arg(long, default_value = "{}", help = "Tool call arguments JSON string")]
    pub arguments: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Args)]
pub struct MigrateBackendArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, help = "Target KV backend: memory or redis")]
    pub target_backend: BackendArg,
    #[arg(long, help = "Target Redis URL; used when target backend is redis")]
    pub target_redis_url: Option<String>,
}

pub async fn call_tool(a: CallToolArgs) -> std::result::Result<(), BoxErr> {
    let args: serde_json::Value = serde_json::from_str(&a.arguments)?;
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({
            "service_name": a.service_name,
            "tool_name": a.tool_name,
            "args": args,
        });
        let result = crate::daemon::client::call_daemon("call_tool", params).await?;
        let is_error = result.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
        if is_error {
            eprintln!("[Error] Tool call returned error");
        }
        let content = result.get("content").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        for item in content {
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("text");
            match item_type {
                "text" => {
                    let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    println!("{}", text);
                }
                "image" => {
                    let mime = item.get("mime_type").and_then(|v| v.as_str()).unwrap_or("?");
                    println!("[Image: {}]", mime);
                }
                _ => {}
            }
        }
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    store.connect_service(&a.service_name).await?;

    let result = store.call_tool(&a.service_name, &a.tool_name, args).await?;

    if result.is_error {
        eprintln!("[Error] Tool call returned error");
    }
    for item in &result.content {
        match item {
            mcpstore::transport::ContentItem::Text { text } => println!("{text}"),
            mcpstore::transport::ContentItem::Image { mime_type, .. } => {
                println!("[Image: {mime_type}]")
            }
        }
    }
    Ok(())
}

pub async fn migrate_backend(a: MigrateBackendArgs) -> std::result::Result<(), BoxErr> {
    let store = build_store(&a.store)?;
    store.load_from_source().await?;

    let target_backend = a.target_backend.as_backend_kind();
    let snapshot = store
        .switch_backend(target_backend.clone(), a.target_redis_url, None)
        .await?;
    let total_entries: usize = snapshot.entities.values().map(HashMap::len).sum::<usize>()
        + snapshot.relations.values().map(HashMap::len).sum::<usize>()
        + snapshot.states.values().map(HashMap::len).sum::<usize>()
        + snapshot.events.values().map(HashMap::len).sum::<usize>();

    println!(
        "[Success] Backend hot migration completed: target={:?} entries={}",
        target_backend, total_entries
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
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"agent_id": a.agent, "service_name": a.service_name});
        crate::daemon::client::call_daemon("assign_service", params).await?;
        println!(
            "[Success] Service authorized to Agent: agent={} service={}",
            a.agent, a.service_name
        );
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    assign_service(&store, &a.agent, &a.service_name).await?;
    println!(
        "[Success] Service authorized to Agent: agent={} service={}",
        a.agent, a.service_name
    );
    Ok(())
}

pub async fn unassign(a: UnassignArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"agent_id": a.agent, "service_name": a.service_name});
        crate::daemon::client::call_daemon("unassign_service", params).await?;
        println!(
            "[Success] Removed Agent service authorization: agent={} service={}",
            a.agent, a.service_name
        );
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    unassign_service(&store, &a.agent, &a.service_name).await?;
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
        "Missing service entry: HTTP/SSE requires URL, stdio requires command".to_string()
    })?;
    let is_url = command_or_url.starts_with("http://") || command_or_url.starts_with("https://");

    let resolved_transport = transport
        .map(|t| match t {
            "http" => "streamable-http",
            other => other,
        })
        .unwrap_or(if is_url { "streamable-http" } else { "stdio" })
        .to_string();

    if matches!(resolved_transport.as_str(), "streamable-http" | "sse") && !is_url {
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
            transport: Some(resolved_transport),
            working_dir: None,
            description: None,
        })
    } else {
        Ok(ServerConfig {
            url: None,
            command: Some(command_or_url.to_string()),
            args: args.to_vec(),
            env: env_map.clone(),
            headers: header_map.clone(),
            transport: Some(resolved_transport),
            working_dir: None,
            description: None,
        })
    }
}

async fn apply_scope_after_service_write(
    store: &MCPStore,
    scope: &Scope,
    agent: Option<&str>,
    service_name: &str,
) -> std::result::Result<(), BoxErr> {
    match scope {
        Scope::Store | Scope::Project => validate_agent_flag(scope, agent),
        Scope::Agent => assign_service(store, require_agent(agent)?, service_name).await,
    }
}

async fn assign_service(
    store: &MCPStore,
    agent_id: &str,
    service_name: &str,
) -> std::result::Result<(), BoxErr> {
    store
        .assign_service_to_agent(agent_id, service_name)
        .await?;
    if store.is_db_source() {
        return Ok(());
    }
    let mut cfg = store.config_manager().load_or_default();
    let services = cfg.agents.entry(agent_id.to_string()).or_default();
    if !services.iter().any(|name| name == service_name) {
        services.push(service_name.to_string());
    }
    store.config_manager().save(&cfg)?;
    Ok(())
}

async fn unassign_service(
    store: &MCPStore,
    agent_id: &str,
    service_name: &str,
) -> std::result::Result<(), BoxErr> {
    store
        .unassign_service_from_agent(agent_id, service_name)
        .await?;
    if store.is_db_source() {
        return Ok(());
    }
    let mut cfg = store.config_manager().load_or_default();
    if let Some(services) = cfg.agents.get_mut(agent_id) {
        services.retain(|name| name != service_name);
    }
    store.config_manager().save(&cfg)?;
    Ok(())
}

async fn list_agent_services(store: &MCPStore, agent_id: &str) -> std::result::Result<(), BoxErr> {
    let mut services = store.list_agent_service_names(agent_id).await?;
    if services.is_empty() && !store.is_db_source() {
        let cfg = store.config_manager().load_or_default();
        services = cfg.agents.get(agent_id).cloned().unwrap_or_default();
    }
    println!("[Agent] {} service_count={}", agent_id, services.len());
    for service in services {
        println!("- {service}");
    }
    Ok(())
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
}
