use std::sync::Arc;
use std::time::Duration;

use mcpstore::config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig};
use mcpstore::error::{Error, FailureCode};
use mcpstore::{
    AuthFlow, InstanceId, MCPStore, McpExecutionOptions, McpStoreExecutionUpdate,
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
    let faces = crate::daemon::listeners::ListenerManager::new();
    faces.start_all(&app_config, &state).await?;

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
        let store = Arc::clone(&store);
        let shutdown = shutdown_task.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_connection(store, stream, shutdown).await {
                tracing::warn!("[KERNEL_HOST] Connection error: {error}");
            }
        });
    }

    faces.shutdown_all();
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
    store: Arc<MCPStore>,
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
    let accepted = crate::daemon::protocol::validate_handshake(&handshake, &store.namespace());
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
        let response = handle_request(&store, request, &mut writer, shutdown.clone()).await;
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
    store: &MCPStore,
    request: KernelRequest,
    writer: &mut W,
    _shutdown: Arc<tokio::sync::Notify>,
) -> Result<(), Error>
where
    W: AsyncWriteExt + Unpin,
{
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
        execute_operation(store, request.operation, request.payload),
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
    store: &MCPStore,
    operation: KernelOperation,
    payload: Value,
) -> Result<Value, Error> {
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
