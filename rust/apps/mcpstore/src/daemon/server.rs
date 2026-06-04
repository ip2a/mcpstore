use std::sync::Arc;

use mcpstore::config::ServerConfig;
use mcpstore::store::MCPStore;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;

use crate::daemon::protocol::{DaemonRequest, DaemonResponse, default_pid_path, default_socket_path};
use crate::store_args::StoreSourceArgs;

/// Start the daemon: create store, bind socket, write PID, accept loop.
pub async fn start_daemon(args: StoreSourceArgs) -> Result<(), Box<dyn std::error::Error>> {
    use crate::store_args::build_store;

    let store = Arc::new(build_store(&args)?);
    store.load_from_source().await?;

    // Ensure any stale files are cleaned up.
    super::protocol::cleanup_stale_files();

    let socket_path = default_socket_path();
    let pid_path = default_pid_path();

    // Write PID file.
    let pid = std::process::id();
    std::fs::write(&pid_path, pid.to_string())?;

    // Bind Unix socket.
    let listener = UnixListener::bind(&socket_path)?;
    tracing::info!("[DAEMON] Started on socket={:?} pid={}", socket_path, pid);
    println!("[DAEMON] MCPStore daemon started (pid={})", pid);

    // Spawn signal handler.
    let shutdown = Arc::new(tokio::sync::Notify::new());
    {
        let shutdown = shutdown.clone();
        let socket_path = socket_path.clone();
        let pid_path = pid_path.clone();
        tokio::spawn(async move {
            let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
                .expect("SIGINT handler");
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler");
            tokio::select! {
                _ = sigint.recv() => {},
                _ = sigterm.recv() => {},
            }
            tracing::info!("[DAEMON] Received shutdown signal");
            let _ = std::fs::remove_file(&socket_path);
            let _ = std::fs::remove_file(&pid_path);
            shutdown.notify_waiters();
        });
    }

    // Accept loop with graceful shutdown.
    let mut accept_loop = true;
    while accept_loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let store = Arc::clone(&store);
                        let shutdown = Arc::clone(&shutdown);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(store, stream, shutdown).await {
                                tracing::warn!("[DAEMON] Connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("[DAEMON] Accept error: {}", e);
                    }
                }
            }
            _ = shutdown.notified() => {
                accept_loop = false;
            }
        }
    }

    // Give in-flight requests a moment to complete.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Final cleanup.
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(&pid_path);
    tracing::info!("[DAEMON] Shut down");
    Ok(())
}

async fn handle_connection(
    store: Arc<MCPStore>,
    stream: UnixStream,
    shutdown: Arc<tokio::sync::Notify>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    let bytes_read = buf_reader.read_line(&mut line).await?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request: DaemonRequest = match serde_json::from_str(&line) {
        Ok(r) => r,
        Err(e) => {
            let resp = DaemonResponse::err(format!("Invalid JSON: {}", e));
            writer
                .write_all(resp.to_json_line()?.as_bytes())
                .await?;
            return Ok(());
        }
    };

    let response = dispatch(store, request, shutdown).await;
    writer
        .write_all(response.to_json_line()?.as_bytes())
        .await?;
    Ok(())
}

async fn dispatch(
    store: Arc<MCPStore>,
    req: DaemonRequest,
    shutdown: Arc<tokio::sync::Notify>,
) -> DaemonResponse {
    match req.method.as_str() {
        "add_service" => handle_add_service(&store, req.params).await,
        "remove_service" => handle_remove_service(&store, req.params).await,
        "connect_service" => handle_connect_service(&store, req.params).await,
        "disconnect_service" => handle_disconnect_service(&store, req.params).await,
        "restart_service" => handle_restart_service(&store, req.params).await,
        "list_services" => handle_list_services(&store, req.params).await,
        "get_service" => handle_get_service(&store, req.params).await,
        "list_tools" => handle_list_tools(&store, req.params).await,
        "call_tool" => handle_call_tool(&store, req.params).await,
        "check_service" => handle_check_service(&store, req.params).await,
        "wait_service" => handle_wait_service(&store, req.params).await,
        "assign_service" => handle_assign_service(&store, req.params).await,
        "unassign_service" => handle_unassign_service(&store, req.params).await,
        "list_agents" => handle_list_agents(&store, req.params).await,
        "show_config" => handle_show_config(&store, req.params).await,
        "reset_config" => handle_reset_config(&store, req.params).await,
        "stop_daemon" => {
            tracing::info!("[DAEMON] Received stop request");
            shutdown.notify_waiters();
            DaemonResponse::ok(Some(json!({"message": "Daemon stopping"})))
        }
        other => DaemonResponse::err(format!("Unknown method: {}", other)),
    }
}

// ---------- Handlers ----------

async fn handle_add_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let name = get_str(&params, "name");
    let config: ServerConfig = match serde_json::from_value(get_field(&params, "config")) {
        Ok(c) => c,
        Err(e) => return DaemonResponse::err(format!("Invalid config: {}", e)),
    };
    match store.add_service(&name, config).await {
        Ok(()) => DaemonResponse::ok(Some(json!({"name": name}))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_remove_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let name = get_str(&params, "name");
    match store.remove_service(&name).await {
        Ok(()) => DaemonResponse::ok(Some(json!({"name": name}))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_connect_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let name = get_str(&params, "name");
    match store.connect_service(&name).await {
        Ok(()) => {
            let tools = store.list_tools(&name).await.unwrap_or_default();
            DaemonResponse::ok(Some(json!({
                "name": name,
                "tools_count": tools.len(),
                "tools": tools.iter().map(|t| json!({"name": t.name, "description": t.description})).collect::<Vec<_>>()
            })))
        }
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_disconnect_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let name = get_str(&params, "name");
    match store.disconnect_service(&name).await {
        Ok(()) => DaemonResponse::ok(Some(json!({"name": name}))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_restart_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let name = get_str(&params, "name");
    match store.restart_service(&name).await {
        Ok(()) => DaemonResponse::ok(Some(json!({"name": name}))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_list_services(store: &MCPStore, _params: Value) -> DaemonResponse {
    let services = store.list_services().await;
    let data: Vec<Value> = services
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "original_name": s.original_name,
                "transport": s.transport,
                "status": format!("{:?}", s.status),
                "tools_count": s.tools.len(),
            })
        })
        .collect();
    DaemonResponse::ok(Some(json!({"services": data, "total": data.len()})))
}

async fn handle_get_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let name = get_str(&params, "name");
    match store.find_service(&name).await {
        Some(s) => DaemonResponse::ok(Some(json!({
            "name": s.name,
            "original_name": s.original_name,
            "transport": s.transport,
            "status": format!("{:?}", s.status),
            "tools": s.tools.iter().map(|t| json!({"name": t.name, "description": t.description})).collect::<Vec<_>>(),
        }))),
        None => DaemonResponse::err(format!("Service not found: {}", name)),
    }
}

async fn handle_list_tools(store: &MCPStore, params: Value) -> DaemonResponse {
    let service_name = get_str(&params, "service_name");
    match store.list_tools(&service_name).await {
        Ok(tools) => {
            let data: Vec<Value> = tools
                .iter()
                .map(|t| json!({"name": t.name, "description": t.description, "schema": t.input_schema}))
                .collect();
            DaemonResponse::ok(Some(json!({"tools": data, "total": data.len()})))
        }
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_call_tool(store: &MCPStore, params: Value) -> DaemonResponse {
    let service_name = get_str(&params, "service_name");
    let tool_name = get_str(&params, "tool_name");
    let args = params.get("args").cloned().unwrap_or_else(|| json!({}));
    match store.call_tool(&service_name, &tool_name, args).await {
        Ok(result) => DaemonResponse::ok(Some(json!({
            "content": result.content.iter().map(|c| match c {
                mcpstore::transport::ContentItem::Text { text } => json!({"type": "text", "text": text}),
                mcpstore::transport::ContentItem::Image { data, mime_type } => json!({"type": "image", "data": data, "mime_type": mime_type}),
            }).collect::<Vec<_>>(),
            "is_error": result.is_error,
        }))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_check_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let name = params.get("name").and_then(Value::as_str);
    if let Some(name) = name {
        match store.health_check(name).await {
            Ok(status) => DaemonResponse::ok(Some(json!({"name": name, "health_status": format!("{:?}", status.health_status)}))),
            Err(e) => DaemonResponse::err(e.to_string()),
        }
    } else {
        let services = store.list_services().await;
        let mut results = serde_json::Map::new();
        for svc in services {
            match store.health_check(&svc.name).await {
                Ok(status) => {
                    results.insert(svc.name.clone(), json!(format!("{:?}", status.health_status)));
                }
                Err(_) => {
                    results.insert(svc.name.clone(), json!("unknown"));
                }
            }
        }
        DaemonResponse::ok(Some(Value::Object(results)))
    }
}

async fn handle_wait_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let name = get_str(&params, "name");
    let timeout = params.get("timeout").and_then(Value::as_u64).unwrap_or(30);
    match store.wait_service_ready(&name, timeout).await {
        Ok(status) => DaemonResponse::ok(Some(json!({
            "name": name,
            "health_status": format!("{:?}", status.health_status),
        }))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_assign_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let agent_id = get_str(&params, "agent_id");
    let service_name = get_str(&params, "service_name");
    match store.assign_service_to_agent(&agent_id, &service_name).await {
        Ok(()) => DaemonResponse::ok(Some(json!({"agent_id": agent_id, "service_name": service_name}))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_unassign_service(store: &MCPStore, params: Value) -> DaemonResponse {
    let agent_id = get_str(&params, "agent_id");
    let service_name = get_str(&params, "service_name");
    match store.unassign_service_from_agent(&agent_id, &service_name).await {
        Ok(()) => DaemonResponse::ok(Some(json!({"agent_id": agent_id, "service_name": service_name}))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_list_agents(store: &MCPStore, _params: Value) -> DaemonResponse {
    match store.list_agents().await {
        Ok(agents) => DaemonResponse::ok(Some(json!({"agents": agents, "total": agents.len()}))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_show_config(store: &MCPStore, _params: Value) -> DaemonResponse {
    match store.show_config().await {
        Ok(config) => DaemonResponse::ok(Some(config)),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

async fn handle_reset_config(store: &MCPStore, _params: Value) -> DaemonResponse {
    match store.reset_config().await {
        Ok(()) => DaemonResponse::ok(Some(json!({"status": "ok"}))),
        Err(e) => DaemonResponse::err(e.to_string()),
    }
}

// ---------- Helpers ----------

fn get_field(params: &Value, key: &str) -> Value {
    params.get(key).cloned().unwrap_or(Value::Null)
}

fn get_str(params: &Value, key: &str) -> String {
    params.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}
