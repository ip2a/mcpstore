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
pub(super) struct McpHubLaunchQuery {
    scope: Option<String>,
    agent_id: Option<String>,
    instance_id: Option<InstanceId>,
    session_key: Option<String>,
    transport: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    path: Option<String>,
}

pub(super) async fn mcp_hub_descriptor(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<McpHubLaunchQuery>,
) -> ApiResult {
    let options = mcp_hub_options(&state, &query)?;
    Ok(success(
        "聚合服务启动信息生成成功",
        serde_json::to_value(options.launch_descriptor("mcpstore"))
            .map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn mcp_hub_status(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<McpHubLaunchQuery>,
) -> ApiResult {
    let options = mcp_hub_options(&state, &query)?;
    let descriptor = options.launch_descriptor("mcpstore");
    let (running, pid) = mcp_hub_process_status(&state)?;
    Ok(success(
        "聚合服务状态获取成功",
        json!({
            "running": running,
            "pid": pid,
            "background_supported": options.transport == McpServerTransport::StreamableHttp,
            "transport": options.transport.as_str(),
            "host": options.host,
            "port": options.port,
            "path": options.path,
            "url": descriptor.url,
            "command": descriptor.command,
            "args": descriptor.args,
        }),
    ))
}

pub(super) async fn mcp_hub_start(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<McpHubLaunchQuery>,
) -> ApiResult {
    let options = mcp_hub_options(&state, &query)?;
    if options.transport != McpServerTransport::StreamableHttp {
        return Err(ApiError::invalid_request(
            "stdio 聚合服务需要由 MCP 客户端直接启动",
        ));
    }

    let descriptor = options.launch_descriptor("mcpstore");
    {
        let mut process = state
            .mcp_hub_process
            .lock()
            .map_err(|_| ApiError::invalid_request("聚合服务进程状态不可用"))?;
        if let Some(existing) = process.as_mut() {
            if existing
                .child
                .try_wait()
                .map_err(|error| {
                    ApiError::invalid_request(format!("检查聚合服务状态失败: {error}"))
                })?
                .is_none()
            {
                return Ok(success(
                    "聚合服务已经在运行",
                    mcp_hub_process_payload(existing, true),
                ));
            }
            *process = None;
        }
    }

    let binary = std::env::current_exe().map_err(|error| {
        ApiError::invalid_request(format!("定位 mcpstore 可执行文件失败: {error}"))
    })?;
    let mut command = Command::new(binary);
    command
        .args(&descriptor.args)
        .arg("--source")
        .arg(match options.source_mode {
            mcpstore::SourceMode::Local => "local",
            mcpstore::SourceMode::Db => "db",
        })
        .arg("--config-path")
        .arg(state.store.config_manager().mcp_path())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|error| ApiError::invalid_request(format!("启动聚合服务失败: {error}")))?;
    let pid = child.id();
    if let Err(error) = wait_for_mcp_hub_ready(&mut child, &options.host, options.port).await {
        let _ = child.kill().await;
        return Err(error);
    }
    state
        .mcp_hub_process
        .lock()
        .map_err(|_| ApiError::invalid_request("聚合服务进程状态不可用"))?
        .replace(McpHubProcess { child, descriptor });

    Ok(success(
        "聚合服务启动成功",
        json!({
            "running": true,
            "pid": pid,
            "transport": options.transport.as_str(),
            "url": options.launch_descriptor("mcpstore").url,
        }),
    ))
}

pub(super) async fn mcp_hub_stop(State(state): State<Arc<ApiState>>) -> ApiResult {
    let aggregate = {
        let mut process = state
            .mcp_hub_process
            .lock()
            .map_err(|_| ApiError::invalid_request("聚合服务进程状态不可用"))?;
        process.take()
    };
    let Some(mut aggregate) = aggregate else {
        return Ok(success("聚合服务当前未运行", json!({ "running": false })));
    };
    let pid = aggregate.child.id();
    aggregate
        .child
        .kill()
        .await
        .map_err(|error| ApiError::invalid_request(format!("停止聚合服务失败: {error}")))?;
    Ok(success(
        "聚合服务已停止",
        json!({ "running": false, "pid": pid }),
    ))
}

fn mcp_hub_options(
    state: &ApiState,
    query: &McpHubLaunchQuery,
) -> Result<McpServerOptions, ApiError> {
    let app_config = state
        .store
        .config_manager()
        .load_app_config_or_default()
        .map_err(|error| ApiError::invalid_request(format!("加载 app 配置失败: {error}")))?;
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
                format!("不支持的 scope: {value}"),
                Some("scope"),
            ))
        }
    };
    let transport = match query
        .transport
        .as_deref()
        .unwrap_or(app_config.mcp_aggregate.transport.as_str())
    {
        "stdio" => McpServerTransport::Stdio,
        "streamable-http" | "http" => McpServerTransport::StreamableHttp,
        value => {
            return Err(ApiError::invalid_parameter(
                format!("不支持的 transport: {value}"),
                Some("transport"),
            ))
        }
    };
    Ok(McpServerOptions {
        config_path: Some(
            state
                .store
                .config_manager()
                .mcp_path()
                .display()
                .to_string(),
        ),
        source_mode: state.store.source_mode(),
        scope,
        instance_id: query.instance_id,
        session_key: query.session_key.clone(),
        transport,
        host: query.host.clone().unwrap_or_else(|| "127.0.0.1".into()),
        port: query.port.unwrap_or(app_config.mcp_aggregate.port),
        path: query.path.clone().unwrap_or_else(|| "/mcp".into()),
        ..Default::default()
    })
}

async fn wait_for_mcp_hub_ready(
    child: &mut tokio::process::Child,
    host: &str,
    port: u16,
) -> Result<(), ApiError> {
    let probe_host = match host {
        "0.0.0.0" => "127.0.0.1",
        "::" => "::1",
        value => value,
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| ApiError::invalid_request(format!("检查聚合服务状态失败: {error}")))?
        {
            return Err(ApiError::invalid_request(format!(
                "聚合服务启动后提前退出: {status}"
            )));
        }
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
                "等待聚合服务监听 {probe_host}:{port} 超时"
            )));
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

fn mcp_hub_process_status(state: &ApiState) -> Result<(bool, Option<u32>), ApiError> {
    let mut process = state
        .mcp_hub_process
        .lock()
        .map_err(|_| ApiError::invalid_request("聚合服务进程状态不可用"))?;
    let Some(aggregate) = process.as_mut() else {
        return Ok((false, None));
    };
    if aggregate
        .child
        .try_wait()
        .map_err(|error| ApiError::invalid_request(format!("检查聚合服务状态失败: {error}")))?
        .is_some()
    {
        *process = None;
        return Ok((false, None));
    }
    Ok((true, aggregate.child.id()))
}

fn mcp_hub_process_payload(process: &McpHubProcess, running: bool) -> Value {
    json!({
        "running": running,
        "pid": process.child.id(),
        "transport": process.descriptor.transport,
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
