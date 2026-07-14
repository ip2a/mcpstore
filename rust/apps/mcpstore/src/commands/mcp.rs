use clap::{Args, ValueEnum};
use mcpstore::config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig};
use std::collections::HashMap;
use std::str::FromStr;

use mcpstore::{InstanceId, ScopeRef};

use crate::{
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
}

pub async fn list(a: ListArgs) -> std::result::Result<(), BoxErr> {
    let scope = a.scope.to_ref(a.agent.as_deref())?;

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
            let status = svc.get("status").and_then(|v| v.as_str()).unwrap_or("?");
            let tools_count = svc.get("tools_count").and_then(|v| v.as_u64()).unwrap_or(0);
            println!(
                "- {}  instance={}  transport={}  status={}  tools={}",
                name, instance_id, transport, status, tools_count
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
        println!(
            "- {}  instance={}  transport={}  status={:?}  tools={}",
            svc.service_name,
            svc.instance_id,
            svc.transport,
            svc.status,
            svc.tools.len()
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
        println!(
            "[Success] Connected: {} (tools={})",
            a.instance_id, tools_count
        );
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let instance_id = parse_instance_id(&a.instance_id)?;
    store.connect_service(instance_id).await?;

    let tools = store.list_tools(instance_id).await.unwrap_or_default();
    println!(
        "[Success] Connected: {} (tools={})",
        instance_id,
        tools.len()
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
}

pub async fn check(a: CheckArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"instance_id": a.instance_id});
        let result = crate::daemon::client::call_daemon("check_service", params).await?;
        if let Some(obj) = result.as_object() {
            for (k, v) in obj {
                println!("[Check] {} => {}", k, v.as_str().unwrap_or("?"));
            }
        }
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;

    let instance_id = parse_instance_id(&a.instance_id)?;
    let status = store.instance_status_entry(instance_id).await?;
    println!("[Check] {} => {:?}", instance_id, status.health_status);
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
        let status = result
            .get("health_status")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        println!("[Success] Service ready: {} ({})", a.instance_id, status);
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let instance_id = parse_instance_id(&a.instance_id)?;
    store.connect_service(instance_id).await?;
    let status = store.wait_instance_ready(instance_id, a.timeout).await?;
    println!(
        "[Success] Service ready: {} ({:?})",
        instance_id, status.health_status
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
}

pub async fn tools(a: ToolsArgs) -> std::result::Result<(), BoxErr> {
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({"instance_id": a.instance_id});
        let result = crate::daemon::client::call_daemon("list_tools", params).await?;
        let tools = result
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        println!("[Tools] instance={} count={}", a.instance_id, tools.len());
        for t in tools {
            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let desc = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
            println!("  - {}: {}", name, desc);
        }
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let instance_id = parse_instance_id(&a.instance_id)?;
    store.connect_service(instance_id).await?;

    let tools = store.list_tools(instance_id).await?;
    println!("[Tools] instance={} count={}", instance_id, tools.len());
    for t in &tools {
        println!("  - {}: {}", t.name, t.description);
    }
    Ok(())
}

#[derive(Args)]
pub struct CallToolArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
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
    let args: serde_json::Value = serde_json::from_str(&a.arguments)?;
    if crate::daemon::client::daemon_socket_exists() {
        let params = serde_json::json!({
            "instance_id": a.instance_id,
            "tool_name": a.tool_name,
            "args": args,
        });
        let result = crate::daemon::client::call_daemon("call_tool", params).await?;
        let is_error = result
            .get("is_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_error {
            eprintln!("[Error] Tool call returned error");
        }
        let content = result
            .get("content")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for item in content {
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("text");
            match item_type {
                "text" => {
                    let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    println!("{}", text);
                }
                "image" => {
                    let mime = item
                        .get("mime_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    println!("[Image: {}]", mime);
                }
                _ => {}
            }
        }
        return Ok(());
    }
    let store = build_store(&a.store)?;
    store.load_from_source().await?;
    let instance_id = parse_instance_id(&a.instance_id)?;
    store.connect_service(instance_id).await?;

    let result = store.call_tool(instance_id, &a.tool_name, args).await?;

    if result.is_error {
        eprintln!("[Error] Tool call returned error");
    }
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
            transport: Some(resolved_transport),
            working_dir: None,
            description: None,
            mcpstore: None,
            extra: Default::default(),
        })
    }
}

fn parse_instance_id(value: &str) -> std::result::Result<InstanceId, BoxErr> {
    Ok(InstanceId::from_str(value)?)
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
