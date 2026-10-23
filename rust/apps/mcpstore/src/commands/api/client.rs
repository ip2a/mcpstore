use super::*;
use axum::{extract::Query, Json};
use mcpstore::ScopeRef;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub(super) struct ClientConfigImportRequest {
    client: String,
    path: String,
    names: Vec<String>,
}

#[derive(Deserialize, Clone, Default)]
pub(super) struct GatewayLaunchQuery {
    scope: Option<String>,
    agent_id: Option<String>,
    instance_id: Option<InstanceId>,
    session_key: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    path: Option<String>,
    expose_session_state: Option<bool>,
    expose_tool: Option<bool>,
    expose_prompt: Option<bool>,
    expose_resource: Option<bool>,
    expose_openapi: Option<bool>,
    expose_service: Option<bool>,
    expose_cache: Option<bool>,
    expose_search: Option<bool>,
}

pub(super) async fn gateway_descriptor(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayLaunchQuery>,
) -> ApiResult {
    let options = gateway_options(&state, &query)?;
    Ok(success(
        "网关服务启动信息生成成功",
        serde_json::to_value(options.launch_descriptor())
            .map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn gateway_status(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayLaunchQuery>,
) -> ApiResult {
    let options = gateway_options(&state, &query)?;
    let descriptor = options.launch_descriptor();
    let (running, pid, summary) = gateway_status_inner(&state)?;
    Ok(success(
        "网关服务Status获取成功",
        json!({
            "running": running,
            "pid": pid,
            "host": options.host,
            "port": options.port,
            "path": options.path,
            "url": descriptor.url,
            "summary": summary,
        }),
    ))
}

pub(super) async fn gateway_start(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayLaunchQuery>,
) -> ApiResult {
    let options = gateway_options(&state, &query)?;

    let descriptor = options.launch_descriptor();
    {
        let process = state
            .gateway
            .lock()
            .map_err(|_| ApiError::invalid_request("网关服务Status不可用"))?;
        if let Some(existing) = process.as_ref() {
            if !existing.task.is_finished() {
                return Ok(success(
                    "网关服务已经在运行",
                    gateway_payload(existing, true),
                ));
            }
        }
    }

    let server = GatewayServer::from_store(
        Arc::clone(&state.store),
        &options,
        None,
        crate::gateway::GatewayCallLog::new(),
    )
    .await
    .map_err(|error| ApiError::invalid_request(error.to_string()))?;
    let call_log = server.call_log_handle();
    let task_options = options.clone();
    let task = tokio::spawn(async move {
        if let Err(error) = run_streamable_http(server, &task_options).await {
            tracing::error!("[GATEWAY] Gateway server stopped: {error}");
        }
    });
    if let Err(error) = wait_for_gateway_ready(&options.host, options.port).await {
        task.abort();
        if task.await.is_err() {
            return Err(ApiError::invalid_request("停止网关服务失败"));
        }
        return Err(error);
    }
    state
        .gateway
        .lock()
        .map_err(|_| ApiError::invalid_request("网关服务Status不可用"))?
        .replace(GatewayProcess {
            task,
            descriptor: descriptor.clone(),
            call_log,
            summary: gateway_summary(&options),
        });

    Ok(success(
        "网关服务启动成功",
        json!({
            "running": true,
            "pid": std::process::id(),
            "url": descriptor.url,
        }),
    ))
}

pub(super) async fn gateway_stop(State(state): State<Arc<ApiState>>) -> ApiResult {
    let process = {
        let mut gateway = state
            .gateway
            .lock()
            .map_err(|_| ApiError::invalid_request("网关服务Status不可用"))?;
        gateway.take()
    };
    let Some(process) = process else {
        return Ok(success("网关服务当前未运行", json!({ "running": false })));
    };
    let pid = std::process::id();
    process.task.abort();
    if process.task.await.is_err() {
        return Err(ApiError::invalid_request("停止网关服务失败"));
    }
    Ok(success(
        "网关服务已停止",
        json!({ "running": false, "pid": pid }),
    ))
}

fn gateway_options(
    state: &ApiState,
    query: &GatewayLaunchQuery,
) -> Result<GatewayOptions, ApiError> {
    let app_config = state
        .store
        .config_manager()
        .load_app_config_or_default()
        .map_err(|error| {
            ApiError::invalid_request(format!("Failed to load app configuration: {error}"))
        })?;
    let scope = match query.scope.as_deref().unwrap_or("store") {
        "store" => ScopeRef::Store,
        "agent" => ScopeRef::Agent {
            agent_id: query
                .agent_id
                .clone()
                .ok_or_else(|| ApiError::missing_parameter("agent_id"))?,
        },
        value => {
            return Err(ApiError::invalid_parameter(
                format!("Unsupported scope: {value}"),
                Some("scope"),
            ))
        }
    };
    Ok(GatewayOptions {
        config_path: Some(
            state
                .store
                .config_manager()
                .mcp_path()
                .display()
                .to_string(),
        ),
        scope,
        instance_id: query.instance_id,
        session_key: query.session_key.clone(),
        host: query.host.clone().unwrap_or_else(|| "127.0.0.1".into()),
        port: query.port.unwrap_or(app_config.gateway.port),
        path: query.path.clone().unwrap_or_else(|| "/mcp".into()),
        expose_session_state_tools: query.expose_session_state.unwrap_or(false),
        expose_tool_override_tools: query.expose_tool.unwrap_or(false),
        expose_prompt_override_tools: query.expose_prompt.unwrap_or(false),
        expose_resource_override_tools: query.expose_resource.unwrap_or(false),
        expose_openapi_tools: query.expose_openapi.unwrap_or(false),
        expose_service_tools: query.expose_service.unwrap_or(false),
        expose_cache_tools: query.expose_cache.unwrap_or(false),
        expose_search_tools: query.expose_search.unwrap_or(false),
        ..Default::default()
    })
}

/// Running-config snapshot (scope + expose families) carried by GatewayProcess.
fn gateway_summary(options: &GatewayOptions) -> Value {
    let scope = match &options.scope {
        ScopeRef::Store => "store".to_string(),
        ScopeRef::Agent { agent_id } => format!("agent:{agent_id}"),
    };
    json!({
        "scope": scope,
        "expose": {
            "session_state": options.expose_session_state_tools,
            "tool": options.expose_tool_override_tools,
            "prompt": options.expose_prompt_override_tools,
            "resource": options.expose_resource_override_tools,
            "openapi": options.expose_openapi_tools,
            "service": options.expose_service_tools,
            "cache": options.expose_cache_tools,
            "search": options.expose_search_tools,
        },
    })
}

async fn wait_for_gateway_ready(host: &str, port: u16) -> Result<(), ApiError> {
    let probe_host = match host {
        "0.0.0.0" => "127.0.0.1",
        "::" => "::1",
        value => value,
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

    loop {
        if tokio::time::timeout(
            std::time::Duration::from_millis(200),
            tokio::net::TcpStream::connect((probe_host, port)),
        )
        .await
        .is_ok_and(|result| result.is_ok())
        {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(ApiError::invalid_request(format!(
                "等待网关服务监听 {probe_host}:{port} 超时"
            )));
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

fn gateway_status_inner(state: &ApiState) -> Result<(bool, Option<u32>, Option<Value>), ApiError> {
    let mut gateway = state
        .gateway
        .lock()
        .map_err(|_| ApiError::invalid_request("网关服务Status不可用"))?;
    if let Some(process) = gateway.as_ref() {
        if process.task.is_finished() {
            *gateway = None;
        }
    }
    if let Some(process) = gateway.as_ref() {
        return Ok((true, Some(std::process::id()), Some(process.summary.clone())));
    }
    // The daemon-face gateway mounts at startup without a GatewayProcess.
    if state.face_log().is_some() {
        return Ok((true, Some(std::process::id()), None));
    }
    Ok((false, None, None))
}

fn gateway_payload(process: &GatewayProcess, running: bool) -> Value {
    json!({
        "running": running,
        "pid": std::process::id(),
        "url": process.descriptor.url,
        "summary": process.summary,
    })
}

#[derive(Deserialize, Default)]
pub(super) struct GatewayRequestsQuery {
    after: Option<u64>,
    wait_ms: Option<u64>,
}

pub(super) async fn gateway_requests(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayRequestsQuery>,
) -> ApiResult {
    let call_log = {
        let mut gateway = state
            .gateway
            .lock()
            .map_err(|_| ApiError::invalid_request("网关服务Status不可用"))?;
        if let Some(process) = gateway.as_ref() {
            if process.task.is_finished() {
                *gateway = None;
            }
        }
        match gateway.as_ref() {
            Some(process) => Some(Arc::clone(&process.call_log)),
            None => state.face_log(),
        }
    };
    let Some(call_log) = call_log else {
        return Ok(success(
            "网关服务当前未运行",
            json!({ "running": false, "requests": [], "latest_seq": 0 }),
        ));
    };
    let after = query.after.unwrap_or(0);
    // Long-poll: block until a record past `after` arrives or the wait budget runs out.
    if let Some(wait_ms) = query.wait_ms {
        if wait_ms > 0 && call_log.latest_seq() <= after {
            let mut rx = call_log.subscribe();
            let wait = std::time::Duration::from_millis(wait_ms.min(25_000));
            let _ = tokio::time::timeout(wait, rx.changed()).await;
        }
    }
    let requests: Vec<Value> = call_log
        .snapshot_after(after)
        .iter()
        .map(|record| serde_json::to_value(record).unwrap_or(Value::Null))
        .collect();
    Ok(success(
        "网关服务请求记录获取成功",
        json!({
            "running": true,
            "requests": requests,
            "latest_seq": call_log.latest_seq(),
        }),
    ))
}

fn wired_state_path(state: &ApiState) -> std::path::PathBuf {
    let app_config_path = state.store.config_manager().app_config_path();
    app_config_path
        .parent()
        .map(|dir| dir.join("gateway-wired.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("gateway-wired.json"))
}

/// Re-point legacy wired entries at their caller-mounted URLs before the faces start.
pub(crate) fn refresh_wired_urls(state: &ApiState) {
    let Ok(app_config) = state.store.config_manager().load_app_config_or_default() else {
        return;
    };
    let port = app_config.gateway.port;
    mcpstore::client_wire::refresh_wired_urls(&wired_state_path(state), |client| {
        format!(
            "http://127.0.0.1:{port}/mcp/{}",
            mcpstore::agents::get(client).id()
        )
    });
}

#[derive(Deserialize, Default)]
pub(super) struct GatewayWireQuery {
    client: String,
    path: Option<String>,
    url: Option<String>,
    // Scope override: "store" or "agent"; if "agent" without agent_id, derives from client_id.
    scope: Option<String>,
    agent_id: Option<String>,
    // Exposure overrides (8 flags); if omitted, all default to false.
    expose_session_state: Option<bool>,
    expose_tool: Option<bool>,
    expose_prompt: Option<bool>,
    expose_resource: Option<bool>,
    expose_openapi: Option<bool>,
    expose_service: Option<bool>,
    expose_cache: Option<bool>,
    expose_search: Option<bool>,
}

fn default_gateway_url(state: &ApiState, client: ClientKind) -> Result<String, ApiError> {
    let app_config = state
        .store
        .config_manager()
        .load_app_config_or_default()
        .map_err(|error| {
            ApiError::invalid_request(format!("Failed to load app configuration: {error}"))
        })?;
    Ok(format!(
        "http://127.0.0.1:{}/mcp/{}",
        app_config.gateway.port,
        mcpstore::agents::get(client).id()
    ))
}

pub(super) async fn gateway_wired(State(state): State<Arc<ApiState>>) -> ApiResult {
    let wired =
        mcpstore::client_wire::load_wired_state(&wired_state_path(&state))
            .map_err(ApiError::from_store)?;
    Ok(success(
        "网关接入状态获取成功",
        serde_json::to_value(wired.entries)
            .map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn gateway_wire(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayWireQuery>,
) -> ApiResult {
    let client = parse_client_kind(&query.client)?;
    let path = match query.path {
        Some(path) => path,
        None => {
            // No explicit path: use the client's own config file from discovery.
            let setup = mcpstore::agents::get(client);
            match mcpstore::agents::detect(setup) {
                Some(detection)
                    if detection.by == mcpstore::agents::DetectedBy::ConfigFile =>
                {
                    detection.path.display().to_string()
                }
                _ => {
                    return Err(ApiError::invalid_request(format!(
                        "{} 配置文件未找到",
                        setup.label()
                    )))
                }
            }
        }
    };
    let url = match query.url {
        Some(url) => url,
        None => default_gateway_url(&state, client)?,
    };

    // Parse scope: explicit "store"/"agent" or None (will derive from client_id).
    let scope = match query.scope.as_deref() {
        Some("store") => Some(ScopeRef::Store),
        Some("agent") => {
            let agent_id = query
                .agent_id
                .as_deref()
                .unwrap_or_else(|| mcpstore::agents::get(client).id());
            Some(ScopeRef::Agent {
                agent_id: agent_id.to_string(),
            })
        }
        None => None, // Will derive Agent { agent_id: client.id() } in effective_scope()
        Some(other) => {
            return Err(ApiError::invalid_request(format!(
                "Invalid scope '{other}'; must be 'store' or 'agent'"
            )))
        }
    };

    // Build expose config if any flag was set; otherwise None (will default to all false).
    let expose = if query.expose_session_state.is_some()
        || query.expose_tool.is_some()
        || query.expose_prompt.is_some()
        || query.expose_resource.is_some()
        || query.expose_openapi.is_some()
        || query.expose_service.is_some()
        || query.expose_cache.is_some()
        || query.expose_search.is_some()
    {
        Some(mcpstore::client_wire::ExposeConfig {
            session_state: query.expose_session_state.unwrap_or(false),
            tool: query.expose_tool.unwrap_or(false),
            prompt: query.expose_prompt.unwrap_or(false),
            resource: query.expose_resource.unwrap_or(false),
            openapi: query.expose_openapi.unwrap_or(false),
            service: query.expose_service.unwrap_or(false),
            cache: query.expose_cache.unwrap_or(false),
            search: query.expose_search.unwrap_or(false),
        })
    } else {
        None
    };

    let entry = mcpstore::client_wire::wire_gateway(
        client,
        std::path::Path::new(&path),
        &url,
        &wired_state_path(&state),
        scope,
        expose,
    )
    .map_err(ApiError::from_store)?;
    Ok(success(
        "客户端已接入网关",
        serde_json::to_value(entry).map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn gateway_unwire(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayWireQuery>,
) -> ApiResult {
    let client = parse_client_kind(&query.client)?;
    let entry =
        mcpstore::client_wire::unwire_gateway(client, &wired_state_path(&state))
            .map_err(ApiError::from_store)?;
    Ok(success(
        "客户端已断开网关",
        serde_json::to_value(entry).map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn client_config_import(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ClientConfigImportRequest>,
) -> ApiResult {
    let client = parse_client_kind(&request.client)?;
    let inspection = inspect_client_config(client, &request.path).map_err(ApiError::from_store)?;
    let services =
        import_selected_services(&inspection, &request.names).map_err(ApiError::from_store)?;
    for (name, _) in &services {
        if state
            .store
            .get_definition_config(name)
            .await
            .map_err(ApiError::from_store)?
            .is_some()
        {
            return Err(ApiError::invalid_request(format!(
                "MCPStore 中已存在服务 {name}，拒绝覆盖"
            )));
        }
    }
    let summaries = services
        .iter()
        .map(|(name, config)| json!({"name": name, "transport": config.infer_transport()}))
        .collect::<Vec<_>>();
    for (name, config) in services {
        state
            .store
            .add_service(&name, config)
            .await
            .map_err(ApiError::from_store)?;
    }
    Ok(success(
        "编程助手服务导入成功",
        json!({"imported": summaries}),
    ))
}

/// Read-only inventory: every known client, discovered, inspected, and matched
/// against the wire ledger. Never writes.
pub(super) async fn clients_list(State(state): State<Arc<ApiState>>) -> ApiResult {
    let wired =
        mcpstore::client_wire::load_wired_state(&wired_state_path(&state)).unwrap_or_default();
    let clients = mcpstore::agents::all()
        .iter()
        .map(|setup| {
            let kind = setup.kind();
            let detection = mcpstore::agents::detect(*setup);
            let detected_by = detection.as_ref().map(|detection| {
                serde_json::to_value(detection.by).unwrap_or(Value::Null)
            });
            let config_exists = detection
                .as_ref()
                .is_some_and(|detection| detection.by == mcpstore::agents::DetectedBy::ConfigFile);
            let config_path = match &detection {
                Some(detection) if config_exists => detection.path.display().to_string(),
                _ => setup
                    .default_config_paths()
                    .first()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
            };
            let inspection = if config_exists {
                inspect_client_config(kind, &detection.as_ref().unwrap().path).ok()
            } else {
                None
            };
            let mcp_servers = inspection.as_ref().map(|inspection| {
                inspection
                    .services
                    .iter()
                    .map(|service| service.name.clone())
                    .collect::<Vec<_>>()
            });
            let unsupported_fields = inspection
                .map(|inspection| inspection.unsupported_fields)
                .unwrap_or_default();
            let entry = wired.entries.iter().find(|entry| entry.client == kind);
            json!({
                "kind": kind,
                "id": setup.id(),
                "label": setup.label(),
                "detected_by": detected_by,
                "config_path": config_path,
                "config_exists": config_exists,
                "mcp_servers": mcp_servers,
                "unsupported_fields": unsupported_fields,
                "wired": entry.is_some(),
                "wired_url": entry.map(|entry| entry.url.clone()),
            })
        })
        .collect::<Vec<_>>();
    Ok(success(
        "客户端清单获取成功",
        json!({ "clients": clients }),
    ))
}

fn parse_client_kind(value: &str) -> std::result::Result<ClientKind, ApiError> {
    match value {
        "codex" => Ok(ClientKind::Codex),
        "claude" | "claude_code" | "claude-code" => Ok(ClientKind::ClaudeCode),
        "opencode" | "open-code" => Ok(ClientKind::OpenCode),
        "cursor" => Ok(ClientKind::Cursor),
        "claude_desktop" | "claude-desktop" => Ok(ClientKind::ClaudeDesktop),
        _ => Err(ApiError::invalid_parameter(
            "client 必须是 codex、claude、opencode、cursor 或 claude-desktop",
            Some("client"),
        )),
    }
}
