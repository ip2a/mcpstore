use super::*;
use axum::{extract::Query, Json};
use serde::Deserialize;
use serde_json::{json, Value};

static CLIENT_CHANGE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Deserialize)]
pub(super) struct ClientConfigRequest {
    client: String,
    path: String,
}

#[derive(Deserialize)]
pub(super) struct ClientEntryRequest {
    name: String,
    kind: String,
    config: Value,
}

#[derive(Deserialize)]
pub(super) struct ClientConfigPlanRequest {
    client: String,
    path: String,
    entries: Vec<ClientEntryRequest>,
}

#[derive(Deserialize)]
pub(super) struct ClientConfigApplyRequest {
    client: String,
    path: String,
    expected_hash: String,
    entries: Vec<ClientEntryRequest>,
}

#[derive(Deserialize)]
pub(super) struct ClientConfigUndoRequest {
    change_id: String,
}

#[derive(Deserialize)]
pub(super) struct ClientConfigImportRequest {
    client: String,
    path: String,
    names: Vec<String>,
}

#[derive(Deserialize, Clone, Default)]
pub(super) struct AggregateLaunchQuery {
    scope: Option<String>,
    agent_id: Option<String>,
    instance_id: Option<InstanceId>,
    session_key: Option<String>,
    transport: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    path: Option<String>,
}

pub(super) async fn aggregate_launch(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AggregateLaunchQuery>,
) -> ApiResult {
    let options = aggregate_options(&state, &query)?;
    Ok(success(
        "聚合服务启动信息生成成功",
        serde_json::to_value(options.launch_descriptor("mcpstore"))
            .map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn aggregate_status(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AggregateLaunchQuery>,
) -> ApiResult {
    let options = aggregate_options(&state, &query)?;
    let descriptor = options.launch_descriptor("mcpstore");
    let (running, pid) = aggregate_process_status(&state)?;
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

pub(super) async fn aggregate_start(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AggregateLaunchQuery>,
) -> ApiResult {
    let options = aggregate_options(&state, &query)?;
    if options.transport != McpServerTransport::StreamableHttp {
        return Err(ApiError::invalid_request(
            "stdio 聚合服务需要由 MCP 客户端直接启动",
        ));
    }

    let descriptor = options.launch_descriptor("mcpstore");
    {
        let mut process = state
            .aggregate_process
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
                    aggregate_process_payload(existing, true),
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
    if let Err(error) = wait_for_aggregate_ready(&mut child, &options.host, options.port).await {
        let _ = child.kill().await;
        return Err(error);
    }
    state
        .aggregate_process
        .lock()
        .map_err(|_| ApiError::invalid_request("聚合服务进程状态不可用"))?
        .replace(AggregateProcess { child, descriptor });

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

pub(super) async fn aggregate_stop(State(state): State<Arc<ApiState>>) -> ApiResult {
    let aggregate = {
        let mut process = state
            .aggregate_process
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

fn aggregate_options(
    state: &ApiState,
    query: &AggregateLaunchQuery,
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

async fn wait_for_aggregate_ready(
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

fn aggregate_process_status(state: &ApiState) -> Result<(bool, Option<u32>), ApiError> {
    let mut process = state
        .aggregate_process
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

fn aggregate_process_payload(process: &AggregateProcess, running: bool) -> Value {
    json!({
        "running": running,
        "pid": process.child.id(),
        "transport": process.descriptor.transport,
        "url": process.descriptor.url,
    })
}

pub(super) async fn client_config_inspect(Json(request): Json<ClientConfigRequest>) -> ApiResult {
    let client = parse_client_kind(&request.client)?;
    let inspection = inspect_client_config(client, &request.path).map_err(ApiError::from_store)?;
    Ok(success(
        "编程助手配置检查成功",
        inspection_summary(&inspection),
    ))
}

pub(super) async fn client_config_plan(Json(request): Json<ClientConfigPlanRequest>) -> ApiResult {
    let client = parse_client_kind(&request.client)?;
    let inspection = inspect_client_config(client, &request.path).map_err(ApiError::from_store)?;
    let plans = plan_add_entries(
        &inspection,
        request
            .entries
            .into_iter()
            .map(parse_entry)
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(success(
        "编程助手配置差异计划生成成功",
        plans_summary(&inspection, &plans),
    ))
}

pub(super) async fn client_config_apply(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ClientConfigApplyRequest>,
) -> ApiResult {
    let client = parse_client_kind(&request.client)?;
    let inspection = inspect_client_config(client, &request.path).map_err(ApiError::from_store)?;
    if inspection.content_hash != request.expected_hash {
        return Err(ApiError::invalid_request(
            "配置 hash 已变化，请重新检查并生成计划",
        ));
    }
    let plans = plan_add_entries(
        &inspection,
        request
            .entries
            .into_iter()
            .map(parse_entry)
            .collect::<Result<Vec<_>, _>>()?,
    );
    let receipt = apply_config_change(&inspection, &plans).map_err(ApiError::from_store)?;
    let Some(receipt) = receipt else {
        return Ok(success(
            "配置无需修改",
            json!({"changed": false, "plans": plans_summary(&inspection, &plans)}),
        ));
    };
    let change_id = format!(
        "{}-{}",
        std::process::id(),
        CLIENT_CHANGE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    state
        .client_changes
        .lock()
        .map_err(|_| ApiError::invalid_request("配置撤销状态不可用"))?
        .insert(change_id.clone(), receipt);
    Ok(success(
        "编程助手配置写入成功",
        json!({"changed": true, "change_id": change_id, "plans": plans_summary(&inspection, &plans)}),
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

pub(super) async fn client_config_undo(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ClientConfigUndoRequest>,
) -> ApiResult {
    let receipt = state
        .client_changes
        .lock()
        .map_err(|_| ApiError::invalid_request("配置撤销状态不可用"))?
        .remove(&request.change_id)
        .ok_or_else(|| {
            ApiError::not_found(
                "CHANGE_NOT_FOUND",
                "找不到可撤销的配置变更",
                Some("change_id"),
                None,
            )
        })?;
    mcpstore::client_config::undo_last_change(&receipt).map_err(ApiError::from_store)?;
    Ok(success("编程助手配置撤销成功", json!({"changed": true})))
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

fn parse_entry(request: ClientEntryRequest) -> std::result::Result<ClientEntrySpec, ApiError> {
    let kind = match request.kind.as_str() {
        "original" => ClientEntryKind::Original,
        "aggregate_stdio" => ClientEntryKind::AggregateStdio,
        "aggregate_http" => ClientEntryKind::AggregateHttp,
        _ => {
            return Err(ApiError::invalid_parameter(
                "kind 必须是 original、aggregate_stdio 或 aggregate_http",
                Some("kind"),
            ))
        }
    };
    Ok(ClientEntrySpec {
        name: request.name,
        kind,
        config: request.config,
    })
}

fn inspection_summary(inspection: &ClientConfigInspection) -> Value {
    json!({"client": format_client(inspection.client), "path": inspection.path, "format": format_format(&inspection.format), "content_hash": inspection.content_hash, "services": inspection.services.iter().map(|service| json!({"name": service.name, "fields": service.config.as_object().map(|object| object.keys().collect::<Vec<_>>()).unwrap_or_default()})).collect::<Vec<_>>(), "unsupported_fields": inspection.unsupported_fields})
}

fn plans_summary(inspection: &ClientConfigInspection, plans: &[ClientEntryPlan]) -> Value {
    json!({"client": format_client(inspection.client), "path": inspection.path, "content_hash": inspection.content_hash, "plans": plans.iter().map(|plan| json!({"name": plan.name, "kind": format_entry_kind(plan.kind), "status": format_status(plan.status), "fields": plan.proposed.as_object().map(|object| object.keys().collect::<Vec<_>>()).unwrap_or_default(), "unsupported_fields": plan.unsupported_fields})).collect::<Vec<_>>()})
}
fn format_client(client: ClientKind) -> &'static str {
    match client {
        ClientKind::Codex => "codex",
        ClientKind::ClaudeCode => "claude_code",
        ClientKind::OpenCode => "opencode",
        ClientKind::Cursor => "cursor",
        ClientKind::ClaudeDesktop => "claude_desktop",
    }
}
fn format_format(format: &mcpstore::client_config::ConfigFormat) -> &'static str {
    match format {
        mcpstore::client_config::ConfigFormat::Json => "json",
        mcpstore::client_config::ConfigFormat::Toml => "toml",
    }
}
fn format_entry_kind(kind: ClientEntryKind) -> &'static str {
    match kind {
        ClientEntryKind::Original => "original",
        ClientEntryKind::AggregateStdio => "aggregate_stdio",
        ClientEntryKind::AggregateHttp => "aggregate_http",
    }
}
fn format_status(status: ClientEntryStatus) -> &'static str {
    match status {
        ClientEntryStatus::New => "new",
        ClientEntryStatus::Same => "same",
        ClientEntryStatus::Conflict => "conflict",
        ClientEntryStatus::Unsupported => "unsupported",
    }
}
