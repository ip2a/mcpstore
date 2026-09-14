use std::sync::Arc;
use std::time::{Duration, Instant};

use mcpstore::error::{Error, FailureCode};
use mcpstore::{
    MCPStore, McpExecutionOptions, McpStoreExecutionUpdate, McpStoreToolExecutionHandle,
};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::signal;

use crate::daemon::ops::{
    config_error, instance_id, plan_config_change, required_str, resolve_daemon_runtime,
};
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
    crate::daemon::ensure::persist_start_args(&args)?;
    crate::daemon::protocol::cleanup_stale_files();

    let pid_path = default_pid_path();
    let pid = std::process::id();
    std::fs::write(&pid_path, pid.to_string())?;
    let (listener, endpoint) = HostListener::bind()?;
    let state = crate::commands::api::state_for_store(Arc::clone(&store));
    let app_config = store.config_manager().load_app_config_or_default()?;
    let remote_listener = bind_remote_listener(&app_config).await?;
    let host_remote_token = app_config.server.rpc_token.clone();
    std::fs::write(&pid_path, pid.to_string())?;
    tracing::info!(
        "[KERNEL_HOST] Started transport={:?} pid={pid}",
        endpoint.transport
    );
    println!("[KERNEL_HOST] MCPStore host started (pid={pid})");

    let host = Arc::new(DaemonHost {
        store,
        state,
        faces: crate::daemon::listeners::ListenerManager::new(),
        started_at: Instant::now(),
    });
    host.faces.start_all(&app_config, &host.state).await;

    let shutdown = Arc::new(tokio::sync::Notify::new());
    spawn_shutdown_watcher(shutdown.clone(), endpoint_cleanup_paths());
    let shutdown_task = shutdown.clone();

    loop {
        let (stream, required_token) = match remote_listener.as_ref() {
            Some(remote_listener) => tokio::select! {
                stream = listener.accept() => (stream, None),
                stream = accept_remote(remote_listener) => (stream, host_remote_token.clone()),
                () = shutdown_task.notified() => break,
            },
            None => tokio::select! {
                stream = listener.accept() => (stream, None),
                () = shutdown_task.notified() => break,
            },
        };
        let stream = stream?;
        let host = Arc::clone(&host);
        let shutdown = shutdown_task.clone();
        tokio::spawn(async move {
            if let Err(error) =
                handle_connection(host, stream, shutdown, required_token.as_deref()).await
            {
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

async fn accept_remote(listener: &tokio::net::TcpListener) -> Result<HostStream, Error> {
    listener
        .accept()
        .await
        .map(|(stream, _)| HostStream::from(stream))
        .map_err(|error| {
            Error::new(
                FailureCode::ServiceUnavailable,
                format!("KernelHost accept failed: {error}"),
            )
        })
}

async fn bind_remote_listener(
    app_config: &mcpstore::AppConfig,
) -> Result<Option<tokio::net::TcpListener>, Box<dyn std::error::Error>> {
    let port = app_config.server.rpc_port;
    if port == 0 {
        return Ok(None);
    }
    let token = app_config
        .server
        .rpc_token
        .as_deref()
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            Box::<dyn std::error::Error>::from(
                "server.rpc_token is required when server.rpc_port is enabled",
            )
        })?;
    let listener = tokio::net::TcpListener::bind((app_config.server.host.as_str(), port)).await?;
    tracing::info!(
        "[KERNEL_HOST] Remote TCP enabled on port {port}; token length {}",
        token.len()
    );
    Ok(Some(listener))
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
    required_token: Option<&str>,
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
    let accepted = crate::daemon::protocol::validate_handshake(
        &handshake,
        &host.store.namespace(),
        required_token,
    );
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
    match operation {
        KernelOperation::StatusHost => status_host_payload(host).await,
        KernelOperation::GetDaemonConfig => get_daemon_config(host),
        KernelOperation::SetDaemonConfig => set_daemon_config(host, payload).await,
        KernelOperation::StopHost => Ok(json!({"message": "KernelHost stopping"})),
        KernelOperation::SubscribeEvents | KernelOperation::StreamToolExecution => {
            unreachable!("stream operations are handled before the request deadline")
        }
        business => crate::daemon::ops::execute(&host.store, business, payload).await,
    }
}

async fn status_host_payload(host: &DaemonHost) -> mcpstore::Result<Value> {
    let reactor_running = host.store.has_reactor().await;
    let requests = host.store.control_requests().await?;
    let pending = requests
        .iter()
        .any(|request| request.is_pending())
        .then(|| requests.len());
    Ok(json!({
        "pid": std::process::id(),
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_s": host.started_at.elapsed().as_secs(),
        "namespace": host.store.namespace(),
        "reactor_running": reactor_running,
        "control_queue": {
            "pending": pending.unwrap_or(0),
            "requests": requests.len(),
        },
        "listeners": host.faces.snapshot(),
    }))
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
    let mut config = manager.load_app_config_or_default().map_err(config_error)?;
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
#[cfg(test)]
mod tests {
    use super::*;
    use mcpstore::AppConfig;

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
    resolve_daemon_runtime(store, instance_id, &payload).await?;
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
