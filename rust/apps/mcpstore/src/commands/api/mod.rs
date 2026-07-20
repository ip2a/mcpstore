use std::{
    collections::HashMap,
    path::Path as FsPath,
    net::IpAddr,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put},
    Json, Router,
};
use clap::Args;
use mcpstore::{
    client_config::{
        apply_config_change, import_selected_services, inspect_client_config, plan_add_entries,
        ClientConfigInspection, ClientEntryKind, ClientEntryPlan, ClientEntrySpec,
        ClientEntryStatus, ClientKind, ConfigChangeReceipt,
    },
    config::{ConfigError, HistoryPayload, HistoryStorage, ScopeDescriptor},
    config_formats::ConfigFormat,
    mcp_server::{McpServerOptions, McpServerTransport},
    AppConfig, AuthFlow, CreateSessionRequest, InstanceId, MCPStore, McpCompletionRequest,
    McpLoggingLevel, OpenApiBundleOptions, OpenApiImportOptions, OpenApiRefCachePolicy, ScopeRef,
    ServerConfig, SessionScope, ToolTransformPatch,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

use crate::{
    store_args::{build_store, StoreSourceArgs},
    BoxErr,
};

mod envelope;
mod parse;

use envelope::{success, ApiError, ApiResult};
static CLIENT_CHANGE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

use parse::{
    cache_storage_label, extract_prompt_args, extract_prompt_name, extract_resource_uri,
    extract_tool_args, extract_tool_name, normalize_prefix, parse_cache_storage,
    parse_positive_u64, parse_positive_usize,
};

#[derive(Args)]
pub struct ApiArgs {
    #[arg(long, default_value_t = 18200, help = "API 服务端口")]
    pub port: u16,
    #[arg(long, default_value = "127.0.0.1", help = "绑定地址")]
    pub host: String,
    #[arg(long, default_value = "", help = "URL 前缀，例如 /mcp")]
    pub url_prefix: String,
    #[arg(long, help = "显式允许非 loopback API 绑定")]
    pub allow_remote: bool,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Clone)]
pub struct ApiState {
    store: Arc<MCPStore>,
    client_changes: Arc<Mutex<HashMap<String, ConfigChangeReceipt>>>,
}

#[derive(Deserialize)]
struct CacheSwitchRequest {
    backend: String,
    redis_url: Option<String>,
    namespace: Option<String>,
}

#[derive(Deserialize)]
struct SessionCreateRequest {
    session_id: String,
    scope: Option<String>,
    agent_id: Option<String>,
    lease_seconds: Option<i64>,
    metadata: Option<Value>,
}

#[derive(Deserialize)]
struct SessionKeyQuery {
    session_key: String,
}

#[derive(Deserialize)]
struct ToolListQuery {
    filter: Option<String>,
}

#[derive(Deserialize)]
struct ToolVisibilityRequest {
    available_tools: Vec<String>,
}

#[derive(Deserialize)]
struct SessionFindQuery {
    session_id: String,
    scope: Option<String>,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
struct ShowConfigQuery {
    format: Option<String>,
    instance_id: Option<InstanceId>,
}

#[derive(Deserialize)]
struct ClientConfigRequest {
    client: String,
    path: String,
}

#[derive(Deserialize)]
struct ClientEntryRequest {
    name: String,
    kind: String,
    config: Value,
}

#[derive(Deserialize)]
struct ClientConfigPlanRequest {
    client: String,
    path: String,
    entries: Vec<ClientEntryRequest>,
}

#[derive(Deserialize)]
struct ClientConfigApplyRequest {
    client: String,
    path: String,
    expected_hash: String,
    entries: Vec<ClientEntryRequest>,
}

#[derive(Deserialize)]
struct ClientConfigUndoRequest {
    change_id: String,
}

#[derive(Deserialize)]
struct ClientConfigImportRequest {
    client: String,
    path: String,
    names: Vec<String>,
}

#[derive(Deserialize)]
struct AggregateLaunchQuery {
    scope: Option<String>,
    agent_id: Option<String>,
    instance_id: Option<InstanceId>,
    session_key: Option<String>,
    transport: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    path: Option<String>,
}

#[derive(Deserialize)]
struct AuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    #[serde(rename = "iss")]
    issuer: Option<String>,
}

#[derive(Deserialize)]
struct AuthCallbackRequest {
    callback_url: String,
}

#[derive(Deserialize)]
struct AuthClientSecretRequest {
    client_secret: String,
}

#[derive(Deserialize)]
struct AuthPrivateKeyRequest {
    private_key_pem: String,
}

#[derive(Deserialize)]
struct AuthScopeUpgradeRequest {
    required_scope: String,
}

#[derive(Deserialize)]
struct ResourceSubscriptionRequest {
    uri: String,
}

#[derive(Deserialize)]
struct LoggingLevelRequest {
    level: McpLoggingLevel,
}

#[derive(Deserialize)]
struct SessionListQuery {
    scope: Option<String>,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
struct SessionCloseRequest {
    session_key: Option<String>,
    reason: Option<String>,
}

#[derive(Deserialize)]
struct SessionExtendRequest {
    session_key: Option<String>,
    lease_seconds: i64,
}

#[derive(Deserialize)]
struct SessionBindServiceRequest {
    session_key: Option<String>,
    instance_id: InstanceId,
}

#[derive(Deserialize)]
struct SessionStateValueQuery {
    session_key: String,
    key: String,
}

#[derive(Deserialize)]
struct SessionStateSetRequest {
    session_key: Option<String>,
    key: String,
    value: Value,
}

#[derive(Deserialize)]
struct SessionStateDeleteRequest {
    session_key: Option<String>,
    key: String,
}

#[derive(Deserialize)]
struct SessionStateClearRequest {
    session_key: Option<String>,
}

#[derive(Deserialize)]
struct OpenApiImportRequest {
    spec_url: String,
    spec: Option<Value>,
    spec_text: Option<String>,
    timeout_millis: Option<u64>,
    fetch_timeout_millis: Option<u64>,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    auth: serde_json::Map<String, Value>,
    #[serde(default)]
    ref_cache: OpenApiRefCachePolicy,
}

#[derive(Deserialize)]
struct UpdateSettingsRequest {
    language: Option<String>,
    default_backup_dir: Option<String>,
    logging: Option<UpdateLoggingRequest>,
    diagnostics: Option<UpdateDiagnosticsRequest>,
}

#[derive(Deserialize)]
struct UpdateLoggingRequest {
    max_size_bytes: Option<u64>,
    retention_days: Option<Option<u64>>,
}

#[derive(Deserialize)]
struct UpdateDiagnosticsRequest {
    enabled: Option<bool>,
    runtime_log: Option<UpdateRuntimeLogRequest>,
    history: Option<UpdateHistoryRequest>,
}

#[derive(Deserialize)]
struct UpdateRuntimeLogRequest {
    enabled: Option<bool>,
    max_size_bytes: Option<u64>,
}

#[derive(Deserialize)]
struct UpdateHistoryRequest {
    enabled: Option<bool>,
    storage: Option<HistoryStorage>,
    max_records: Option<usize>,
    max_size_bytes: Option<u64>,
    retention_days: Option<Option<u64>>,
    payload: Option<HistoryPayload>,
}

pub async fn run(args: ApiArgs) -> Result<(), BoxErr> {
    let loopback = args.host == "localhost"
        || args.host.parse::<IpAddr>().is_ok_and(|address| address.is_loopback());
    if !loopback && !args.allow_remote {
        return Err("API 默认只允许 loopback 绑定；使用 --allow-remote 明确开启远程暴露".into());
    }

    let store = build_store(&args.store)?;
    store.load_from_source().await?;

    let prefix = normalize_prefix(&args.url_prefix);
    let app = router_for_store(store, &prefix);

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let display_prefix = if prefix.is_empty() {
        "/".to_string()
    } else {
        prefix.clone()
    };
    println!("[API] Starting at http://{addr}{display_prefix}");

    axum::serve(listener, app).await?;
    Ok(())
}

pub fn router_for_store(store: Arc<MCPStore>, prefix: &str) -> Router {
    let state = Arc::new(ApiState {
        store,
        client_changes: Arc::new(Mutex::new(HashMap::new())),
    });
    if !state.store.is_db_source() {
        let store = state.store.clone();
        tokio::spawn(async move {
            if let Err(error) = store.restart_control_reactor().await {
                tracing::error!(
                    "[API] Failed to restart event reactor after cache switch: {error}"
                );
            }
        });
    }
    router(state, prefix)
}

/// Start an EventReactor that processes control_requests via push-based
/// ChangeFeed events (replaces the old 1-second polling scanner).
///
/// Before starting the reactor, one catch-up scan processes any backlog left
/// from a previous shutdown. After that, all new requests are handled by the
/// reactor's ChangeFeed subscription — no polling.
fn spawn_control_reactor(store: Arc<MCPStore>) {
    tokio::spawn(async move {
        if let Err(error) = store.restart_control_reactor().await {
            tracing::error!("[API] Failed to start event reactor: {error}");
        }
    });
}

fn router(state: Arc<ApiState>, prefix: &str) -> Router {
    let base = Router::new()
        .route("/health", get(health))
        .route("/v1/meta", get(app_meta))
        .route("/v1/settings", put(app_update_settings))
        .route("/agents/list", get(list_agents))
        .route("/events/history", get(event_history))
        .route("/history/tool-calls", get(tool_call_history))
        .route("/history/tool-calls/clear", post(clear_tool_call_history))
        .route("/events/capability_report", get(event_capability_report))
        .route("/sessions/create", post(session_create))
        .route("/sessions/get/:session_key", get(session_get))
        .route("/sessions/get", get(session_get_by_query))
        .route("/sessions/find", get(session_find))
        .route("/sessions/list", get(session_list))
        .route("/sessions/snapshot", get(session_export_snapshot))
        .route("/sessions/snapshot/import", post(session_import_snapshot))
        .route("/sessions/status/:session_key", get(session_status))
        .route("/sessions/status", get(session_status_by_query))
        .route("/sessions/close", post(session_close_by_body))
        .route("/sessions/close/:session_key", post(session_close))
        .route("/sessions/extend", post(session_extend_by_body))
        .route("/sessions/extend/:session_key", post(session_extend))
        .route("/sessions/bind_service", post(session_bind_service_by_body))
        .route(
            "/sessions/bind_service/:session_key",
            post(session_bind_service),
        )
        .route(
            "/sessions/unbind_service",
            post(session_unbind_service_by_body),
        )
        .route(
            "/sessions/unbind_service/:session_key",
            post(session_unbind_service),
        )
        .route(
            "/sessions/list_services",
            get(session_list_services_by_query),
        )
        .route(
            "/sessions/list_services/:session_key",
            get(session_list_services),
        )
        .route("/sessions/list_tools", get(session_list_tools_by_query))
        .route("/sessions/list_tools/:session_key", get(session_list_tools))
        .route("/sessions/call_tool", post(session_call_tool_by_body))
        .route("/sessions/call_tool/:session_key", post(session_call_tool))
        .route("/sessions/state/list", get(session_list_state_by_query))
        .route("/sessions/state/list/:session_key", get(session_list_state))
        .route("/sessions/state/value", get(session_get_state_value))
        .route("/sessions/state/set", post(session_set_state_by_body))
        .route("/sessions/state/set/:session_key", post(session_set_state))
        .route("/sessions/state/delete", post(session_delete_state_by_body))
        .route(
            "/sessions/state/delete/:session_key",
            post(session_delete_state),
        )
        .route("/sessions/state/clear", post(session_clear_state_by_body))
        .route(
            "/sessions/state/clear/:session_key",
            post(session_clear_state),
        )
        .route("/scopes/store/instances", get(store_list_services))
        .route(
            "/scopes/agents/:agent_id/instances",
            get(agent_list_services),
        )
        .route(
            "/services/:service_name",
            post(add_service_definition)
                .put(update_service_definition)
                .delete(remove_service_definition),
        )
        .route(
            "/services/:service_name/scopes/store",
            put(declare_store_scope).delete(remove_store_scope),
        )
        .route(
            "/services/:service_name/scopes/agents/:agent_id",
            put(declare_agent_scope).delete(remove_agent_scope),
        )
        .route(
            "/instances/:instance_id/connect",
            post(store_connect_service),
        )
        .route("/instances/:instance_id/auth", get(store_auth_status))
        .route("/instances/:instance_id/auth/start", post(store_auth_start))
        .route(
            "/instances/:instance_id/auth/callback",
            get(store_auth_callback_get).post(store_auth_callback_post),
        )
        .route(
            "/instances/:instance_id/auth/refresh",
            post(store_auth_refresh),
        )
        .route(
            "/instances/:instance_id/auth/logout",
            post(store_auth_logout),
        )
        .route(
            "/instances/:instance_id/auth/client-secret",
            post(store_auth_save_client_secret),
        )
        .route(
            "/instances/:instance_id/auth/private-key",
            post(store_auth_save_private_key),
        )
        .route(
            "/instances/:instance_id/auth/scope-upgrade",
            post(store_auth_scope_upgrade),
        )
        .route(
            "/instances/:instance_id/disconnect",
            post(store_disconnect_service),
        )
        .route(
            "/instances/:instance_id/restart",
            post(store_restart_service),
        )
        .route("/instances/:instance_id/wait", get(store_wait_service))
        .route("/instances/:instance_id/tools", get(store_list_tools))
        .route(
            "/instances/:instance_id/tool-policy",
            get(store_get_tool_policy)
                .put(store_set_tool_policy)
                .delete(store_clear_tool_policy),
        )
        .route("/instances/:instance_id/call", post(store_call_tool))
        .route("/tool_transforms", get(store_list_tool_transforms))
        .route(
            "/instances/:instance_id/tool_transforms/:tool_name",
            get(store_get_tool_transform_by_path)
                .put(store_set_tool_transform_by_path)
                .delete(store_delete_tool_transform_by_path),
        )
        .route("/openapi_imports", get(store_list_openapi_imports))
        .route(
            "/openapi_imports/:name",
            get(store_get_openapi_import_by_path),
        )
        .route(
            "/openapi_imports/:name/import",
            post(store_import_openapi_by_path),
        )
        .route("/openapi_imports/bundle", post(store_bundle_openapi))
        .route(
            "/openapi_imports/bundle_artifact",
            post(store_bundle_openapi_artifact),
        )
        .route(
            "/instances/:instance_id/resources",
            get(store_list_resources),
        )
        .route(
            "/instances/:instance_id/resource_templates",
            get(store_list_resource_templates),
        )
        .route(
            "/instances/:instance_id/read_resource",
            get(store_read_resource),
        )
        .route("/instances/:instance_id/prompts", get(store_list_prompts))
        .route("/instances/:instance_id/get_prompt", post(store_get_prompt))
        .route(
            "/instances/:instance_id/completions",
            post(store_complete_argument),
        )
        .route(
            "/instances/:instance_id/resources/subscribe",
            post(store_subscribe_resource),
        )
        .route(
            "/instances/:instance_id/resources/unsubscribe",
            post(store_unsubscribe_resource),
        )
        .route(
            "/instances/:instance_id/logging/level",
            post(store_set_logging_level),
        )
        .route("/instances/:instance_id/check", get(store_check_service))
        .route("/instances/:instance_id", get(store_service_info))
        .route("/instances/:instance_id/state", get(store_service_state))
        .route("/config", get(store_show_config))
        .route("/client-config/inspect", post(client_config_inspect))
        .route("/client-config/plan", post(client_config_plan))
        .route("/client-config/apply", post(client_config_apply))
        .route("/client-config/undo", post(client_config_undo))
        .route("/client-config/import", post(client_config_import))
        .route("/aggregate/launch", get(aggregate_launch))
        .route("/config/reset", post(store_reset_config))
        .route("/scopes/agents/:agent_id/config", get(agent_show_config))
        .route("/scopes/agents/:agent_id/reset", post(agent_reset_config))
        .route("/cache/health", get(cache_health))
        .route("/cache/inspect", get(cache_inspect))
        .route("/cache/switch", post(cache_switch))
        .with_state(state);

    if prefix.is_empty() {
        base.layer(CorsLayer::permissive())
    } else {
        Router::new()
            .nest(prefix, base)
            .layer(CorsLayer::permissive())
    }
}

async fn aggregate_launch(Query(query): Query<AggregateLaunchQuery>) -> ApiResult {
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

async fn client_config_inspect(Json(request): Json<ClientConfigRequest>) -> ApiResult {
    let client = parse_client_kind(&request.client)?;
    let inspection = inspect_client_config(client, &request.path).map_err(ApiError::from_store)?;
    Ok(success(
        "编程助手配置检查成功",
        inspection_summary(&inspection),
    ))
}

async fn client_config_plan(Json(request): Json<ClientConfigPlanRequest>) -> ApiResult {
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

async fn client_config_apply(
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

async fn client_config_import(
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

async fn client_config_undo(
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

async fn health(State(state): State<Arc<ApiState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "backend": cache_storage_label(state.store.current_cache_storage().await),
    }))
}

async fn app_meta(State(state): State<Arc<ApiState>>) -> ApiResult {
    let payload = app_meta_payload(&state)?;
    Ok(success("应用元信息获取成功", payload))
}

async fn app_update_settings(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> ApiResult {
    let config_manager = state.store.config_manager();
    let mut config = config_manager
        .load_app_config_or_default()
        .map_err(config_api_error)?;

    if let Some(language) = payload.language {
        config.ui.language = normalize_ui_language(&language)?;
    }

    if let Some(default_backup_dir) = payload.default_backup_dir {
        let value = default_backup_dir.trim();
        if value.is_empty() {
            return Err(ApiError::invalid_parameter(
                "默认备份目录不能为空",
                Some("default_backup_dir"),
            ));
        }
        config.ui.default_backup_dir = value.to_string();
    }

    if let Some(logging) = payload.logging {
        if let Some(max_size_bytes) = logging.max_size_bytes {
            if max_size_bytes == 0 {
                return Err(ApiError::invalid_parameter(
                    "日志大小上限必须大于 0",
                    Some("logging.max_size_bytes"),
                ));
            }
            config.ui.logging.max_size_bytes = max_size_bytes;
        }
        if let Some(retention_days) = logging.retention_days {
            config.ui.logging.retention_days = retention_days;
        }
    }

    if let Some(diagnostics) = payload.diagnostics {
        if let Some(enabled) = diagnostics.enabled {
            config.diagnostics.enabled = enabled;
        }
        if let Some(runtime_log) = diagnostics.runtime_log {
            if let Some(enabled) = runtime_log.enabled {
                config.diagnostics.runtime_log.enabled = enabled;
            }
            if let Some(max_size_bytes) = runtime_log.max_size_bytes {
                if max_size_bytes == 0 {
                    return Err(ApiError::invalid_parameter(
                        "运行日志大小上限必须大于 0",
                        Some("diagnostics.runtime_log.max_size_bytes"),
                    ));
                }
                config.diagnostics.runtime_log.max_size_bytes = max_size_bytes;
            }
        }
        if let Some(history) = diagnostics.history {
            if let Some(enabled) = history.enabled {
                config.diagnostics.history.enabled = enabled && config.diagnostics.enabled;
            }
            if let Some(storage) = history.storage {
                config.diagnostics.history.storage = storage;
            }
            if let Some(max_records) = history.max_records {
                if max_records == 0 {
                    return Err(ApiError::invalid_parameter(
                        "调用历史条数上限必须大于 0",
                        Some("diagnostics.history.max_records"),
                    ));
                }
                config.diagnostics.history.max_records = max_records;
            }
            if let Some(max_size_bytes) = history.max_size_bytes {
                if max_size_bytes == 0 {
                    return Err(ApiError::invalid_parameter(
                        "调用历史大小上限必须大于 0",
                        Some("diagnostics.history.max_size_bytes"),
                    ));
                }
                config.diagnostics.history.max_size_bytes = max_size_bytes;
            }
            if let Some(retention_days) = history.retention_days {
                config.diagnostics.history.retention_days = retention_days;
            }
            if let Some(payload) = history.payload {
                config.diagnostics.history.payload = payload;
            }
        }
    }

    config_manager
        .save_app_config(&config)
        .map_err(config_api_error)?;
    state
        .store
        .update_history_config(mcpstore::config::HistoryConfig {
            enabled: config.diagnostics.enabled && config.diagnostics.history.enabled,
            ..config.diagnostics.history.clone()
        })
        .await;

    Ok(success("设置保存成功", settings_payload(&config)))
}

fn app_meta_payload(state: &ApiState) -> Result<Value, ApiError> {
    let config_manager = state.store.config_manager();
    let config = config_manager
        .load_app_config_or_default()
        .map_err(config_api_error)?;
    let config_path = config_manager.app_config_path();
    let config_content = if config_path.exists() {
        std::fs::read_to_string(config_path).map_err(config_io_api_error)?
    } else {
        config_manager
            .default_app_config_toml()
            .map_err(config_api_error)?
    };

    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "settings": settings_payload(&config),
        "settings_paths": settings_paths_payload(config_manager.mcp_path(), &config),
        "config_file": {
            "path": config_path.display().to_string(),
            "format": "toml",
            "content": config_content,
        },
    }))
}

fn settings_payload(config: &AppConfig) -> Value {
    json!({
        "language": api_ui_language(&config.ui.language),
        "default_backup_dir": config.ui.default_backup_dir,
        "logging": {
            "max_size_bytes": config.ui.logging.max_size_bytes,
            "retention_days": config.ui.logging.retention_days,
        },
        "diagnostics": {
            "enabled": config.diagnostics.enabled,
            "runtime_log": {
                "enabled": config.diagnostics.runtime_log.enabled,
                "max_size_bytes": config.diagnostics.runtime_log.max_size_bytes,
            },
            "history": {
                "enabled": config.diagnostics.history.enabled,
                "storage": config.diagnostics.history.storage,
                "max_records": config.diagnostics.history.max_records,
                "max_size_bytes": config.diagnostics.history.max_size_bytes,
                "retention_days": config.diagnostics.history.retention_days,
                "payload": config.diagnostics.history.payload,
            }
        },
    })
}

fn settings_paths_payload(mcp_path: &FsPath, config: &AppConfig) -> Value {
    let base = mcp_path.parent().unwrap_or_else(|| FsPath::new("."));
    let backup_dir = FsPath::new(&config.ui.default_backup_dir);
    let backup_dir_resolved = if backup_dir.is_absolute() {
        backup_dir.to_path_buf()
    } else {
        base.join(backup_dir)
    };
    let log_dir = base.join("logs");
    let log_file_name = "mcpstore.log";

    json!({
        "backup_dir_base": base.display().to_string(),
        "backup_dir_input": config.ui.default_backup_dir,
        "backup_dir_resolved": backup_dir_resolved.display().to_string(),
        "log_dir": log_dir.display().to_string(),
        "log_file_name": log_file_name,
        "log_file_path": log_dir.join(log_file_name).display().to_string(),
    })
}

fn normalize_ui_language(language: &str) -> Result<String, ApiError> {
    match language.trim() {
        "auto" => Ok("auto".to_string()),
        "zh" | "zh-cn" => Ok("zh".to_string()),
        "en" => Ok("en".to_string()),
        _ => Err(ApiError::invalid_parameter(
            "语言必须是 auto、zh 或 en",
            Some("language"),
        )),
    }
}

fn api_ui_language(language: &str) -> &str {
    match language {
        "zh-cn" => "zh",
        value => value,
    }
}

fn config_api_error(error: ConfigError) -> ApiError {
    ApiError::invalid_request(error.to_string())
}

fn config_io_api_error(error: std::io::Error) -> ApiError {
    ApiError::invalid_request(error.to_string())
}

async fn list_agents(State(state): State<Arc<ApiState>>) -> ApiResult {
    let agents = state
        .store
        .list_agents()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 列表获取成功",
        json!({ "agents": agents, "total": agents.len() }),
    ))
}

fn parse_session_scope_param(scope: Option<&str>) -> ApiResult<Option<SessionScope>> {
    match scope {
        None => Ok(None),
        Some("store") => Ok(Some(SessionScope::Store)),
        Some("agent") => Ok(Some(SessionScope::Agent)),
        Some(other) => Err(ApiError::invalid_parameter(
            format!("无效的 session scope: {other}"),
            Some("scope"),
        )),
    }
}

fn require_present_session<T>(
    value: Option<T>,
    session_key: &str,
    label: &str,
) -> Result<T, ApiError> {
    value.ok_or_else(|| {
        ApiError::not_found(
            "SESSION_NOT_FOUND",
            format!("Session not found: session_key={session_key}"),
            Some("session_key"),
            Some(json!({ "session_key": session_key, "resource": label })),
        )
    })
}

async fn session_create(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionCreateRequest>,
) -> ApiResult {
    let scope = parse_session_scope_param(payload.scope.as_deref())?.unwrap_or(SessionScope::Store);
    let session = state
        .store
        .create_session(CreateSessionRequest {
            session_id: payload.session_id,
            scope,
            agent_id: payload.agent_id,
            lease_seconds: payload.lease_seconds,
            metadata: payload.metadata.unwrap_or_else(|| json!({})),
        })
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Session 创建成功", json!({ "session": session })))
}

async fn session_get(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_get_impl(state, session_key).await
}

async fn session_get_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_get_impl(state, query.session_key).await
}

async fn session_get_impl(state: Arc<ApiState>, session_key: String) -> ApiResult {
    let session = state
        .store
        .get_session(&session_key)
        .await
        .map_err(ApiError::from_store)?;
    let session = require_present_session(session, &session_key, "session")?;
    Ok(success("Session 获取成功", json!({ "session": session })))
}

async fn session_find(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionFindQuery>,
) -> ApiResult {
    let scope = parse_session_scope_param(query.scope.as_deref())?.unwrap_or(SessionScope::Store);
    let session = state
        .store
        .find_session(scope, query.agent_id.as_deref(), &query.session_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Session 查找成功", json!({ "session": session })))
}

async fn session_list(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionListQuery>,
) -> ApiResult {
    let scope = parse_session_scope_param(query.scope.as_deref())?;
    let sessions = state
        .store
        .list_sessions(scope, query.agent_id.as_deref())
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session 列表获取成功",
        json!({ "sessions": sessions, "total": sessions.len() }),
    ))
}

async fn session_export_snapshot(State(state): State<Arc<ApiState>>) -> ApiResult {
    let snapshot = state
        .store
        .export_sessions_snapshot()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session snapshot 导出成功",
        json!({ "snapshot": snapshot }),
    ))
}

async fn session_import_snapshot(
    State(state): State<Arc<ApiState>>,
    Json(snapshot): Json<Value>,
) -> ApiResult {
    let report = state
        .store
        .import_sessions_snapshot(snapshot)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session snapshot 导入成功",
        json!({ "report": report }),
    ))
}

async fn session_status(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_status_impl(state, session_key).await
}

async fn session_status_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_status_impl(state, query.session_key).await
}

async fn session_status_impl(state: Arc<ApiState>, session_key: String) -> ApiResult {
    let status = state
        .store
        .get_session_status(&session_key)
        .await
        .map_err(ApiError::from_store)?;
    let status = require_present_session(status, &session_key, "session_status")?;
    Ok(success("Session 状态获取成功", json!({ "status": status })))
}

async fn session_close(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionCloseRequest>,
) -> ApiResult {
    session_close_impl(state, session_key, payload.reason).await
}

async fn session_close_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionCloseRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_close_impl(state, session_key, payload.reason).await
}

async fn session_close_impl(
    state: Arc<ApiState>,
    session_key: String,
    reason: Option<String>,
) -> ApiResult {
    let status = state
        .store
        .close_session(&session_key, reason)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Session 已关闭", json!({ "status": status })))
}

async fn session_extend(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionExtendRequest>,
) -> ApiResult {
    session_extend_impl(state, session_key, payload.lease_seconds).await
}

async fn session_extend_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionExtendRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_extend_impl(state, session_key, payload.lease_seconds).await
}

async fn session_extend_impl(
    state: Arc<ApiState>,
    session_key: String,
    lease_seconds: i64,
) -> ApiResult {
    let session = state
        .store
        .extend_session(&session_key, lease_seconds)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Session 已续期", json!({ "session": session })))
}

async fn session_bind_service(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    session_bind_service_impl(state, session_key, payload.instance_id).await
}

async fn session_bind_service_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_bind_service_impl(state, session_key, payload.instance_id).await
}

async fn session_bind_service_impl(
    state: Arc<ApiState>,
    session_key: String,
    instance_id: InstanceId,
) -> ApiResult {
    let relation = state
        .store
        .bind_service_to_session(&session_key, instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session 服务绑定成功",
        json!({ "relation": relation }),
    ))
}

async fn session_unbind_service(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    session_unbind_service_impl(state, session_key, payload.instance_id).await
}

async fn session_unbind_service_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_unbind_service_impl(state, session_key, payload.instance_id).await
}

async fn session_unbind_service_impl(
    state: Arc<ApiState>,
    session_key: String,
    instance_id: InstanceId,
) -> ApiResult {
    let relation = state
        .store
        .unbind_service_from_session(&session_key, instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session 服务解绑成功",
        json!({ "relation": relation }),
    ))
}

async fn session_list_services(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_list_services_impl(state, session_key).await
}

async fn session_list_services_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_list_services_impl(state, query.session_key).await
}

async fn session_list_services_impl(state: Arc<ApiState>, session_key: String) -> ApiResult {
    let services = state
        .store
        .list_session_services(&session_key)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session 服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

async fn session_list_tools(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_list_tools_impl(state, session_key).await
}

async fn session_list_tools_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_list_tools_impl(state, query.session_key).await
}

async fn session_list_tools_impl(state: Arc<ApiState>, session_key: String) -> ApiResult {
    let tools = state
        .store
        .list_tools_in_session(&session_key)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session 工具列表获取成功",
        json!({ "tools": tools, "total": tools.len() }),
    ))
}

async fn session_call_tool(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    session_call_tool_impl(state, session_key, payload).await
}

async fn session_call_tool_by_body(
    State(state): State<Arc<ApiState>>,
    Json(mut payload): Json<Value>,
) -> ApiResult {
    let session_key = payload
        .get("session_key")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    if let Some(object) = payload.as_object_mut() {
        object.remove("session_key");
    }
    session_call_tool_impl(state, session_key, payload).await
}

async fn session_call_tool_impl(
    state: Arc<ApiState>,
    session_key: String,
    payload: Value,
) -> ApiResult {
    let instance_id = payload
        .get("instance_id")
        .cloned()
        .ok_or_else(|| ApiError::missing_parameter("instance_id"))
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|error| ApiError::invalid_request(format!("instance_id 无效: {error}")))
        })?;
    let tool_name = extract_tool_name(&payload)?;
    let args = extract_tool_args(&payload)?;
    let result = state
        .store
        .call_tool_in_session(&session_key, instance_id, &tool_name, args)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session 工具调用完成",
        serde_json::to_value(result).unwrap_or(Value::Null),
    ))
}

async fn session_list_state(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_list_state_impl(state, session_key).await
}

async fn session_list_state_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_list_state_impl(state, query.session_key).await
}

async fn session_list_state_impl(state: Arc<ApiState>, session_key: String) -> ApiResult {
    let session_state = state
        .store
        .list_session_state(&session_key)
        .await
        .map_err(ApiError::from_store)?;
    let values = session_state.values.clone();
    Ok(success(
        "Session state 获取成功",
        json!({
            "state": session_state,
            "values": values,
        }),
    ))
}

async fn session_get_state_value(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionStateValueQuery>,
) -> ApiResult {
    let value = state
        .store
        .get_session_state_value(&query.session_key, &query.key)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session state value 获取成功",
        json!({ "key": query.key, "value": value }),
    ))
}

async fn session_set_state(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionStateSetRequest>,
) -> ApiResult {
    session_set_state_impl(state, session_key, payload.key, payload.value).await
}

async fn session_set_state_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionStateSetRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_set_state_impl(state, session_key, payload.key, payload.value).await
}

async fn session_set_state_impl(
    state: Arc<ApiState>,
    session_key: String,
    key: String,
    value: Value,
) -> ApiResult {
    let session_state = state
        .store
        .set_session_state(&session_key, &key, value)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session state 设置成功",
        json!({ "state": session_state }),
    ))
}

async fn session_delete_state(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionStateDeleteRequest>,
) -> ApiResult {
    session_delete_state_impl(state, session_key, payload.key).await
}

async fn session_delete_state_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionStateDeleteRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_delete_state_impl(state, session_key, payload.key).await
}

async fn session_delete_state_impl(
    state: Arc<ApiState>,
    session_key: String,
    key: String,
) -> ApiResult {
    let session_state = state
        .store
        .delete_session_state(&session_key, &key)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session state 删除成功",
        json!({ "state": session_state }),
    ))
}

async fn session_clear_state(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_clear_state_impl(state, session_key).await
}

async fn session_clear_state_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionStateClearRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_clear_state_impl(state, session_key).await
}

async fn session_clear_state_impl(state: Arc<ApiState>, session_key: String) -> ApiResult {
    let session_state = state
        .store
        .clear_session_state(&session_key)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session state 清理成功",
        json!({ "state": session_state }),
    ))
}

async fn event_history(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let count = params
        .get("count")
        .map(String::as_str)
        .map(parse_positive_usize)
        .transpose()?
        .unwrap_or(100);
    let events = state.store.event_history(count).await;
    Ok(success(
        "事件历史获取成功",
        json!({ "events": events, "total": events.len() }),
    ))
}

async fn tool_call_history(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let count = params
        .get("count")
        .map(String::as_str)
        .map(parse_positive_usize)
        .transpose()?
        .unwrap_or(100);
    let records = state.store.tool_call_history(count).await;
    Ok(success(
        "工具调用历史获取成功",
        json!({ "records": records, "total": records.len() }),
    ))
}

async fn clear_tool_call_history(State(state): State<Arc<ApiState>>) -> ApiResult {
    state
        .store
        .clear_tool_call_history()
        .await
        .map_err(config_io_api_error)?;
    Ok(success("工具调用历史已清空", json!({})))
}

async fn event_capability_report(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state.store.event_capability_report().await;
    Ok(success("事件能力报告获取成功", report))
}

async fn store_list_services(State(state): State<Arc<ApiState>>) -> ApiResult {
    let services = state
        .store
        .list_services_scoped(&ScopeRef::Store)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

async fn add_service_definition(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let config: ServerConfig = serde_json::from_value(payload)
        .map_err(|error| ApiError::invalid_request(format!("服务配置无效: {error}")))?;
    state
        .store
        .add_service(&service_name, config)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务定义添加成功", json!({ "status": "ok" })))
}

async fn update_service_definition(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    if payload
        .as_object()
        .is_some_and(|config| config.contains_key("_mcpstore"))
    {
        return Err(ApiError::invalid_request(
            "基础配置更新不能包含 _mcpstore；请使用作用域接口修改 scope",
        ));
    }
    let config: ServerConfig = serde_json::from_value(payload)
        .map_err(|error| ApiError::invalid_request(format!("服务配置无效: {error}")))?;
    state
        .store
        .update_service(&service_name, config)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务定义更新成功", json!({ "status": "ok" })))
}

async fn remove_service_definition(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .remove_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务定义删除成功", json!({ "status": "ok" })))
}

async fn declare_store_scope(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Json(descriptor): Json<ScopeDescriptor>,
) -> ApiResult {
    state
        .store
        .declare_service_scope(&service_name, &ScopeRef::Store, descriptor)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Store 作用域已声明", json!({ "status": "ok" })))
}

async fn remove_store_scope(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .remove_service_scope(&service_name, &ScopeRef::Store)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Store 作用域已删除", json!({ "status": "ok" })))
}

fn parse_config_format(value: Option<&str>) -> Result<ConfigFormat, ApiError> {
    value
        .unwrap_or("native")
        .parse()
        .map_err(ApiError::from_store)
}

async fn store_auth_status(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("认证状态获取成功", json!({ "auth": auth })))
}

async fn store_auth_start(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    match auth.flow {
        Some(AuthFlow::AuthorizationCode) => {
            let authorization = state
                .store
                .begin_authorization(instance_id)
                .await
                .map_err(ApiError::from_store)?;
            let auth = state
                .store
                .auth_status_view(instance_id)
                .await
                .map_err(ApiError::from_store)?;
            Ok(success(
                "授权已开始",
                json!({ "auth": auth, "authorization": authorization }),
            ))
        }
        Some(AuthFlow::ClientCredentials) => {
            state
                .store
                .refresh_authorization(instance_id)
                .await
                .map_err(ApiError::from_store)?;
            reconnect_authorized_service(&state, instance_id).await?;
            let auth = state
                .store
                .auth_status_view(instance_id)
                .await
                .map_err(ApiError::from_store)?;
            Ok(success(
                "客户端凭证授权成功",
                json!({ "auth": auth, "authorization": null }),
            ))
        }
        None => Err(ApiError::from_store(mcpstore::StoreError::Auth(
            mcpstore::AuthError::UnsupportedFlow,
        ))),
    }
}

async fn store_auth_callback_get(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Query(query): Query<AuthCallbackQuery>,
) -> ApiResult {
    let code = query
        .code
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::missing_parameter("code"))?;
    let csrf_state = query
        .state
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::missing_parameter("state"))?;
    state
        .store
        .complete_authorization_callback(instance_id, code, csrf_state, query.issuer.as_deref())
        .await
        .map_err(ApiError::from_store)?;
    reconnect_authorized_service(&state, instance_id).await?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("授权回调处理成功", json!({ "auth": auth })))
}

async fn store_auth_callback_post(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<AuthCallbackRequest>,
) -> ApiResult {
    if payload.callback_url.trim().is_empty() {
        return Err(ApiError::invalid_parameter(
            "callback_url 不能为空",
            Some("callback_url"),
        ));
    }
    state
        .store
        .complete_authorization(instance_id, &payload.callback_url)
        .await
        .map_err(ApiError::from_store)?;
    reconnect_authorized_service(&state, instance_id).await?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("授权回调处理成功", json!({ "auth": auth })))
}

async fn store_auth_refresh(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .refresh_authorization(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    reconnect_authorized_service(&state, instance_id).await?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("授权刷新成功", json!({ "auth": auth })))
}

async fn store_auth_logout(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .logout_authorization(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("授权已退出", json!({ "auth": auth })))
}

async fn store_auth_save_client_secret(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<AuthClientSecretRequest>,
) -> ApiResult {
    state
        .store
        .save_oauth_client_secret(instance_id, payload.client_secret)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("客户端密钥已安全保存", json!({ "stored": true })))
}

async fn store_auth_save_private_key(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<AuthPrivateKeyRequest>,
) -> ApiResult {
    state
        .store
        .save_oauth_private_key(instance_id, payload.private_key_pem.into_bytes())
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("私钥已安全保存", json!({ "stored": true })))
}

async fn store_auth_scope_upgrade(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<AuthScopeUpgradeRequest>,
) -> ApiResult {
    let authorization = state
        .store
        .begin_scope_upgrade(instance_id, &payload.required_scope)
        .await
        .map_err(ApiError::from_store)?;
    let auth = state
        .store
        .auth_status_view(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "权限范围升级授权已开始",
        json!({ "auth": auth, "authorization": authorization }),
    ))
}

async fn reconnect_authorized_service(
    state: &Arc<ApiState>,
    instance_id: InstanceId,
) -> Result<(), ApiError> {
    state.store.disconnect_service(instance_id).await.ok();
    state
        .store
        .connect_service(instance_id)
        .await
        .map_err(ApiError::from_store)
}

async fn store_connect_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .connect_service(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务连接成功", json!({ "status": "ok" })))
}

async fn store_disconnect_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .disconnect_service(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务断开成功", json!({ "status": "ok" })))
}

async fn store_restart_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .restart_service(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务重启成功", json!({ "status": "ok" })))
}

async fn store_wait_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let timeout = params
        .get("timeout")
        .map(String::as_str)
        .map(parse_positive_u64)
        .transpose()?
        .unwrap_or(10);
    let status = state
        .store
        .wait_instance_ready(instance_id, timeout)
        .await
        .map_err(ApiError::from_store)?;
    let status = serde_json::to_value(status)
        .map_err(|error| ApiError::invalid_request(format!("服务状态序列化失败: {error}")))?;
    Ok(success("服务等待完成", status))
}

async fn store_list_tools(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Query(query): Query<ToolListQuery>,
) -> ApiResult {
    let filter = match query.filter.as_deref().unwrap_or("available") {
        "all" => mcpstore::ToolVisibilityFilter::All,
        "available" => mcpstore::ToolVisibilityFilter::Available,
        "removed" => mcpstore::ToolVisibilityFilter::Removed,
        value => {
            return Err(ApiError::invalid_parameter(
                format!("不支持的工具过滤器: {value}"),
                Some("filter"),
            ));
        }
    };
    let filter_name = match filter {
        mcpstore::ToolVisibilityFilter::All => "all",
        mcpstore::ToolVisibilityFilter::Available => "available",
        mcpstore::ToolVisibilityFilter::Removed => "removed",
    };
    let tools = state
        .store
        .list_tools_for_instance_with_filter(instance_id, filter)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具列表获取成功",
        json!({ "filter": filter_name, "tools": tools, "total": tools.len() }),
    ))
}

async fn store_get_tool_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let policy = state
        .store
        .get_context_tool_visibility(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具策略获取成功", json!({ "policy": policy })))
}

async fn store_set_tool_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<ToolVisibilityRequest>,
) -> ApiResult {
    let policy = state
        .store
        .set_context_tool_visibility(instance_id, payload.available_tools)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具策略更新成功", json!({ "policy": policy })))
}

async fn store_clear_tool_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .clear_context_tool_visibility(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具策略已清除", json!({ "policy": null })))
}

async fn store_call_tool(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let tool_name = extract_tool_name(&payload)?;
    let args = extract_tool_args(&payload)?;
    let result = state
        .store
        .call_tool(instance_id, &tool_name, args)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具调用完成",
        serde_json::to_value(result).unwrap_or(Value::Null),
    ))
}

async fn store_list_tool_transforms(State(state): State<Arc<ApiState>>) -> ApiResult {
    let transforms = state
        .store
        .list_tool_transforms()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具转换规则列表获取成功",
        json!({ "transforms": transforms, "total": transforms.len() }),
    ))
}

async fn store_get_tool_transform_by_path(
    State(state): State<Arc<ApiState>>,
    Path((instance_id, tool_name)): Path<(InstanceId, String)>,
) -> ApiResult {
    let transform = state
        .store
        .get_tool_transform(instance_id, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具转换规则获取成功",
        json!({ "transform": transform }),
    ))
}

async fn store_set_tool_transform_by_path(
    State(state): State<Arc<ApiState>>,
    Path((instance_id, tool_name)): Path<(InstanceId, String)>,
    Json(transform): Json<ToolTransformPatch>,
) -> ApiResult {
    let transform = state
        .store
        .set_tool_transform(instance_id, &tool_name, transform)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具转换规则设置成功",
        json!({ "transform": transform }),
    ))
}

async fn store_delete_tool_transform_by_path(
    State(state): State<Arc<ApiState>>,
    Path((instance_id, tool_name)): Path<(InstanceId, String)>,
) -> ApiResult {
    state
        .store
        .delete_tool_transform(instance_id, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具转换规则删除成功", json!({ "status": "ok" })))
}

async fn store_list_openapi_imports(State(state): State<Arc<ApiState>>) -> ApiResult {
    let imports = state
        .store
        .list_openapi_imports()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "OpenAPI 导入列表获取成功",
        json!({ "imports": imports, "total": imports.len() }),
    ))
}

async fn store_get_openapi_import_by_path(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
) -> ApiResult {
    let import = state
        .store
        .get_openapi_import(&name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "OpenAPI 导入结果获取成功",
        json!({ "import": import }),
    ))
}

async fn store_import_openapi_by_path(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
    Json(payload): Json<OpenApiImportRequest>,
) -> ApiResult {
    let options = OpenApiImportOptions {
        headers: payload.headers,
        auth: payload.auth,
        ref_cache: payload.ref_cache,
        timeout_millis: openapi_timeout_millis(payload.timeout_millis, "timeout_millis")?
            .unwrap_or_else(OpenApiImportOptions::default_timeout_millis),
        fetch_timeout_millis: openapi_timeout_millis(
            payload.fetch_timeout_millis,
            "fetch_timeout_millis",
        )?
        .unwrap_or_else(OpenApiImportOptions::default_fetch_timeout_millis),
    };
    store_import_openapi_impl(
        state,
        name,
        payload.spec_url,
        payload.spec,
        payload.spec_text,
        options,
    )
    .await
}

async fn store_import_openapi_impl(
    state: Arc<ApiState>,
    name: String,
    spec_url: String,
    spec: Option<Value>,
    spec_text: Option<String>,
    options: OpenApiImportOptions,
) -> ApiResult {
    let import = match (spec, payload_spec_text(spec_text)) {
        (Some(_), Some(_)) => {
            return Err(ApiError::invalid_request(
                "spec and spec_text cannot both be provided",
            ));
        }
        (Some(spec), None) => {
            state
                .store
                .import_openapi_service_from_spec_with_options(&name, &spec_url, spec, options)
                .await
        }
        (None, Some(spec_text)) => {
            state
                .store
                .import_openapi_service_from_spec_text_with_options(
                    &name, &spec_url, &spec_text, options,
                )
                .await
        }
        (None, None) => {
            state
                .store
                .import_openapi_service_with_options(&name, &spec_url, options)
                .await
        }
    }
    .map_err(ApiError::from_store)?;
    Ok(success("OpenAPI 导入成功", json!({ "import": import })))
}

async fn store_bundle_openapi(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<OpenApiImportRequest>,
) -> ApiResult {
    let options = OpenApiBundleOptions {
        ref_cache: payload.ref_cache,
        timeout_millis: openapi_timeout_millis(
            payload.fetch_timeout_millis,
            "fetch_timeout_millis",
        )?
        .or(openapi_timeout_millis(
            payload.timeout_millis,
            "timeout_millis",
        )?)
        .unwrap_or_else(OpenApiBundleOptions::default_timeout_millis),
    };
    let bundle = match (payload.spec, payload_spec_text(payload.spec_text)) {
        (Some(_), Some(_)) => {
            return Err(ApiError::invalid_request(
                "spec and spec_text cannot both be provided",
            ));
        }
        (Some(spec), None) => {
            state
                .store
                .bundle_openapi_spec_from_value_with_options(&payload.spec_url, spec, options)
                .await
        }
        (None, Some(spec_text)) => {
            state
                .store
                .bundle_openapi_spec_from_text_with_options(&payload.spec_url, &spec_text, options)
                .await
        }
        (None, None) => {
            state
                .store
                .bundle_openapi_spec_with_options(&payload.spec_url, options)
                .await
        }
    }
    .map_err(ApiError::from_store)?;
    Ok(success("OpenAPI 规范打包成功", json!({ "bundle": bundle })))
}

async fn store_bundle_openapi_artifact(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<OpenApiImportRequest>,
) -> ApiResult {
    let options = OpenApiBundleOptions {
        ref_cache: payload.ref_cache,
        timeout_millis: openapi_timeout_millis(
            payload.fetch_timeout_millis,
            "fetch_timeout_millis",
        )?
        .or(openapi_timeout_millis(
            payload.timeout_millis,
            "timeout_millis",
        )?)
        .unwrap_or_else(OpenApiBundleOptions::default_timeout_millis),
    };
    let artifact = match (payload.spec, payload_spec_text(payload.spec_text)) {
        (Some(_), Some(_)) => {
            return Err(ApiError::invalid_request(
                "spec and spec_text cannot both be provided",
            ));
        }
        (Some(spec), None) => {
            state
                .store
                .bundle_openapi_artifact_from_value_with_options(&payload.spec_url, spec, options)
                .await
        }
        (None, Some(spec_text)) => {
            state
                .store
                .bundle_openapi_artifact_from_text_with_options(
                    &payload.spec_url,
                    &spec_text,
                    options,
                )
                .await
        }
        (None, None) => {
            state
                .store
                .bundle_openapi_artifact_with_options(&payload.spec_url, options)
                .await
        }
    }
    .map_err(ApiError::from_store)?;
    Ok(success(
        "OpenAPI 规范打包产物获取成功",
        json!({ "artifact": artifact }),
    ))
}

fn payload_spec_text(spec_text: Option<String>) -> Option<String> {
    spec_text.filter(|text| !text.trim().is_empty())
}

fn openapi_timeout_millis(value: Option<u64>, field: &'static str) -> ApiResult<Option<u64>> {
    match value {
        Some(0) => Err(ApiError::invalid_parameter(
            format!("{field} must be a positive integer"),
            Some(field),
        )),
        other => Ok(other),
    }
}

async fn store_list_resources(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let resources = state
        .store
        .list_resources_for_instance(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "资源列表获取成功",
        json!({ "resources": resources, "total": resources.len() }),
    ))
}

async fn store_list_resource_templates(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let templates = state
        .store
        .list_resource_templates_for_instance(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "资源模板列表获取成功",
        json!({ "resource_templates": templates, "total": templates.len() }),
    ))
}

async fn store_read_resource(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let uri = extract_resource_uri(&params)?;
    let result = state
        .store
        .read_resource_scoped(instance_id, &uri)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("资源读取成功", result))
}

async fn store_list_prompts(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let prompts = state
        .store
        .list_prompts_for_instance(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Prompt 列表获取成功",
        json!({ "prompts": prompts, "total": prompts.len() }),
    ))
}

async fn store_get_prompt(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let prompt_name = extract_prompt_name(&payload)?;
    let args = extract_prompt_args(&payload)?;
    let result = state
        .store
        .get_prompt_scoped(instance_id, &prompt_name, args)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Prompt 获取成功", result))
}

async fn store_complete_argument(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<McpCompletionRequest>,
) -> ApiResult {
    let completion = state
        .store
        .complete_mcp_argument(instance_id, payload)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("参数补全成功", json!(completion)))
}

async fn store_subscribe_resource(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<ResourceSubscriptionRequest>,
) -> ApiResult {
    let uri = payload.uri.trim();
    if uri.is_empty() {
        return Err(ApiError::invalid_parameter(
            "资源 URI 不能为空",
            Some("uri"),
        ));
    }
    state
        .store
        .subscribe_resource_updates(instance_id, uri)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("资源更新订阅成功", json!({ "uri": uri })))
}

async fn store_unsubscribe_resource(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<ResourceSubscriptionRequest>,
) -> ApiResult {
    let uri = payload.uri.trim();
    if uri.is_empty() {
        return Err(ApiError::invalid_parameter(
            "资源 URI 不能为空",
            Some("uri"),
        ));
    }
    state
        .store
        .unsubscribe_resource_updates(instance_id, uri)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("资源更新订阅已取消", json!({ "uri": uri })))
}

async fn store_set_logging_level(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<LoggingLevelRequest>,
) -> ApiResult {
    state
        .store
        .set_mcp_logging_level(instance_id, payload.level)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "远端日志级别设置成功",
        json!({ "level": payload.level }),
    ))
}

async fn store_check_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let result = state
        .store
        .health_check(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务检查完成", json!(result)))
}

async fn store_service_info(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let service = state
        .store
        .service_info_scoped(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务信息获取成功", service))
}

async fn store_service_state(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let service_state = state
        .store
        .service_state(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务状态获取成功", service_state))
}

async fn store_show_config(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ShowConfigQuery>,
) -> ApiResult {
    let format = parse_config_format(query.format.as_deref())?;
    let config = if format == ConfigFormat::Native {
        state.store.show_config().await
    } else {
        let instance_id = query
            .instance_id
            .ok_or_else(|| ApiError::missing_parameter("instance_id"))?;
        state
            .store
            .export_instance_config(instance_id, format)
            .await
    }
    .map_err(ApiError::from_store)?;
    Ok(success("配置获取成功", config))
}

async fn store_reset_config(State(state): State<Arc<ApiState>>) -> ApiResult {
    state
        .store
        .reset_config()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("配置重置成功", json!({ "status": "ok" })))
}

async fn agent_list_services(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
) -> ApiResult {
    let services = state
        .store
        .list_services_scoped(&ScopeRef::Agent {
            agent_id: agent_id.clone(),
        })
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

async fn declare_agent_scope(
    State(state): State<Arc<ApiState>>,
    Path((service_name, agent_id)): Path<(String, String)>,
    Json(descriptor): Json<ScopeDescriptor>,
) -> ApiResult {
    state
        .store
        .declare_service_scope(&service_name, &ScopeRef::Agent { agent_id }, descriptor)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 作用域已声明", json!({ "status": "ok" })))
}

async fn remove_agent_scope(
    State(state): State<Arc<ApiState>>,
    Path((service_name, agent_id)): Path<(String, String)>,
) -> ApiResult {
    state
        .store
        .remove_service_scope(&service_name, &ScopeRef::Agent { agent_id })
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 作用域已删除", json!({ "status": "ok" })))
}

async fn agent_show_config(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(query): Query<ShowConfigQuery>,
) -> ApiResult {
    let format = parse_config_format(query.format.as_deref())?;
    let scope = ScopeRef::Agent { agent_id };
    let config = if format == ConfigFormat::Native {
        state.store.show_scope_config(&scope).await
    } else {
        let instance_id = query
            .instance_id
            .ok_or_else(|| ApiError::missing_parameter("instance_id"))?;
        state
            .store
            .export_instance_config(instance_id, format)
            .await
    }
    .map_err(ApiError::from_store)?;
    Ok(success("Agent 配置获取成功", config))
}

async fn agent_reset_config(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
) -> ApiResult {
    state
        .store
        .reset_scope(&ScopeRef::Agent { agent_id })
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 配置重置成功", json!({ "status": "ok" })))
}

async fn cache_inspect(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .cache_inspect()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("缓存视图获取成功", report))
}

async fn cache_health(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .cache_health_check()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("缓存健康检查成功", report))
}

async fn cache_switch(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<CacheSwitchRequest>,
) -> ApiResult {
    let cache_storage = parse_cache_storage(&payload.backend)?;
    let snapshot = state
        .store
        .switch_cache_storage(cache_storage, payload.redis_url, payload.namespace)
        .await
        .map_err(ApiError::from_store)?;
    if !state.store.is_db_source() {
        spawn_control_reactor(state.store.clone());
    }
    let snapshot = serde_json::to_value(snapshot)
        .map_err(|error| ApiError::invalid_request(format!("缓存切换结果序列化失败: {error}")))?;
    Ok(success("缓存后端切换成功", snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpstore::{
        cache::models::{
            InstanceToolRelation, ServiceDefinitionEntity, ServiceInstanceEntity, ToolEntity,
        },
        config::{McpStoreExtension, ScopeDeclarations},
        registry::ConfigRevision,
        CacheStorage, ServiceInstanceKey, SourceMode, StoreOptions,
    };
    use std::{
        collections::HashMap,
        net::SocketAddr,
        sync::atomic::{AtomicUsize, Ordering},
        time::SystemTime,
    };

    static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn unique_namespace() -> String {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        format!("api-session-test-{nanos}")
    }

    fn unique_temp_dir_path(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let count = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}-{count}", std::process::id()))
    }

    fn stdio_config() -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("echo".to_string()),
            args: vec!["fixture".to_string()],
            env: HashMap::new(),
            headers: HashMap::new(),
            auth: Default::default(),
            transport: Some("stdio".to_string()),
            working_dir: None,
            description: Some("fixture".to_string()),
            mcpstore: None,
            extra: Default::default(),
        }
    }

    fn stdio_config_with_lifecycle() -> ServerConfig {
        let mut config = stdio_config();
        config.mcpstore = Some(mcpstore::config::McpStoreExtension {
            scopes: ScopeDeclarations::store_only(),
            lifecycle: Some(mcpstore::config::ServiceLifecycleConfig {
                startup_policy: Some(mcpstore::config::StartupPolicy::Lazy),
                restart_policy: Some(mcpstore::config::RestartPolicy {
                    kind: mcpstore::config::RestartPolicyKind::OnFailure,
                    max_retries: Some(3),
                }),
            }),
            ..mcpstore::config::McpStoreExtension::default()
        });
        config
    }

    async fn seed_db_service(store: &MCPStore) {
        let config = stdio_config();
        seed_db_service_config(store, config).await;
    }

    async fn seed_db_service_config(store: &MCPStore, config: ServerConfig) {
        let cache = store.cache();
        let scope = ScopeRef::Store;
        let instance_id = ServiceInstanceKey::new("demo", scope.clone()).instance_id();
        let base_config = config.base_config();
        let lifecycle = config
            .mcpstore
            .as_ref()
            .and_then(|extension| extension.lifecycle.clone());
        let metadata = config
            .mcpstore
            .as_ref()
            .map(|extension| extension.extra.clone())
            .unwrap_or_default();
        cache
            .put_entity(
                "service_definitions",
                "demo",
                serde_json::to_value(ServiceDefinitionEntity {
                    service_name: "demo".to_string(),
                    base_config: base_config.clone(),
                    scopes: ScopeDeclarations::store_only(),
                    lifecycle,
                    metadata,
                    base_revision: 1,
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        cache
            .put_entity(
                "service_instances",
                &instance_id.to_string(),
                serde_json::to_value(ServiceInstanceEntity {
                    instance_id,
                    service_name: "demo".to_string(),
                    scope: scope.clone(),
                    transport: "stdio".to_string(),
                    url: None,
                    command: Some("echo".to_string()),
                    effective_config: base_config,
                    config_revision: ConfigRevision {
                        base_revision: 1,
                        scope_revision: 1,
                    },
                    applied_config_revision: None,
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        cache
            .put_entity(
                "tools",
                &format!("{instance_id}:echo"),
                serde_json::to_value(ToolEntity {
                    instance_id,
                    service_name: "demo".to_string(),
                    scope: scope.clone(),
                    tool_name: "echo".to_string(),
                    title: None,
                    description: "echo tool".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Original text."},
                            "debug": {"type": "boolean"}
                        },
                        "required": ["text", "debug"]
                    }),
                    output_schema: None,
                    annotations: None,
                    meta: None,
                    created_time: 111,
                    tool_hash: "fixture".to_string(),
                })
                .unwrap(),
            )
            .await
            .unwrap();
        cache
            .put_relation(
                "instance_tools",
                &instance_id.to_string(),
                serde_json::to_value(InstanceToolRelation {
                    instance_id,
                    service_name: "demo".to_string(),
                    scope,
                    tools: vec!["echo".to_string()],
                })
                .unwrap(),
            )
            .await
            .unwrap();
    }

    async fn spawn_test_api(store: Arc<MCPStore>) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let (addr, handle, _) = spawn_test_api_with_state(store).await;
        (addr, handle)
    }

    async fn spawn_test_api_with_state(
        store: Arc<MCPStore>,
    ) -> (SocketAddr, tokio::task::JoinHandle<()>, Arc<ApiState>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(ApiState {
            store,
            client_changes: Arc::new(Mutex::new(HashMap::new())),
        });
        let app = router(Arc::clone(&state), "");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (addr, handle, state)
    }

    #[tokio::test]
    async fn client_config_routes_preserve_secrets_and_require_expected_hash_for_undo() {
        let path = unique_temp_dir_path("client-config-api").with_extension("json");
        let secret = "api-secret-do-not-return";
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "mcpServers": {
                    "existing": {
                        "command": "node",
                        "args": ["server.js"],
                        "env": {"TOKEN": secret},
                        "headers": {"Authorization": "Bearer header-secret"}
                    },
                    "new": {"command": "python"},
                    "conflict": {"command": "node"},
                    "unrelated": {"enabled": true}
                },
                "otherSettings": {"keep": true}
            }))
            .unwrap(),
        )
        .unwrap();

        let store_path = unique_temp_dir_path("client-config-api-store").with_extension("json");
        std::fs::write(&store_path, b"{}").unwrap();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(store_path.to_string_lossy().into_owned()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        let (addr, handle, state) = spawn_test_api_with_state(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");
        state
            .store
            .add_service("conflict", ServerConfig {
                command: Some("already-owned".to_string()),
                ..ServerConfig::default()
            })
            .await
            .unwrap();
        let entry = json!({
            "name": "aggregate",
            "kind": "aggregate_stdio",
            "config": {"command": "mcpstore", "args": ["mcp-server"]}
        });

        let inspect = client
            .post(format!("{base_url}/client-config/inspect"))
            .json(&json!({"client": "claude_code", "path": path}))
            .send()
            .await
            .unwrap();
        assert!(inspect.status().is_success());
        let inspect_body = inspect.text().await.unwrap();
        assert!(inspect_body.contains("existing"));
        assert!(!inspect_body.contains(secret));
        assert!(!inspect_body.contains("Bearer header-secret"));

        let inspection: Value = serde_json::from_str(&inspect_body).unwrap();
        let hash = inspection["data"]["content_hash"].as_str().unwrap();

        let plan = client
            .post(format!("{base_url}/client-config/plan"))
            .json(&json!({
                "client": "claude_code",
                "path": path,
                "entries": [entry]
            }))
            .send()
            .await
            .unwrap();
        assert!(plan.status().is_success());
        let plan_body = plan.text().await.unwrap();
        assert!(plan_body.contains("new"));
        assert!(!plan_body.contains(secret));
        assert!(!plan_body.contains("Bearer header-secret"));

        let stale_apply = client
            .post(format!("{base_url}/client-config/apply"))
            .json(&json!({
                "client": "claude_code",
                "path": path,
                "expected_hash": "stale-hash",
                "entries": [entry]
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(stale_apply.status(), axum::http::StatusCode::BAD_REQUEST);

        let apply = client
            .post(format!("{base_url}/client-config/apply"))
            .json(&json!({
                "client": "claude_code",
                "path": path,
                "expected_hash": hash,
                "entries": [entry]
            }))
            .send()
            .await
            .unwrap();
        assert!(apply.status().is_success());
        let apply_payload: Value = apply.json().await.unwrap();
        let change_id = apply_payload["data"]["change_id"].as_str().unwrap();

        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(written["mcpServers"]["aggregate"]["command"], "mcpstore");
        assert_eq!(written["mcpServers"]["unrelated"]["enabled"], true);
        assert_eq!(written["otherSettings"]["keep"], true);
        assert_eq!(written["mcpServers"]["existing"]["env"]["TOKEN"], secret);

        let undo = client
            .post(format!("{base_url}/client-config/undo"))
            .json(&json!({"change_id": change_id}))
            .send()
            .await
            .unwrap();
        assert!(undo.status().is_success());
        let restored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(restored["mcpServers"].get("aggregate").is_none());
        assert_eq!(restored["mcpServers"]["existing"]["env"]["TOKEN"], secret);

        let partial_import = client
            .post(format!("{base_url}/client-config/import"))
            .json(&json!({
                "client": "claude_code",
                "path": path,
                "names": ["new", "conflict"]
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(partial_import.status(), axum::http::StatusCode::BAD_REQUEST);
        assert!(state
            .store
            .get_definition_config("new")
            .await
            .unwrap()
            .is_none());

        let import = client
            .post(format!("{base_url}/client-config/import"))
            .json(&json!({
                "client": "claude_code",
                "path": path,
                "names": ["existing"]
            }))
            .send()
            .await
            .unwrap();
        assert!(import.status().is_success());
        let import_body = import.text().await.unwrap();
        assert!(import_body.contains("existing"));
        assert!(!import_body.contains(secret));
        assert!(!import_body.contains("Bearer header-secret"));
        let imported = state
            .store
            .get_definition_config("existing")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(imported["command"], "node");
        assert_eq!(imported["env"]["TOKEN"], secret);

        let unknown = client
            .post(format!("{base_url}/client-config/undo"))
            .json(&json!({"change_id": "missing-change"}))
            .send()
            .await
            .unwrap();
        assert_eq!(unknown.status(), axum::http::StatusCode::NOT_FOUND);
        let unknown_body = unknown.text().await.unwrap();
        assert!(unknown_body.contains("CHANGE_NOT_FOUND"));

        handle.abort();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&store_path);
        let _ = std::fs::remove_file(path.with_file_name(format!(
            ".{}.mcpstore.lock",
            path.file_name().unwrap().to_string_lossy()
        )));
    }

    #[tokio::test]
    async fn oauth_routes_expose_lifecycle_without_echoing_callback_or_credentials() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        let config = ServerConfig {
            url: Some("http://127.0.0.1:9/mcp".to_string()),
            auth: serde_json::from_value(json!({
                "type": "oauth_authorization_code",
                "client_id": "api-client",
                "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
                "scopes": ["tools.read"]
            }))
            .unwrap(),
            transport: Some("streamable-http".to_string()),
            ..ServerConfig::default()
        };
        seed_db_service_config(&store, config).await;
        store.load_from_source().await.unwrap();
        let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();
        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");

        let status = client
            .get(format!("{base_url}/instances/{instance_id}/auth"))
            .send()
            .await
            .unwrap();
        let status_code = status.status();
        let status_body = status.text().await.unwrap();
        assert!(
            status_code.is_success(),
            "status={status_code} body={status_body}"
        );
        let status_payload = serde_json::from_str::<Value>(&status_body).unwrap();
        assert_eq!(status_payload["data"]["auth"]["status"], "unauthenticated");
        assert_eq!(status_payload["data"]["auth"]["flow"], "authorization_code");
        assert_eq!(
            status_payload["data"]["auth"]["scopes"],
            json!(["tools.read"])
        );

        let callback = client
            .get(format!("{base_url}/instances/{instance_id}/auth/callback"))
            .query(&[
                ("code", "sensitive-code"),
                ("state", "sensitive-state"),
                ("iss", "https://issuer.example"),
            ])
            .send()
            .await
            .unwrap();
        assert_eq!(callback.status(), axum::http::StatusCode::BAD_REQUEST);
        let callback_body = callback.text().await.unwrap();
        assert!(callback_body.contains("AUTH_CALLBACK_REJECTED"));
        assert!(!callback_body.contains("sensitive-code"));
        assert!(!callback_body.contains("sensitive-state"));

        let empty_secret = client
            .post(format!(
                "{base_url}/instances/{instance_id}/auth/client-secret"
            ))
            .json(&json!({"client_secret": ""}))
            .send()
            .await
            .unwrap();
        assert_eq!(empty_secret.status(), axum::http::StatusCode::BAD_REQUEST);
        let secret_body = empty_secret.text().await.unwrap();
        assert!(secret_body.contains("AUTH_CONFIG_INVALID"));
        assert!(!secret_body.contains("client_secret"));

        let empty_key = client
            .post(format!(
                "{base_url}/instances/{instance_id}/auth/private-key"
            ))
            .json(&json!({"private_key_pem": ""}))
            .send()
            .await
            .unwrap();
        assert_eq!(empty_key.status(), axum::http::StatusCode::BAD_REQUEST);
        let key_body = empty_key.text().await.unwrap();
        assert!(key_body.contains("AUTH_CONFIG_INVALID"));
        assert!(!key_body.contains("private_key_pem"));

        handle.abort();
    }

    #[tokio::test]
    async fn session_routes_use_rust_core_session_state_from_shared_cache() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        seed_db_service(&store).await;
        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");

        let create = client
            .post(format!("{base_url}/sessions/create"))
            .json(&json!({
                "session_id": "api-core-session",
                "lease_seconds": 60,
                "metadata": {"owner": "api-test"},
            }))
            .send()
            .await
            .unwrap();
        assert!(create.status().is_success());
        let create_payload = create.json::<Value>().await.unwrap();
        let session_key = create_payload["data"]["session"]["session_key"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(session_key, "store:api-core-session");
        let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();

        let bind = client
            .post(format!("{base_url}/sessions/bind_service"))
            .json(&json!({"session_key": session_key, "instance_id": instance_id}))
            .send()
            .await
            .unwrap();
        assert!(bind.status().is_success());

        let tools = client
            .get(format!("{base_url}/sessions/list_tools"))
            .query(&[("session_key", session_key.as_str())])
            .send()
            .await
            .unwrap();
        assert!(tools.status().is_success());
        let tools_payload = tools.json::<Value>().await.unwrap();
        assert_eq!(tools_payload["data"]["total"], 1);
        assert_eq!(tools_payload["data"]["tools"][0]["name"], "echo");

        let set_state = client
            .post(format!("{base_url}/sessions/state/set"))
            .json(&json!({
                "session_key": session_key,
                "key": "cursor",
                "value": {"page": 1},
            }))
            .send()
            .await
            .unwrap();
        assert!(set_state.status().is_success());
        let set_state_payload = set_state.json::<Value>().await.unwrap();
        assert_eq!(
            set_state_payload["data"]["state"]["values"]["cursor"]["page"],
            1
        );

        let get_state_value = client
            .get(format!("{base_url}/sessions/state/value"))
            .query(&[("session_key", session_key.as_str()), ("key", "cursor")])
            .send()
            .await
            .unwrap();
        assert!(get_state_value.status().is_success());
        let get_state_value_payload = get_state_value.json::<Value>().await.unwrap();
        assert_eq!(get_state_value_payload["data"]["value"]["page"], 1);

        let list_state = client
            .get(format!("{base_url}/sessions/state/list"))
            .query(&[("session_key", session_key.as_str())])
            .send()
            .await
            .unwrap();
        assert!(list_state.status().is_success());
        let list_state_payload = list_state.json::<Value>().await.unwrap();
        assert_eq!(list_state_payload["data"]["values"]["cursor"]["page"], 1);

        let delete_state = client
            .post(format!("{base_url}/sessions/state/delete/{session_key}"))
            .json(&json!({"key": "cursor"}))
            .send()
            .await
            .unwrap();
        assert!(delete_state.status().is_success());
        let delete_state_payload = delete_state.json::<Value>().await.unwrap();
        assert!(delete_state_payload["data"]["state"]["values"]
            .as_object()
            .unwrap()
            .is_empty());

        let set_answer = client
            .post(format!("{base_url}/sessions/state/set/{session_key}"))
            .json(&json!({"key": "answer", "value": 42}))
            .send()
            .await
            .unwrap();
        assert!(set_answer.status().is_success());

        let clear_state = client
            .post(format!("{base_url}/sessions/state/clear"))
            .json(&json!({"session_key": session_key}))
            .send()
            .await
            .unwrap();
        assert!(clear_state.status().is_success());
        let clear_state_payload = clear_state.json::<Value>().await.unwrap();
        assert!(clear_state_payload["data"]["state"]["values"]
            .as_object()
            .unwrap()
            .is_empty());

        let set_path_clear = client
            .post(format!("{base_url}/sessions/state/set/{session_key}"))
            .json(&json!({"key": "path_clear", "value": true}))
            .send()
            .await
            .unwrap();
        assert!(set_path_clear.status().is_success());

        let clear_state_by_path = client
            .post(format!("{base_url}/sessions/state/clear/{session_key}"))
            .send()
            .await
            .unwrap();
        assert!(clear_state_by_path.status().is_success());
        let clear_state_by_path_payload = clear_state_by_path.json::<Value>().await.unwrap();
        assert!(clear_state_by_path_payload["data"]["state"]["values"]
            .as_object()
            .unwrap()
            .is_empty());

        let close = client
            .post(format!("{base_url}/sessions/close"))
            .json(&json!({"session_key": session_key, "reason": "done"}))
            .send()
            .await
            .unwrap();
        assert!(close.status().is_success());

        let closed_tools = client
            .get(format!("{base_url}/sessions/list_tools"))
            .query(&[("session_key", session_key.as_str())])
            .send()
            .await
            .unwrap();
        assert_eq!(closed_tools.status(), axum::http::StatusCode::CONFLICT);
        let closed_payload = closed_tools.json::<Value>().await.unwrap();
        assert_eq!(closed_payload["errors"][0]["code"], "SESSION_NOT_ACTIVE");

        let closed_set_state = client
            .post(format!("{base_url}/sessions/state/set"))
            .json(&json!({
                "session_key": session_key,
                "key": "after_close",
                "value": true,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(closed_set_state.status(), axum::http::StatusCode::CONFLICT);
        let closed_set_state_payload = closed_set_state.json::<Value>().await.unwrap();
        assert_eq!(
            closed_set_state_payload["errors"][0]["code"],
            "SESSION_NOT_ACTIVE"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn third_party_config_export_requires_instance_id() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        seed_db_service_config(&store, stdio_config_with_lifecycle()).await;
        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");

        let native = client
            .get(format!("{base_url}/config"))
            .send()
            .await
            .unwrap();
        assert!(native.status().is_success());
        let native_payload = native.json::<Value>().await.unwrap();
        assert!(native_payload["data"]["mcpServers"]["demo"]
            .get("_mcpstore")
            .is_some());

        let missing_instance = client
            .get(format!("{base_url}/config?format=claude"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            missing_instance.status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        let missing_payload = missing_instance.json::<Value>().await.unwrap();
        assert_eq!(
            missing_payload["errors"][0]["code"],
            json!("MISSING_PARAMETER")
        );

        let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();
        let claude = client
            .get(format!(
                "{base_url}/config?format=claude&instance_id={instance_id}"
            ))
            .send()
            .await
            .unwrap();
        assert!(claude.status().is_success());
        let claude_payload = claude.json::<Value>().await.unwrap();
        assert!(claude_payload["data"]["mcpServers"]["demo"]
            .get("_mcpstore")
            .is_none());
        assert_eq!(
            claude_payload["data"]["mcpServers"]["demo"]["command"],
            json!("echo")
        );

        handle.abort();
    }

    #[tokio::test]
    async fn store_update_only_changes_base_config() {
        let fixture_dir = unique_temp_dir_path("mcpstore-api-update");
        std::fs::create_dir_all(&fixture_dir).unwrap();
        let config_path = fixture_dir.join("mcp.json");
        let store = MCPStore::setup(Some(config_path.to_str().unwrap())).unwrap();
        store
            .add_service("demo", stdio_config_with_lifecycle())
            .await
            .unwrap();
        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");

        let update = client
            .put(format!("{base_url}/services/demo"))
            .json(&json!({
                "command": "printf",
                "args": ["updated"],
                "transport": "stdio",
                "description": "updated fixture"
            }))
            .send()
            .await
            .unwrap();
        assert!(update.status().is_success());

        let native = client
            .get(format!("{base_url}/config"))
            .send()
            .await
            .unwrap();
        assert!(native.status().is_success());
        let native_payload = native.json::<Value>().await.unwrap();
        let service = &native_payload["data"]["mcpServers"]["demo"];
        assert_eq!(service["command"], "printf");
        assert_eq!(service["args"], json!(["updated"]));
        assert!(service["_mcpstore"]["scopes"]["store"].is_object());
        assert_eq!(service["_mcpstore"]["lifecycle"]["startup_policy"], "lazy");

        let invalid = client
            .put(format!("{base_url}/services/demo"))
            .json(&json!({
                "command": "printf",
                "_mcpstore": {"scopes": {"agents": {"agent-a": {}}}}
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(invalid.status(), axum::http::StatusCode::BAD_REQUEST);

        let invalid_null = client
            .put(format!("{base_url}/services/demo"))
            .json(&json!({
                "command": "printf",
                "_mcpstore": null
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(invalid_null.status(), axum::http::StatusCode::BAD_REQUEST);

        handle.abort();
        std::fs::remove_dir_all(fixture_dir).ok();
    }

    #[tokio::test]
    async fn store_scope_endpoint_declares_scope_for_existing_agent_only_definition() {
        let fixture_dir = unique_temp_dir_path("mcpstore-api-store-scope");
        std::fs::create_dir_all(&fixture_dir).unwrap();
        let config_path = fixture_dir.join("mcp.json");
        let store = MCPStore::setup(Some(config_path.to_str().unwrap())).unwrap();
        let mut config = stdio_config();
        let mut scopes = ScopeDeclarations::default();
        scopes
            .agents
            .insert("agent-a".to_string(), ScopeDescriptor::default());
        config.mcpstore = Some(McpStoreExtension {
            scopes,
            ..McpStoreExtension::default()
        });
        store.add_service("demo", config).await.unwrap();

        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");
        let add = client
            .put(format!("{base_url}/services/demo/scopes/store"))
            .json(&json!({
                "config": {
                    "command": "printf",
                    "args": ["store"],
                    "transport": "stdio"
                }
            }))
            .send()
            .await
            .unwrap();
        assert!(add.status().is_success());

        let native = client
            .get(format!("{base_url}/config"))
            .send()
            .await
            .unwrap();
        assert!(native.status().is_success());
        let native_payload = native.json::<Value>().await.unwrap();
        let service = &native_payload["data"]["mcpServers"]["demo"];
        assert!(service["_mcpstore"]["scopes"]["store"].is_object());
        assert!(service["_mcpstore"]["scopes"]["agents"]["agent-a"].is_object());
        assert_eq!(
            service["_mcpstore"]["scopes"]["store"]["config"]["command"],
            "printf"
        );

        handle.abort();
        std::fs::remove_dir_all(fixture_dir).ok();
    }

    #[tokio::test]
    async fn definition_and_scope_routes_are_explicit_and_isolated() {
        let fixture_dir = unique_temp_dir_path("mcpstore-api-explicit-scope-routes");
        std::fs::create_dir_all(&fixture_dir).unwrap();
        let config_path = fixture_dir.join("mcp.json");
        let store = MCPStore::setup(Some(config_path.to_str().unwrap())).unwrap();
        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");

        let add_definition = client
            .post(format!("{base_url}/services/demo"))
            .json(&json!({
                "command": "printf",
                "args": ["store"],
                "transport": "stdio",
                "_mcpstore": {"scopes": {"store": {}}}
            }))
            .send()
            .await
            .unwrap();
        assert!(add_definition.status().is_success());

        let declare_agent = client
            .put(format!("{base_url}/services/demo/scopes/agents/agent-a"))
            .json(&json!({"config": {"args": ["agent"]}}))
            .send()
            .await
            .unwrap();
        assert!(declare_agent.status().is_success());

        let store_instances = client
            .get(format!("{base_url}/scopes/store/instances"))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        let agent_instances = client
            .get(format!("{base_url}/scopes/agents/agent-a/instances"))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_ne!(
            store_instances["data"]["services"][0]["instance_id"],
            agent_instances["data"]["services"][0]["instance_id"]
        );

        let remove_agent = client
            .delete(format!("{base_url}/services/demo/scopes/agents/agent-a"))
            .send()
            .await
            .unwrap();
        assert!(remove_agent.status().is_success());

        let store_after_remove = client
            .get(format!("{base_url}/scopes/store/instances"))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        let agent_after_remove = client
            .get(format!("{base_url}/scopes/agents/agent-a/instances"))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_eq!(store_after_remove["data"]["total"], 1);
        assert_eq!(agent_after_remove["data"]["total"], 0);

        let legacy_route = client
            .post(format!("{base_url}/for_store/add_service"))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(legacy_route.status(), axum::http::StatusCode::NOT_FOUND);

        let remove_definition = client
            .delete(format!("{base_url}/services/demo"))
            .send()
            .await
            .unwrap();
        assert!(remove_definition.status().is_success());

        handle.abort();
        std::fs::remove_dir_all(fixture_dir).ok();
    }

    #[tokio::test]
    async fn session_snapshot_routes_export_and_import_rust_core_state() {
        let source = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        seed_db_service(&source).await;
        let (source_addr, source_handle) = spawn_test_api(source).await;
        let client = reqwest::Client::new();
        let source_base_url = format!("http://{source_addr}");

        let create = client
            .post(format!("{source_base_url}/sessions/create"))
            .json(&json!({
                "session_id": "snapshot-session",
                "lease_seconds": 60,
                "metadata": {"owner": "snapshot-test"},
            }))
            .send()
            .await
            .unwrap();
        assert!(create.status().is_success());
        let create_payload = create.json::<Value>().await.unwrap();
        let session_key = create_payload["data"]["session"]["session_key"]
            .as_str()
            .unwrap()
            .to_string();
        let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();

        let bind = client
            .post(format!("{source_base_url}/sessions/bind_service"))
            .json(&json!({"session_key": session_key, "instance_id": instance_id}))
            .send()
            .await
            .unwrap();
        assert!(bind.status().is_success());

        let set_state = client
            .post(format!("{source_base_url}/sessions/state/set"))
            .json(&json!({
                "session_key": session_key,
                "key": "cursor",
                "value": {"page": 2},
            }))
            .send()
            .await
            .unwrap();
        assert!(set_state.status().is_success());

        let export = client
            .get(format!("{source_base_url}/sessions/snapshot"))
            .send()
            .await
            .unwrap();
        assert!(export.status().is_success());
        let export_payload = export.json::<Value>().await.unwrap();
        let snapshot = export_payload["data"]["snapshot"].clone();
        assert_eq!(
            snapshot["entities"][session_key.as_str()]["metadata"]["owner"],
            "snapshot-test"
        );
        assert_eq!(
            snapshot["states"]["session_state"][session_key.as_str()]["values"]["cursor"]["page"],
            2
        );

        let target = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        seed_db_service(&target).await;
        let (target_addr, target_handle) = spawn_test_api(target).await;
        let target_base_url = format!("http://{target_addr}");

        let import = client
            .post(format!("{target_base_url}/sessions/snapshot/import"))
            .json(&snapshot)
            .send()
            .await
            .unwrap();
        assert!(import.status().is_success());
        let import_payload = import.json::<Value>().await.unwrap();
        assert_eq!(import_payload["data"]["report"]["sessions_imported"], 1);
        assert_eq!(
            import_payload["data"]["report"]["session_state_records_imported"],
            1
        );

        let imported_state = client
            .get(format!("{target_base_url}/sessions/state/value"))
            .query(&[("session_key", session_key.as_str()), ("key", "cursor")])
            .send()
            .await
            .unwrap();
        assert!(imported_state.status().is_success());
        let imported_state_payload = imported_state.json::<Value>().await.unwrap();
        assert_eq!(imported_state_payload["data"]["value"]["page"], 2);

        let import_again = client
            .post(format!("{target_base_url}/sessions/snapshot/import"))
            .json(&snapshot)
            .send()
            .await
            .unwrap();
        assert!(import_again.status().is_success());
        let import_again_payload = import_again.json::<Value>().await.unwrap();
        assert_eq!(
            import_again_payload["data"]["report"]["sessions_unchanged"],
            1
        );

        source_handle.abort();
        target_handle.abort();
    }

    #[tokio::test]
    async fn store_routes_filter_tools_and_manage_tool_policy() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        seed_db_service(&store).await;
        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");
        let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();

        let default_list = client
            .get(format!("{base_url}/instances/{instance_id}/tools"))
            .send()
            .await
            .unwrap();
        assert!(default_list.status().is_success());
        let default_payload = default_list.json::<Value>().await.unwrap();
        assert_eq!(default_payload["data"]["filter"], "available");
        assert_eq!(default_payload["data"]["total"], 1);

        let set_policy = client
            .put(format!("{base_url}/instances/{instance_id}/tool-policy"))
            .json(&json!({"available_tools": []}))
            .send()
            .await
            .unwrap();
        assert!(set_policy.status().is_success());
        assert_eq!(
            set_policy.json::<Value>().await.unwrap()["data"]["policy"]["tools"],
            json!([])
        );

        let available = client
            .get(format!(
                "{base_url}/instances/{instance_id}/tools?filter=available"
            ))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_eq!(available["data"]["total"], 0);

        let removed = client
            .get(format!(
                "{base_url}/instances/{instance_id}/tools?filter=removed"
            ))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_eq!(removed["data"]["filter"], "removed");
        assert_eq!(removed["data"]["total"], 1);

        let invalid = client
            .get(format!(
                "{base_url}/instances/{instance_id}/tools?filter=hidden"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(invalid.status(), axum::http::StatusCode::BAD_REQUEST);

        let clear = client
            .delete(format!("{base_url}/instances/{instance_id}/tool-policy"))
            .send()
            .await
            .unwrap();
        assert!(clear.status().is_success());

        let restored = client
            .get(format!("{base_url}/instances/{instance_id}/tools"))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_eq!(restored["data"]["total"], 1);

        handle.abort();
    }

    #[tokio::test]
    async fn store_routes_manage_rust_core_tool_transforms() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        seed_db_service(&store).await;
        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");
        let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();

        let set_transform = client
            .put(format!(
                "{base_url}/instances/{instance_id}/tool_transforms/echo"
            ))
            .json(&json!({
                "display_name": "say",
                "description": "Say text with a stable hidden debug flag.",
                "arguments": [
                    {
                        "original_name": "text",
                        "new_name": "message",
                        "hidden": false,
                        "description": "Message to echo."
                    },
                    {
                        "original_name": "debug",
                        "hidden": true,
                        "default_value": false
                    }
                ],
                "tags": ["compat"],
                "enabled": true
            }))
            .send()
            .await
            .unwrap();
        assert!(set_transform.status().is_success());
        let set_payload = set_transform.json::<Value>().await.unwrap();
        assert_eq!(set_payload["data"]["transform"]["display_name"], "say");
        assert_eq!(set_payload["data"]["transform"]["version"], 1);

        let list_tools = client
            .get(format!("{base_url}/instances/{instance_id}/tools"))
            .send()
            .await
            .unwrap();
        assert!(list_tools.status().is_success());
        let list_tools_payload = list_tools.json::<Value>().await.unwrap();
        let tool = &list_tools_payload["data"]["tools"][0];
        assert_eq!(tool["name"], "say");
        assert_eq!(
            tool["input_schema"]["properties"]["message"]["description"],
            "Message to echo."
        );
        assert!(tool["input_schema"]["properties"].get("debug").is_none());
        assert_eq!(tool["input_schema"]["required"], json!(["message"]));

        let get_transform = client
            .get(format!(
                "{base_url}/instances/{instance_id}/tool_transforms/echo"
            ))
            .send()
            .await
            .unwrap();
        assert!(get_transform.status().is_success());
        let get_payload = get_transform.json::<Value>().await.unwrap();
        assert_eq!(get_payload["data"]["transform"]["tool_name"], "echo");

        let list_transforms = client
            .get(format!("{base_url}/tool_transforms"))
            .send()
            .await
            .unwrap();
        assert!(list_transforms.status().is_success());
        let list_payload = list_transforms.json::<Value>().await.unwrap();
        assert_eq!(list_payload["data"]["total"], 1);

        let delete_transform = client
            .delete(format!(
                "{base_url}/instances/{instance_id}/tool_transforms/echo"
            ))
            .send()
            .await
            .unwrap();
        assert!(delete_transform.status().is_success());

        let list_tools_after_delete = client
            .get(format!("{base_url}/instances/{instance_id}/tools"))
            .send()
            .await
            .unwrap();
        assert!(list_tools_after_delete.status().is_success());
        let list_tools_after_delete_payload =
            list_tools_after_delete.json::<Value>().await.unwrap();
        assert_eq!(
            list_tools_after_delete_payload["data"]["tools"][0]["name"],
            "echo"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn store_routes_manage_rust_core_openapi_imports() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");
        let spec = json!({
            "openapi": "3.0.0",
            "info": {"title": "Inventory", "version": "1.0.0"},
            "components": {
                "securitySchemes": {
                    "ApiKeyAuth": {"type": "apiKey", "in": "header", "name": "x-api-key"}
                }
            },
            "security": [{"ApiKeyAuth": []}],
            "paths": {
                "/items": {
                    "get": {"operationId": "listItems", "responses": {"200": {"description": "ok"}}},
                    "post": {
                        "operationId": "createItem",
                        "requestBody": {
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {"name": {"type": "string"}},
                                        "required": ["name"]
                                    }
                                }
                            }
                        },
                        "responses": {"201": {"description": "created"}}
                    }
                }
            }
        });

        let import_response = client
            .post(format!("{base_url}/openapi_imports/inventory/import"))
            .json(&json!({
                "spec_url": "memory://inventory",
                "spec": spec,
                "timeout_millis": 4100,
                "fetch_timeout_millis": 4200,
                "auth": {"ApiKeyAuth": "secret"}
            }))
            .send()
            .await
            .unwrap();
        assert!(import_response.status().is_success());
        let import_payload = import_response.json::<Value>().await.unwrap();
        assert_eq!(
            import_payload["data"]["import"]["service_name"],
            "inventory"
        );
        assert_eq!(import_payload["data"]["import"]["total_endpoints"], 2);
        assert_eq!(
            import_payload["data"]["import"]["component_types"]["tools"],
            1
        );
        assert_eq!(
            import_payload["data"]["import"]["component_types"]["resources"],
            1
        );
        assert_eq!(import_payload["data"]["import"]["runtime_executable"], true);
        assert_eq!(
            import_payload["data"]["import"]["security_schemes"]["ApiKeyAuth"]["name"],
            "x-api-key"
        );

        let service_response = client
            .get(format!(
                "{base_url}/instances/{}",
                ServiceInstanceKey::new("inventory", ScopeRef::Store).instance_id()
            ))
            .send()
            .await
            .unwrap();
        assert!(service_response.status().is_success());
        let service_payload = service_response.json::<Value>().await.unwrap();
        assert_eq!(
            service_payload["data"]["effective_config"]["openapi_timeout_millis"],
            4100
        );
        assert_eq!(
            service_payload["data"]["effective_config"]["openapi_fetch_timeout_millis"],
            4200
        );

        let get_response = client
            .get(format!("{base_url}/openapi_imports/inventory"))
            .send()
            .await
            .unwrap();
        assert!(get_response.status().is_success());
        let get_payload = get_response.json::<Value>().await.unwrap();
        assert_eq!(
            get_payload["data"]["import"]["spec_info"]["title"],
            "Inventory"
        );

        let list_response = client
            .get(format!("{base_url}/openapi_imports"))
            .send()
            .await
            .unwrap();
        assert!(list_response.status().is_success());
        let list_payload = list_response.json::<Value>().await.unwrap();
        assert_eq!(list_payload["data"]["total"], 1);

        let invalid_timeout = client
            .post(format!("{base_url}/openapi_imports/bundle"))
            .json(&json!({
                "spec_url": "memory://invalid-timeout",
                "spec": {"openapi": "3.0.0", "info": {"title": "Invalid", "version": "1.0.0"}, "paths": {}},
                "fetch_timeout_millis": 0
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            invalid_timeout.status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        let invalid_timeout_payload = invalid_timeout.json::<Value>().await.unwrap();
        assert_eq!(
            invalid_timeout_payload["errors"][0]["field"],
            "fetch_timeout_millis"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn store_route_bundles_openapi_without_importing() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        let (addr, handle) = spawn_test_api(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");
        let fixture_dir = unique_temp_dir_path("mcpstore-api");
        std::fs::create_dir_all(&fixture_dir).unwrap();
        let schemas_path = fixture_dir.join("schemas.json");
        std::fs::write(
            &schemas_path,
            serde_json::to_vec(&json!({
                "Item": {
                    "type": "object",
                    "properties": {"id": {"type": "string"}},
                    "required": ["id"]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let bundle_response = client
            .post(format!("{base_url}/openapi_imports/bundle"))
            .json(&json!({
                "spec_url": fixture_dir.join("openapi.json").to_string_lossy(),
                "spec": {
                    "openapi": "3.0.0",
                    "info": {"title": "Inventory", "version": "1.0.0"},
                    "paths": {
                        "/items": {
                            "get": {
                                "operationId": "listItems",
                                "responses": {
                                    "200": {
                                        "description": "ok",
                                        "content": {
                                            "application/json": {
                                                "schema": {"$ref": "./schemas.json#/Item"}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }))
            .send()
            .await
            .unwrap();
        assert!(bundle_response.status().is_success());
        let bundle_payload = bundle_response.json::<Value>().await.unwrap();
        assert_eq!(
            bundle_payload["data"]["bundle"]["paths"]["/items"]["get"]["responses"]["200"]
                ["content"]["application/json"]["schema"]["properties"]["id"]["type"],
            "string"
        );

        let list_response = client
            .get(format!("{base_url}/openapi_imports"))
            .send()
            .await
            .unwrap();
        assert!(list_response.status().is_success());
        let list_payload = list_response.json::<Value>().await.unwrap();
        assert_eq!(list_payload["data"]["total"], 0);

        handle.abort();
        std::fs::remove_dir_all(&fixture_dir).ok();
    }

    #[tokio::test]
    async fn store_route_bundles_openapi_artifact_without_importing() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some(unique_namespace()),
        })
        .unwrap();
        let (addr, handle, state) = spawn_test_api_with_state(store).await;
        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}");
        let fixture_dir = unique_temp_dir_path("mcpstore-api");
        std::fs::create_dir_all(&fixture_dir).unwrap();
        let schemas_path = fixture_dir.join("schemas.json");
        std::fs::write(
            &schemas_path,
            serde_json::to_vec(&json!({
                "Item": {
                    "type": "object",
                    "properties": {"id": {"type": "string"}},
                    "required": ["id"]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let artifact_response = client
            .post(format!("{base_url}/openapi_imports/bundle_artifact"))
            .json(&json!({
                "spec_url": fixture_dir.join("openapi.json").to_string_lossy(),
                "ref_cache": {"ttl_seconds": 17},
                "spec": {
                    "openapi": "3.0.0",
                    "info": {"title": "Inventory", "version": "1.0.0"},
                    "paths": {
                        "/items": {
                            "get": {
                                "operationId": "listItems",
                                "responses": {
                                    "200": {
                                        "description": "ok",
                                        "content": {
                                            "application/json": {
                                                "schema": {"$ref": "./schemas.json#/Item"}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }))
            .send()
            .await
            .unwrap();
        assert!(artifact_response.status().is_success());
        let artifact_payload = artifact_response.json::<Value>().await.unwrap();
        let artifact = &artifact_payload["data"]["artifact"];
        assert_eq!(
            artifact["bundle"]["paths"]["/items"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["properties"]["id"]["type"],
            "string"
        );
        assert_eq!(artifact["documents"].as_array().unwrap().len(), 2);
        assert_eq!(artifact["dependencies"].as_array().unwrap().len(), 1);
        assert_eq!(
            artifact["dependencies"][0]["source_ref"],
            "./schemas.json#/Item"
        );
        assert_eq!(artifact["diagnostics"].as_array().unwrap().len(), 0);
        let states = state
            .store
            .cache()
            .get_all_states_async("openapi_ref_documents")
            .await
            .unwrap();
        assert_eq!(states.len(), 1);
        let cached = states.values().next().unwrap();
        assert_eq!(cached["ttl_seconds"], json!(17));

        let list_response = client
            .get(format!("{base_url}/openapi_imports"))
            .send()
            .await
            .unwrap();
        assert!(list_response.status().is_success());
        let list_payload = list_response.json::<Value>().await.unwrap();
        assert_eq!(list_payload["data"]["total"], 0);

        handle.abort();
        std::fs::remove_dir_all(&fixture_dir).ok();
    }
}
