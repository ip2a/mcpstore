use super::*;
use axum::{extract::Query, Json};
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
    let (running, pid) = gateway_status_inner(&state)?;
    Ok(success(
        "网关服务Status获取成功",
        json!({
            "running": running,
            "pid": pid,
            "host": options.host,
            "port": options.port,
            "path": options.path,
            "url": descriptor.url,
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
        options.scope.clone(),
        options.instance_id,
        options.session_key.clone(),
        options.expose_session_state_tools,
        options.expose_tool_override_tools,
        options.expose_prompt_override_tools,
        options.expose_resource_override_tools,
        options.expose_openapi_tools,
        options.expose_service_tools,
        options.expose_cache_tools,
        options.expose_search_tools,
    )
    .await
    .map_err(|error| ApiError::invalid_request(error.to_string()))?;
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
        ..Default::default()
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

fn gateway_status_inner(state: &ApiState) -> Result<(bool, Option<u32>), ApiError> {
    let mut gateway = state
        .gateway
        .lock()
        .map_err(|_| ApiError::invalid_request("网关服务Status不可用"))?;
    let Some(process) = gateway.as_ref() else {
        return Ok((false, None));
    };
    if process.task.is_finished() {
        *gateway = None;
        return Ok((false, None));
    }
    Ok((true, Some(std::process::id())))
}

fn gateway_payload(process: &GatewayProcess, running: bool) -> Value {
    json!({
        "running": running,
        "pid": std::process::id(),
        "url": process.descriptor.url,
    })
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

fn parse_client_kind(value: &str) -> std::result::Result<ClientKind, ApiError> {
    match value {
        "codex" => Ok(ClientKind::Codex),
        "claude_code" | "claude-code" => Ok(ClientKind::ClaudeCode),
        "opencode" | "open-code" => Ok(ClientKind::OpenCode),
        "cursor" => Ok(ClientKind::Cursor),
        "claude_desktop" | "claude-desktop" => Ok(ClientKind::ClaudeDesktop),
        _ => Err(ApiError::invalid_parameter(
            "client 必须是 codex、claude_code、opencode、cursor 或 claude_desktop",
            Some("client"),
        )),
    }
}
