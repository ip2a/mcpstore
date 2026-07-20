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

#[derive(Deserialize)]
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

pub(super) async fn aggregate_launch(Query(query): Query<AggregateLaunchQuery>) -> ApiResult {
    let scope = match query.scope.as_deref().unwrap_or("store") {
        "store" => ScopeRef::Store,
        "agent" => ScopeRef::Agent {
            agent_id: query
                .agent_id
                .ok_or_else(|| ApiError::missing_parameter("agent_id"))?,
        },
        value => {
            return Err(ApiError::invalid_parameter(
                format!("不支持的 scope: {value}"),
                Some("scope"),
            ))
        }
    };
    let transport = match query.transport.as_deref().unwrap_or("stdio") {
        "stdio" => McpServerTransport::Stdio,
        "streamable-http" | "http" => McpServerTransport::StreamableHttp,
        value => {
            return Err(ApiError::invalid_parameter(
                format!("不支持的 transport: {value}"),
                Some("transport"),
            ))
        }
    };
    let options = McpServerOptions {
        scope,
        instance_id: query.instance_id,
        session_key: query.session_key,
        transport,
        host: query.host.unwrap_or_else(|| "127.0.0.1".into()),
        port: query.port.unwrap_or(18300),
        path: query.path.unwrap_or_else(|| "/mcp".into()),
        ..Default::default()
    };
    Ok(success(
        "聚合服务启动信息生成成功",
        serde_json::to_value(options.launch_descriptor("mcpstore"))
            .map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
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
