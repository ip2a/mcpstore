use std::sync::Arc;
use std::time::{Duration, Instant};

use mcpstore::config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig};
use mcpstore::error::{Error, FailureCode};
use mcpstore::{
    AppConfig, AuthFlow, InstanceId, MCPStore, McpExecutionOptions, McpStoreExecutionUpdate,
    McpStoreToolExecutionHandle, ScopeRef,
};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::signal;

use crate::daemon::protocol::{
    deadline, default_pid_path, HandshakeRequest, KernelError, KernelEvent, KernelOperation,
    KernelRequest, KernelResponse,
};
use crate::daemon::transport::{HostListener, HostStream};
use crate::store_args::{load_kernel, StoreSourceArgs};

/// daemon 进程持有的全部运行时：kernel、HTTP 面、共享 ApiState。
struct DaemonHost {
    store: Arc<MCPStore>,
    state: Arc<crate::commands::api::ApiState>,
    faces: crate::daemon::listeners::ListenerManager,
    started_at: Instant,
}

/// Start the KernelHost: create one StoreKernel and accept typed IPC requests.
pub async fn start_daemon(args: StoreSourceArgs) -> Result<(), Box<dyn std::error::Error>> {
    let store = load_kernel(&args).await?.store().clone();
    crate::daemon::protocol::cleanup_stale_files();

    let pid_path = default_pid_path();
    let pid = std::process::id();
    std::fs::write(&pid_path, pid.to_string())?;
    let (listener, endpoint) = HostListener::bind()?;
    std::fs::write(&pid_path, pid.to_string())?;
    tracing::info!(
        "[KERNEL_HOST] Started transport={:?} pid={pid}",
        endpoint.transport
    );
    println!("[KERNEL_HOST] MCPStore host started (pid={pid})");

    let state = crate::commands::api::state_for_store(Arc::clone(&store));
    let app_config = store.config_manager().load_app_config_or_default()?;
    let host = Arc::new(DaemonHost {
        store,
        state,
        faces: crate::daemon::listeners::ListenerManager::new(),
        started_at: Instant::now(),
    });
    host.faces.start_all(&app_config, &host.state).await?;

    let shutdown = Arc::new(tokio::sync::Notify::new());
    spawn_shutdown_watcher(shutdown.clone(), endpoint_cleanup_paths());
    let shutdown_task = shutdown.clone();

    loop {
        let stream = tokio::select! {
            stream = listener.accept() => match stream {
                Ok(stream) => stream,
                Err(error) => {
                    tracing::error!("[KERNEL_HOST] Accept failed: {error}");
                    break;
                }
            },
            () = shutdown_task.notified() => break,
        };
        let host = Arc::clone(&host);
        let shutdown = shutdown_task.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_connection(host, stream, shutdown).await {
                tracing::warn!("[KERNEL_HOST] Connection error: {error}");
            }
        });
    }

    host.faces.shutdown_all();
    tokio::time::sleep(Duration::from_millis(500)).await;
    cleanup_paths(endpoint_cleanup_paths());
    tracing::info!("[KERNEL_HOST] Shut down");
    Ok(())
}

#[cfg(unix)]
fn endpoint_cleanup_paths() -> Vec<std::path::PathBuf> {
    vec![
        crate::daemon::protocol::default_socket_path(),
        default_pid_path(),
    ]
}

#[cfg(not(unix))]
fn endpoint_cleanup_paths() -> Vec<std::path::PathBuf> {
    vec![default_pid_path()]
}

fn cleanup_paths(paths: Vec<std::path::PathBuf>) {
    for path in paths {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(unix)]
fn spawn_shutdown_watcher(shutdown: Arc<tokio::sync::Notify>, paths: Vec<std::path::PathBuf>) {
    tokio::spawn(async move {
        let mut sigint =
            signal::unix::signal(signal::unix::SignalKind::interrupt()).expect("SIGINT handler");
        let mut sigterm =
            signal::unix::signal(signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = sigint.recv() => {},
            _ = sigterm.recv() => {},
        }
        tracing::info!("[KERNEL_HOST] Received shutdown signal");
        cleanup_paths(paths);
        shutdown.notify_waiters();
    });
}

#[cfg(not(unix))]
fn spawn_shutdown_watcher(shutdown: Arc<tokio::sync::Notify>, _paths: Vec<std::path::PathBuf>) {
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        tracing::info!("[KERNEL_HOST] Received shutdown signal");
        shutdown.notify_waiters();
    });
}

async fn handle_connection(
    host: Arc<DaemonHost>,
    stream: HostStream,
    shutdown: Arc<tokio::sync::Notify>,
) -> Result<(), Error> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let handshake_line = read_connection_line(&mut reader).await?;
    let handshake: HandshakeRequest = serde_json::from_str(&handshake_line).map_err(|error| {
        Error::new(
            FailureCode::HandshakeFailed,
            format!("invalid KernelHost handshake: {error}"),
        )
    })?;
    let accepted = crate::daemon::protocol::validate_handshake(&handshake, &host.store.namespace());
    let response = match accepted {
        Ok(handshake) => {
            KernelResponse::ok(0, serde_json::to_value(handshake).unwrap_or(Value::Null))
        }
        Err(error) => KernelResponse::error(None, error),
    };
    write_response(&mut writer, &response).await?;
    if response.error.is_some() {
        return Ok(());
    }

    loop {
        let line = match read_connection_line(&mut reader).await {
            Ok(line) => line,
            Err(error) if error.code() == FailureCode::ConnectionClosed => break,
            Err(error) => return Err(error),
        };
        let request: KernelRequest = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                let response = KernelResponse::error(
                    None,
                    KernelError::new(
                        FailureCode::InvalidInput,
                        format!("invalid KernelHost request: {error}"),
                    ),
                );
                write_response(&mut writer, &response).await?;
                continue;
            }
        };
        let stop = matches!(request.operation, KernelOperation::StopHost);
        let response = handle_request(&host, request, &mut writer, shutdown.clone()).await;
        if let Err(error) = response {
            write_response(
                &mut writer,
                &KernelResponse::error(None, KernelError::from_error(&error)),
            )
            .await?;
            break;
        }
        if stop {
            shutdown.notify_waiters();
            break;
        }
    }
    Ok(())
}

async fn read_connection_line<R>(reader: &mut BufReader<R>) -> Result<String, Error>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).await.map_err(|error| {
        Error::new(
            FailureCode::ConnectionClosed,
            format!("KernelHost read failed: {error}"),
        )
    })?;
    if bytes == 0 {
        return Err(Error::new(
            FailureCode::ConnectionClosed,
            "KernelHost connection closed",
        ));
    }
    Ok(line)
}

async fn handle_request<W>(
    host: &DaemonHost,
    request: KernelRequest,
    writer: &mut W,
    _shutdown: Arc<tokio::sync::Notify>,
) -> Result<(), Error>
where
    W: AsyncWriteExt + Unpin,
{
    let store = host.store.as_ref();
    if matches!(
        request.operation,
        KernelOperation::SubscribeEvents | KernelOperation::StreamToolExecution
    ) {
        let request_id = request.request_id;
        let operation = request.operation;
        let result = match operation {
            KernelOperation::SubscribeEvents => {
                subscribe_events(store, request.payload, writer).await
            }
            _ => {
                if let Err(error) = stream_execution(store, request.payload, writer).await {
                    write_response(
                        writer,
                        &KernelResponse::error(Some(request_id), KernelError::from_error(&error)),
                    )
                    .await?;
                }
                Ok(())
            }
        };
        return result;
    }
    let timeout = deadline(request.deadline_ms);
    let request_id = request.request_id;
    let result = tokio::time::timeout(
        timeout,
        execute_operation(host, request.operation, request.payload),
    )
    .await;
    match result {
        Ok(Ok(value)) => {
            write_response(writer, &KernelResponse::ok(request_id, value)).await?;
            Ok(())
        }
        Ok(Err(error)) => {
            write_response(
                writer,
                &KernelResponse::error(Some(request_id), KernelError::from_error(&error)),
            )
            .await
        }
        Err(_) => {
            write_response(
                writer,
                &KernelResponse::error(
                    Some(request_id),
                    KernelError::new(
                        FailureCode::ConnectionTimedOut,
                        "KernelHost request deadline exceeded",
                    ),
                ),
            )
            .await
        }
    }
}

async fn execute_operation(
    host: &DaemonHost,
    operation: KernelOperation,
    payload: Value,
) -> Result<Value, Error> {
    let store = host.store.as_ref();
    match operation {
        KernelOperation::CallTool => {
            let instance_id = instance_id(&payload)?;
            let tool_name = required_str(&payload, "tool_name")?;
            let args = payload.get("args").cloned().unwrap_or_else(|| json!({}));
            let result = store.call_tool(instance_id, &tool_name, args).await?;
            Ok(json!({
                "content": result.content,
                "is_error": result.is_error,
            }))
        }
        KernelOperation::ListTools => {
            let instance_id = instance_id(&payload)?;
            let tools = store
                .list_tool_entries_for_instance_with_filter(
                    instance_id,
                    mcpstore::ToolVisibilityFilter::Available,
                )
                .await?;
            let tools: Vec<Value> = tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "schema": tool.input_schema,
                    })
                })
                .collect();
            Ok(json!({"tools": tools, "total": tools.len()}))
        }
        KernelOperation::ListServices => {
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            let services = store.list_scope_instances(&scope).await?;
            let mut data = Vec::with_capacity(services.len());
            for service in services {
                let state = store.service_state_entry(service.instance_id).await?;
                let metadata = store.mcp_server_metadata(service.instance_id).await?;
                data.push(json!({
                    "instance_id": service.instance_id,
                    "service_name": service.service_name,
                    "scope": service.scope,
                    "transport": service.transport,
                    "state": state,
                    "tools_count": service.tools.len(),
                    "mcp": metadata,
                }));
            }
            Ok(json!({"services": data, "total": data.len()}))
        }
        KernelOperation::GetService => {
            let instance_id = instance_id(&payload)?;
            let service = store
                .find_instance(instance_id)
                .await
                .ok_or_else(|| service_not_found(instance_id))?;
            let state = store.service_state_entry(instance_id).await?;
            Ok(json!({
                "instance_id": service.instance_id,
                "service_name": service.service_name,
                "scope": service.scope,
                "transport": service.transport,
                "state": state,
                "tools": service.tools.iter().map(|tool| json!({
                    "name": tool.name,
                    "description": tool.description,
                })).collect::<Vec<_>>(),
            }))
        }
        KernelOperation::ConnectService => {
            let instance_id = instance_id(&payload)?;
            store.connect_service(instance_id).await?;
            let tools = store
                .list_tool_entries_for_instance_with_filter(
                    instance_id,
                    mcpstore::ToolVisibilityFilter::Available,
                )
                .await
                .unwrap_or_default();
            let metadata = store.mcp_server_metadata(instance_id).await?;
            Ok(json!({
                "instance_id": instance_id,
                "tools_count": tools.len(),
                "tools": tools.iter().map(|tool| json!({
                    "name": tool.name,
                    "description": tool.description,
                })).collect::<Vec<_>>(),
                "mcp": metadata,
            }))
        }
        KernelOperation::DisconnectService => {
            let instance_id = instance_id(&payload)?;
            store.disconnect_service(instance_id).await?;
            Ok(json!({"instance_id": instance_id}))
        }
        KernelOperation::RestartService => {
            let instance_id = instance_id(&payload)?;
            store.restart_service(instance_id).await?;
            Ok(json!({"instance_id": instance_id}))
        }
        KernelOperation::CheckService => {
            let instance_id = instance_id(&payload)?;
            let state = store.service_state_entry(instance_id).await?;
            Ok(json!({"instance_id": instance_id, "state": state}))
        }
        KernelOperation::WaitService => {
            let instance_id = instance_id(&payload)?;
            let timeout = payload.get("timeout").and_then(Value::as_u64).unwrap_or(30);
            let state = store
                .wait_instance_ready(instance_id, Duration::from_secs(timeout))
                .await?;
            Ok(json!({"instance_id": instance_id, "state": state}))
        }
        KernelOperation::AddService => add_service(store, payload).await,
        KernelOperation::DeclareServiceScope => {
            let service_name = required_str(&payload, "service_name")?;
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            let descriptor = payload_field::<ScopeDescriptor>(&payload, "descriptor")?;
            let instance_id = store
                .declare_service_scope(&service_name, &scope, descriptor)
                .await?;
            Ok(json!({
                "instance_id": instance_id,
                "service_name": service_name,
                "scope": scope,
            }))
        }
        KernelOperation::RemoveServiceScope => {
            let service_name = required_str(&payload, "service_name")?;
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            store.remove_service_scope(&service_name, &scope).await?;
            Ok(json!({"service_name": service_name, "scope": scope}))
        }
        KernelOperation::ListAgents => {
            let agents = store.list_agents().await?;
            Ok(json!({"agents": agents, "total": agents.len()}))
        }
        KernelOperation::ShowConfig => store.show_config().await,
        KernelOperation::ResetConfig => {
            store.reset_config().await?;
            Ok(json!({"status": "ok"}))
        }
        KernelOperation::AuthStatus => {
            let instance_id = instance_id(&payload)?;
            Ok(json!({"auth": store.auth_status_view(instance_id).await?}))
        }
        KernelOperation::AuthCallbackUri => {
            let instance_id = instance_id(&payload)?;
            let callback_uri = store.authorization_callback_uri(instance_id).await?;
            Ok(json!({"callback_uri": callback_uri}))
        }
        KernelOperation::AuthBegin => auth_begin(store, payload).await,
        KernelOperation::AuthCallback => {
            let instance_id = instance_id(&payload)?;
            let code = required_str(&payload, "code")?;
            let state = required_str(&payload, "state")?;
            let issuer = payload.get("issuer").and_then(Value::as_str);
            store
                .complete_authorization_callback(instance_id, &code, &state, issuer)
                .await?;
            reconnect_authorized_service(store, instance_id).await?;
            Ok(json!({"auth": store.auth_status_view(instance_id).await?}))
        }
        KernelOperation::AuthRefresh => {
            let instance_id = instance_id(&payload)?;
            store.refresh_authorization(instance_id).await?;
            reconnect_authorized_service(store, instance_id).await?;
            Ok(json!({"auth": store.auth_status_view(instance_id).await?}))
        }
        KernelOperation::AuthLogout => {
            let instance_id = instance_id(&payload)?;
            store.logout_authorization(instance_id).await?;
            Ok(json!({"auth": store.auth_status_view(instance_id).await?}))
        }
        KernelOperation::AuthScopeUpgrade => {
            let instance_id = instance_id(&payload)?;
            let required_scope = required_str(&payload, "required_scope")?;
            let authorization = store
                .begin_scope_upgrade(instance_id, required_scope.trim())
                .await?;
            let auth = store.auth_status_view(instance_id).await?;
            Ok(json!({"auth": auth, "authorization": authorization}))
        }
        KernelOperation::AuthSaveClientSecret => {
            let instance_id = instance_id(&payload)?;
            let secret = required_str(&payload, "client_secret")?;
            store.save_oauth_client_secret(instance_id, secret).await?;
            Ok(json!({"stored": true}))
        }
        KernelOperation::AuthSavePrivateKey => {
            let instance_id = instance_id(&payload)?;
            let private_key = required_str(&payload, "private_key_pem")?;
            store
                .save_oauth_private_key(instance_id, private_key.into_bytes())
                .await?;
            Ok(json!({"stored": true}))
        }
        KernelOperation::SubscribeEvents | KernelOperation::StreamToolExecution => {
            unreachable!("stream operations are handled before the request deadline")
        }
        KernelOperation::StopHost => Ok(json!({"message": "KernelHost stopping"})),
        KernelOperation::StatusHost => Ok(status_host_payload(host)),
        KernelOperation::GetDaemonConfig => get_daemon_config(host),
        KernelOperation::SetDaemonConfig => set_daemon_config(host, payload).await,
    }
}

fn status_host_payload(host: &DaemonHost) -> Value {
    json!({
        "pid": std::process::id(),
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_s": host.started_at.elapsed().as_secs(),
        "namespace": host.store.namespace(),
        "listeners": host.faces.snapshot(),
    })
}

fn get_daemon_config(host: &DaemonHost) -> Result<Value, Error> {
    let config = host
        .store
        .config_manager()
        .load_app_config_or_default()
        .map_err(config_error)?;
    Ok(json!({
        "config": config,
        "listeners": host.faces.snapshot(),
    }))
}

/// 单 key 配置修改：校验 → listener 热应用（先新后旧）→ 原子回写 config.toml。
/// 任一步失败即整体失败：listener 未变就不落盘，落盘成功即已生效。
async fn set_daemon_config(host: &DaemonHost, payload: Value) -> Result<Value, Error> {
    let key = required_str(&payload, "key")?;
    let value = payload
        .get("value")
        .cloned()
        .ok_or_else(|| Error::new(FailureCode::InvalidInput, "value is required"))?;
    let manager = host.store.config_manager();
    let mut config = manager
        .load_app_config_or_default()
        .map_err(config_error)?;
    let planned = plan_config_change(&mut config, &key, &value)?;
    for (face, port) in planned {
        match port {
            Some(port) => {
                host.faces
                    .apply(
                        face,
                        crate::daemon::listeners::resolve_bind(&config.server.host, port)?,
                        &host.state,
                    )
                    .await?;
            }
            None => host.faces.stop(face),
        }
    }
    manager.save_app_config(&config).map_err(config_error)?;
    Ok(json!({"applied": "hot", "key": key}))
}

fn config_error(error: mcpstore::config::ConfigError) -> Error {
    Error::new(
        FailureCode::InvalidInput,
        format!("config.toml 操作失败: {error}"),
    )
}

/// 配置 key 表（设计文档 §7）：把单 key 修改应用到 AppConfig，
/// 返回受影响面的目标端口（Some=起/重绑，None=停；空=仅改配置无面变更）。
fn plan_config_change(
    config: &mut AppConfig,
    key: &str,
    value: &Value,
) -> Result<Vec<(crate::daemon::listeners::ListenerKey, Option<u16>)>, Error> {
    use crate::daemon::listeners::ListenerKey;

    fn aggregate_target(config: &AppConfig) -> Option<u16> {
        (config.mcp_aggregate.enabled && config.mcp_aggregate.transport == "streamable-http")
            .then_some(config.mcp_aggregate.port)
    }

    let server = &mut config.server;
    match key {
        "host" => {
            let host = required_str_value(value, "host")?;
            if host.trim().is_empty() {
                return Err(Error::new(FailureCode::InvalidInput, "host 不能为空"));
            }
            let mut planned = Vec::new();
            if server.core_enabled {
                planned.push((ListenerKey::Core, Some(server.port)));
            }
            if server.app_enabled {
                planned.push((ListenerKey::App, Some(server.app_port)));
            }
            if server.web_enabled {
                planned.push((ListenerKey::Web, Some(server.web_port)));
            }
            server.host = host;
            let aggregate = aggregate_target(config);
            if aggregate.is_some() {
                planned.push((ListenerKey::Aggregate, aggregate));
            }
            Ok(planned)
        }
        "core" => {
            let on = parse_switch(value, "core")?;
            server.core_enabled = on;
            Ok(vec![(ListenerKey::Core, on.then_some(server.port))])
        }
        "core-port" => {
            let port = parse_port(value, "core-port")?;
            server.port = port;
            Ok(server
                .core_enabled
                .then_some(vec![(ListenerKey::Core, Some(port))])
                .unwrap_or_default())
        }
        "app" => {
            let on = parse_switch(value, "app")?;
            server.app_enabled = on;
            Ok(vec![(ListenerKey::App, on.then_some(server.app_port))])
        }
        "app-port" => {
            let port = parse_port(value, "app-port")?;
            server.app_port = port;
            Ok(server
                .app_enabled
                .then_some(vec![(ListenerKey::App, Some(port))])
                .unwrap_or_default())
        }
        "web" => {
            let on = parse_switch(value, "web")?;
            server.web_enabled = on;
            Ok(vec![(ListenerKey::Web, on.then_some(server.web_port))])
        }
        "web-port" => {
            let port = parse_port(value, "web-port")?;
            server.web_port = port;
            Ok(server
                .web_enabled
                .then_some(vec![(ListenerKey::Web, Some(port))])
                .unwrap_or_default())
        }
        "mcp" => {
            let on = parse_switch(value, "mcp")?;
            config.mcp_aggregate.enabled = on;
            let target = aggregate_target(config);
            if on && target.is_none() {
                return Err(Error::new(
                    FailureCode::InvalidInput,
                    "mcp 聚合监听要求 transport=streamable-http；先 --mcp-transport streamable-http",
                ));
            }
            Ok(vec![(ListenerKey::Aggregate, target)])
        }
        "mcp-port" => {
            let port = parse_port(value, "mcp-port")?;
            config.mcp_aggregate.port = port;
            Ok(aggregate_target(config)
                .map(|_| vec![(ListenerKey::Aggregate, Some(port))])
                .unwrap_or_default())
        }
        "mcp-transport" => {
            let transport = required_str_value(value, "mcp-transport")?;
            if !matches!(transport.as_str(), "stdio" | "streamable-http") {
                return Err(Error::new(
                    FailureCode::InvalidInput,
                    format!("mcp-transport 必须是 stdio 或 streamable-http，得到 {transport}"),
                ));
            }
            config.mcp_aggregate.transport = transport;
            Ok(config
                .mcp_aggregate
                .enabled
                .then_some(vec![(ListenerKey::Aggregate, aggregate_target(config))])
                .unwrap_or_default())
        }
        other => Err(Error::new(
            FailureCode::InvalidInput,
            format!(
                "未知配置 key {other:?}；可用：host, core, core-port, app, app-port, web, web-port, mcp, mcp-port, mcp-transport"
            ),
        )),
    }
}

fn required_str_value(value: &Value, field: &str) -> Result<String, Error> {
    value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            Error::new(
                FailureCode::InvalidInput,
                format!("{field} 需要非空字符串"),
            )
        })
}

fn parse_switch(value: &Value, field: &str) -> Result<bool, Error> {
    match value.as_str() {
        Some("on") | Some("true") => Ok(true),
        Some("off") | Some("false") => Ok(false),
        other => Err(Error::new(
            FailureCode::InvalidInput,
            format!("{field} 需要 on/off，得到 {other:?}"),
        )),
    }
}

fn parse_port(value: &Value, field: &str) -> Result<u16, Error> {
    value
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
        .ok_or_else(|| {
            Error::new(
                FailureCode::InvalidInput,
                format!("{field} 需要 1-65535 的端口号"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AppConfig {
        AppConfig::default()
    }

    #[test]
    fn plan_port_change_rebinds_only_that_face() {
        let mut config = config();
        let planned = plan_config_change(&mut config, "web-port", &json!(1829)).unwrap();
        assert_eq!(
            planned,
            vec![(crate::daemon::listeners::ListenerKey::Web, Some(1829))]
        );
        assert_eq!(config.server.web_port, 1829);
        assert_eq!(config.server.port, 1820);
    }

    #[test]
    fn plan_disabled_face_port_change_is_config_only() {
        let mut config = config();
        config.server.core_enabled = false;
        let planned = plan_config_change(&mut config, "core-port", &json!(1822)).unwrap();
        assert!(planned.is_empty());
        assert_eq!(config.server.port, 1822);
    }

    #[test]
    fn plan_face_off_stops_listener() {
        let mut config = config();
        let planned = plan_config_change(&mut config, "web", &json!("off")).unwrap();
        assert_eq!(
            planned,
            vec![(crate::daemon::listeners::ListenerKey::Web, None)]
        );
        assert!(!config.server.web_enabled);
    }

    #[test]
    fn plan_host_rebinds_all_enabled_faces() {
        let mut config = config();
        let planned = plan_config_change(&mut config, "host", &json!("192.168.1.10")).unwrap();
        assert_eq!(planned.len(), 3); // aggregate 默认关
        assert_eq!(config.server.host, "192.168.1.10");
    }

    #[test]
    fn plan_mcp_on_requires_streamable_http() {
        let mut config = config();
        assert!(plan_config_change(&mut config, "mcp", &json!("on")).is_err());

        plan_config_change(&mut config, "mcp-transport", &json!("streamable-http")).unwrap();
        let planned = plan_config_change(&mut config, "mcp", &json!("on")).unwrap();
        assert_eq!(
            planned,
            vec![(crate::daemon::listeners::ListenerKey::Aggregate, Some(1830))]
        );
    }

    #[test]
    fn plan_transport_stdio_stops_aggregate() {
        let mut config = config();
        config.mcp_aggregate.enabled = true;
        config.mcp_aggregate.transport = "streamable-http".to_string();
        let planned = plan_config_change(&mut config, "mcp-transport", &json!("stdio")).unwrap();
        assert_eq!(
            planned,
            vec![(crate::daemon::listeners::ListenerKey::Aggregate, None)]
        );
    }

    #[test]
    fn plan_rejects_unknown_key_and_bad_values() {
        let mut config = config();
        assert!(plan_config_change(&mut config, "nope", &json!(1)).is_err());
        assert!(plan_config_change(&mut config, "web-port", &json!(0)).is_err());
        assert!(plan_config_change(&mut config, "web-port", &json!("1829")).is_err());
        assert!(plan_config_change(&mut config, "web", &json!("maybe")).is_err());
    }
}

async fn stream_execution<W>(store: &MCPStore, payload: Value, writer: &mut W) -> Result<(), Error>
where
    W: AsyncWriteExt + Unpin,
{
    let instance_id = instance_id(&payload)?;
    let tool_name = required_str(&payload, "tool_name")?;
    let args = payload.get("args").cloned().unwrap_or_else(|| json!({}));
    let task = payload
        .get("task")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if payload
        .get("connect")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        store.connect_service(instance_id).await?;
    }
    let options = execution_options(&payload);
    let mut execution: McpStoreToolExecutionHandle<'_> = if task {
        store
            .start_task_execution(instance_id, &tool_name, args, options)
            .await?
    } else {
        store
            .start_tool_execution(instance_id, &tool_name, args, None, options)
            .await?
    };
    let request_id = execution
        .request_id()
        .cloned()
        .unwrap_or_else(|| json!(null));
    write_response(
        writer,
        &KernelResponse::event(KernelEvent::Started {
            request_id,
            instance_id,
            cancellation: execution.supports_cancellation(),
        }),
    )
    .await?;
    while let Some(update) = execution.next_update().await {
        let event = match update {
            McpStoreExecutionUpdate::Progress(progress) => KernelEvent::Progress {
                request_id: json!(null),
                progress,
            },
            McpStoreExecutionUpdate::Finished(result) => KernelEvent::Finished {
                result: match result {
                    Ok(execution) => serde_json::to_value(execution).unwrap_or(Value::Null),
                    Err(error) => {
                        serde_json::to_value(KernelError::from_error(&error)).unwrap_or(Value::Null)
                    }
                },
            },
        };
        write_response(writer, &KernelResponse::event(event)).await?;
    }
    Ok(())
}

async fn subscribe_events<W>(store: &MCPStore, _payload: Value, writer: &mut W) -> Result<(), Error>
where
    W: AsyncWriteExt + Unpin,
{
    let mut receiver = store.event_bus().subscribe_all();
    loop {
        let event = receiver.recv().await.map_err(|error| {
            Error::new(
                FailureCode::ConnectionClosed,
                format!("event stream closed: {error}"),
            )
        })?;
        write_response(
            writer,
            &KernelResponse::event(KernelEvent::Finished {
                result: serde_json::to_value(event).unwrap_or(Value::Null),
            }),
        )
        .await?;
    }
}

async fn add_service(store: &MCPStore, payload: Value) -> Result<Value, Error> {
    let name = required_str(&payload, "name")?;
    let mut config = payload_field::<ServerConfig>(&payload, "config")?;
    let scope = payload_field::<ScopeRef>(&payload, "scope")?;
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
    let definition_exists = store.get_definition_config(&name).await?.is_some();
    if definition_exists {
        let lifecycle = config
            .mcpstore
            .as_ref()
            .and_then(|extension| extension.lifecycle.clone());
        store
            .declare_service_scope(
                &name,
                &scope,
                ScopeDescriptor {
                    config: config.base_config(),
                    lifecycle,
                    revision: 0,
                    ..Default::default()
                },
            )
            .await?;
    } else {
        store.add_service(&name, config).await?;
    }
    Ok(json!({"service_name": name, "scope": scope}))
}

async fn auth_begin(store: &MCPStore, payload: Value) -> Result<Value, Error> {
    let instance_id = instance_id(&payload)?;
    let auth = store.auth_status_view(instance_id).await?;
    match auth.flow {
        Some(AuthFlow::AuthorizationCode) => {
            let authorization = store.begin_authorization(instance_id).await?;
            let auth = store.auth_status_view(instance_id).await?;
            Ok(json!({"auth": auth, "authorization": authorization}))
        }
        Some(AuthFlow::ClientCredentials) => {
            store.refresh_authorization(instance_id).await?;
            reconnect_authorized_service(store, instance_id).await?;
            let auth = store.auth_status_view(instance_id).await?;
            Ok(json!({"auth": auth, "authorization": null}))
        }
        None => Err(Error::new(
            FailureCode::ConnectionAuthRequired,
            "authentication is not configured for this instance",
        )),
    }
}

fn execution_options(payload: &Value) -> McpExecutionOptions {
    let mut options = McpExecutionOptions::default();
    if let Some(timeout) = payload.get("idle_timeout").and_then(Value::as_u64) {
        options = options.with_idle_timeout(Duration::from_millis(timeout));
    }
    if let Some(timeout) = payload.get("max_total_timeout").and_then(Value::as_u64) {
        options = options.with_max_total_timeout(Duration::from_millis(timeout));
    }
    options
}

async fn reconnect_authorized_service(
    store: &MCPStore,
    instance_id: InstanceId,
) -> mcpstore::Result<()> {
    store.disconnect_service(instance_id).await.ok();
    store.connect_service(instance_id).await.map(|_| ())
}

fn service_not_found(instance_id: InstanceId) -> Error {
    Error::new(
        FailureCode::ServiceNotFound,
        format!("service instance not found: {instance_id}"),
    )
}

fn payload_field<T>(payload: &Value, key: &str) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(payload.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|error| Error::new(FailureCode::InvalidInput, format!("invalid {key}: {error}")))
}

fn required_str(payload: &Value, key: &str) -> Result<String, Error> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| Error::new(FailureCode::InvalidInput, format!("{key} is required")))
}

fn instance_id(payload: &Value) -> Result<InstanceId, Error> {
    required_str(payload, "instance_id")?
        .parse()
        .map_err(|error| {
            Error::new(
                FailureCode::InvalidInput,
                format!("invalid instance_id: {error}"),
            )
        })
}

async fn write_response<W>(writer: &mut W, response: &KernelResponse) -> Result<(), Error>
where
    W: AsyncWriteExt + Unpin,
{
    writer
        .write_all(response.to_json_line().map_err(wire_error)?.as_bytes())
        .await
        .map_err(|error| {
            Error::new(
                FailureCode::ConnectionClosed,
                format!("KernelHost write failed: {error}"),
            )
        })
}

fn wire_error(error: serde_json::Error) -> Error {
    Error::new(
        FailureCode::ConnectionClosed,
        format!("failed to serialize KernelHost response: {error}"),
    )
}
