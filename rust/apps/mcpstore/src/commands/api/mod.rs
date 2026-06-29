use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use clap::Args;
use mcpstore::{
    perspective::GLOBAL_AGENT_STORE, CreateSessionRequest, MCPStore, OpenApiImportOptions,
    SessionScope, ToolTransformPatch,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    store_args::{build_store, StoreSourceArgs},
    BoxErr,
};

mod envelope;
mod parse;

use envelope::{success, ApiError, ApiResult};
use parse::{
    cache_storage_label, ensure_json_object, extract_prompt_args, extract_prompt_name,
    extract_resource_uri, extract_tool_args, extract_tool_name, normalize_prefix,
    parse_cache_storage, parse_named_service_payload, parse_positive_u64, parse_positive_usize,
};

#[derive(Args)]
pub struct ApiArgs {
    #[arg(long, default_value_t = 18200, help = "API 服务端口")]
    pub port: u16,
    #[arg(long, default_value = "127.0.0.1", help = "绑定地址")]
    pub host: String,
    #[arg(long, default_value = "", help = "URL 前缀，例如 /mcp")]
    pub url_prefix: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Clone)]
struct ApiState {
    store: Arc<MCPStore>,
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
struct SessionFindQuery {
    session_id: String,
    scope: Option<String>,
    agent_id: Option<String>,
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
    service_name: String,
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
struct ToolTransformTargetQuery {
    service_name: String,
    tool_name: String,
}

#[derive(Deserialize)]
struct ToolTransformSetRequest {
    service_name: Option<String>,
    tool_name: Option<String>,
    #[serde(flatten)]
    transform: ToolTransformPatch,
}

#[derive(Deserialize)]
struct ToolTransformDeleteRequest {
    service_name: Option<String>,
    tool_name: Option<String>,
}

#[derive(Deserialize)]
struct OpenApiImportTargetQuery {
    name: String,
}

#[derive(Deserialize)]
struct OpenApiImportRequest {
    name: Option<String>,
    spec_url: String,
    spec: Option<Value>,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    auth: serde_json::Map<String, Value>,
}

pub async fn run(args: ApiArgs) -> Result<(), BoxErr> {
    let store = build_store(&args.store)?;
    store.load_from_source().await?;

    let prefix = normalize_prefix(&args.url_prefix);
    let state = Arc::new(ApiState {
        store: Arc::new(store),
    });
    if !state.store.is_db_source() {
        spawn_control_request_worker(state.store.clone());
    }
    let app = router(state, &prefix);

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

fn spawn_control_request_worker(store: Arc<MCPStore>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            match store.process_control_requests().await {
                Ok(processed) if processed > 0 => {
                    tracing::info!("[API] Processed {processed} queued control request(s)");
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!("[API] Control request processing failed: {error}");
                }
            }
        }
    });
}

fn router(state: Arc<ApiState>, prefix: &str) -> Router {
    let base = Router::new()
        .route("/health", get(health))
        .route("/agents/list", get(list_agents))
        .route("/events/history", get(event_history))
        .route("/events/capability_report", get(event_capability_report))
        .route("/sessions/create", post(session_create))
        .route("/sessions/get/:session_key", get(session_get))
        .route("/sessions/get", get(session_get_by_query))
        .route("/sessions/find", get(session_find))
        .route("/sessions/list", get(session_list))
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
        .route("/for_store/list_services", get(store_list_services))
        .route("/for_store/add_service", post(store_add_service))
        .route(
            "/for_store/update_service/:service_name",
            post(store_update_service),
        )
        .route(
            "/for_store/remove_service/:service_name",
            post(store_remove_service),
        )
        .route(
            "/for_store/connect_service/:service_name",
            post(store_connect_service),
        )
        .route(
            "/for_store/disconnect_service/:service_name",
            post(store_disconnect_service),
        )
        .route(
            "/for_store/restart_service/:service_name",
            post(store_restart_service),
        )
        .route(
            "/for_store/wait_service/:service_name",
            get(store_wait_service),
        )
        .route("/for_store/list_tools", get(store_list_tools))
        .route("/for_store/call_tool", post(store_call_tool))
        .route(
            "/for_store/tool_transforms",
            get(store_list_tool_transforms),
        )
        .route(
            "/for_store/tool_transforms/get",
            get(store_get_tool_transform),
        )
        .route(
            "/for_store/tool_transforms/get/:service_name/:tool_name",
            get(store_get_tool_transform_by_path),
        )
        .route(
            "/for_store/tool_transforms/set",
            post(store_set_tool_transform),
        )
        .route(
            "/for_store/tool_transforms/set/:service_name/:tool_name",
            post(store_set_tool_transform_by_path),
        )
        .route(
            "/for_store/tool_transforms/delete",
            post(store_delete_tool_transform),
        )
        .route(
            "/for_store/tool_transforms/delete/:service_name/:tool_name",
            post(store_delete_tool_transform_by_path),
        )
        .route(
            "/for_store/openapi_imports",
            get(store_list_openapi_imports),
        )
        .route(
            "/for_store/openapi_imports/get",
            get(store_get_openapi_import),
        )
        .route(
            "/for_store/openapi_imports/get/:name",
            get(store_get_openapi_import_by_path),
        )
        .route(
            "/for_store/openapi_imports/import",
            post(store_import_openapi),
        )
        .route(
            "/for_store/openapi_imports/import/:name",
            post(store_import_openapi_by_path),
        )
        .route("/for_store/list_resources", get(store_list_resources))
        .route(
            "/for_store/list_resource_templates",
            get(store_list_resource_templates),
        )
        .route("/for_store/read_resource", get(store_read_resource))
        .route("/for_store/list_prompts", get(store_list_prompts))
        .route("/for_store/get_prompt", post(store_get_prompt))
        .route("/for_store/check_services", get(store_check_services))
        .route(
            "/for_store/service_info/:service_name",
            get(store_service_info),
        )
        .route(
            "/for_store/service_status/:service_name",
            get(store_service_status),
        )
        .route("/for_store/show_config", get(store_show_config))
        .route("/for_store/reset_config", post(store_reset_config))
        .route(
            "/for_agent/:agent_id/list_services",
            get(agent_list_services),
        )
        .route("/for_agent/:agent_id/add_service", post(agent_add_service))
        .route(
            "/for_agent/:agent_id/update_service/:service_name",
            post(agent_update_service),
        )
        .route(
            "/for_agent/:agent_id/remove_service/:service_name",
            post(agent_remove_service),
        )
        .route(
            "/for_agent/:agent_id/connect_service/:service_name",
            post(agent_connect_service),
        )
        .route(
            "/for_agent/:agent_id/disconnect_service/:service_name",
            post(agent_disconnect_service),
        )
        .route(
            "/for_agent/:agent_id/restart_service/:service_name",
            post(agent_restart_service),
        )
        .route(
            "/for_agent/:agent_id/wait_service/:service_name",
            get(agent_wait_service),
        )
        .route(
            "/for_agent/:agent_id/assign_service/:service_name",
            post(agent_assign_service),
        )
        .route(
            "/for_agent/:agent_id/unassign_service/:service_name",
            post(agent_unassign_service),
        )
        .route("/for_agent/:agent_id/list_tools", get(agent_list_tools))
        .route("/for_agent/:agent_id/call_tool", post(agent_call_tool))
        .route(
            "/for_agent/:agent_id/list_resources",
            get(agent_list_resources),
        )
        .route(
            "/for_agent/:agent_id/list_resource_templates",
            get(agent_list_resource_templates),
        )
        .route(
            "/for_agent/:agent_id/read_resource",
            get(agent_read_resource),
        )
        .route("/for_agent/:agent_id/list_prompts", get(agent_list_prompts))
        .route("/for_agent/:agent_id/get_prompt", post(agent_get_prompt))
        .route(
            "/for_agent/:agent_id/service_info/:service_name",
            get(agent_service_info),
        )
        .route(
            "/for_agent/:agent_id/service_status/:service_name",
            get(agent_service_status),
        )
        .route("/for_agent/:agent_id/show_config", get(agent_show_config))
        .route("/cache/health", get(cache_health))
        .route("/cache/inspect", get(cache_inspect))
        .route("/cache/switch", post(cache_switch))
        .with_state(state);

    if prefix.is_empty() {
        base
    } else {
        Router::new().nest(prefix, base)
    }
}

async fn health(State(state): State<Arc<ApiState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "backend": cache_storage_label(state.store.current_cache_storage().await),
    }))
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
    session_bind_service_impl(state, session_key, payload.service_name).await
}

async fn session_bind_service_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_bind_service_impl(state, session_key, payload.service_name).await
}

async fn session_bind_service_impl(
    state: Arc<ApiState>,
    session_key: String,
    service_name: String,
) -> ApiResult {
    let relation = state
        .store
        .bind_service_to_session(&session_key, &service_name)
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
    session_unbind_service_impl(state, session_key, payload.service_name).await
}

async fn session_unbind_service_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_unbind_service_impl(state, session_key, payload.service_name).await
}

async fn session_unbind_service_impl(
    state: Arc<ApiState>,
    session_key: String,
    service_name: String,
) -> ApiResult {
    let relation = state
        .store
        .unbind_service_from_session(&session_key, &service_name)
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
    let tool_name = extract_tool_name(&payload)?;
    let args = extract_tool_args(&payload)?;
    let result = state
        .store
        .call_tool_in_session(&session_key, &tool_name, args)
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

async fn event_capability_report(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state.store.event_capability_report().await;
    Ok(success("事件能力报告获取成功", report))
}

async fn store_list_services(State(state): State<Arc<ApiState>>) -> ApiResult {
    let services = state
        .store
        .list_services_scoped(None)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

async fn store_add_service(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Value>,
) -> ApiResult {
    for (name, config) in parse_named_service_payload(payload)? {
        state
            .store
            .add_service(&name, config)
            .await
            .map_err(ApiError::from_store)?;
    }
    Ok(success("服务添加成功", json!({ "status": "ok" })))
}

async fn store_update_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let updates = ensure_json_object(payload, "payload")?;
    state
        .store
        .patch_service(&service_name, updates)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务更新成功", json!({ "status": "ok" })))
}

async fn store_remove_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .remove_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务删除成功", json!({ "status": "ok" })))
}

async fn store_connect_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .connect_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务连接成功", json!({ "status": "ok" })))
}

async fn store_disconnect_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .disconnect_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务断开成功", json!({ "status": "ok" })))
}

async fn store_restart_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .restart_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务重启成功", json!({ "status": "ok" })))
}

async fn store_wait_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
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
        .wait_service_ready(&service_name, timeout)
        .await
        .map_err(ApiError::from_store)?;
    let status = serde_json::to_value(status)
        .map_err(|error| ApiError::invalid_request(format!("服务状态序列化失败: {error}")))?;
    Ok(success("服务等待完成", status))
}

async fn store_list_tools(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let tools = state
        .store
        .list_tools_scoped(None, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具列表获取成功",
        json!({ "tools": tools, "total": tools.len() }),
    ))
}

async fn store_call_tool(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let tool_name = extract_tool_name(&payload)?;
    let args = extract_tool_args(&payload)?;
    let resolution = state
        .store
        .resolve_tool_for_agent(GLOBAL_AGENT_STORE, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    let result = state
        .store
        .call_tool(
            &resolution.global_service_name,
            &resolution.canonical_tool_name,
            args,
        )
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

async fn store_get_tool_transform(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ToolTransformTargetQuery>,
) -> ApiResult {
    store_get_tool_transform_impl(state, query.service_name, query.tool_name).await
}

async fn store_get_tool_transform_by_path(
    State(state): State<Arc<ApiState>>,
    Path((service_name, tool_name)): Path<(String, String)>,
) -> ApiResult {
    store_get_tool_transform_impl(state, service_name, tool_name).await
}

async fn store_get_tool_transform_impl(
    state: Arc<ApiState>,
    service_name: String,
    tool_name: String,
) -> ApiResult {
    let transform = state
        .store
        .get_tool_transform(&service_name, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具转换规则获取成功",
        json!({ "transform": transform }),
    ))
}

async fn store_set_tool_transform(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<ToolTransformSetRequest>,
) -> ApiResult {
    let service_name = payload
        .service_name
        .ok_or_else(|| ApiError::missing_parameter("service_name"))?;
    let tool_name = payload
        .tool_name
        .ok_or_else(|| ApiError::missing_parameter("tool_name"))?;
    store_set_tool_transform_impl(state, service_name, tool_name, payload.transform).await
}

async fn store_set_tool_transform_by_path(
    State(state): State<Arc<ApiState>>,
    Path((service_name, tool_name)): Path<(String, String)>,
    Json(payload): Json<ToolTransformSetRequest>,
) -> ApiResult {
    store_set_tool_transform_impl(state, service_name, tool_name, payload.transform).await
}

async fn store_set_tool_transform_impl(
    state: Arc<ApiState>,
    service_name: String,
    tool_name: String,
    transform: ToolTransformPatch,
) -> ApiResult {
    let transform = state
        .store
        .set_tool_transform(&service_name, &tool_name, transform)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具转换规则设置成功",
        json!({ "transform": transform }),
    ))
}

async fn store_delete_tool_transform(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<ToolTransformDeleteRequest>,
) -> ApiResult {
    let service_name = payload
        .service_name
        .ok_or_else(|| ApiError::missing_parameter("service_name"))?;
    let tool_name = payload
        .tool_name
        .ok_or_else(|| ApiError::missing_parameter("tool_name"))?;
    store_delete_tool_transform_impl(state, service_name, tool_name).await
}

async fn store_delete_tool_transform_by_path(
    State(state): State<Arc<ApiState>>,
    Path((service_name, tool_name)): Path<(String, String)>,
) -> ApiResult {
    store_delete_tool_transform_impl(state, service_name, tool_name).await
}

async fn store_delete_tool_transform_impl(
    state: Arc<ApiState>,
    service_name: String,
    tool_name: String,
) -> ApiResult {
    state
        .store
        .delete_tool_transform(&service_name, &tool_name)
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

async fn store_get_openapi_import(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<OpenApiImportTargetQuery>,
) -> ApiResult {
    store_get_openapi_import_impl(state, query.name).await
}

async fn store_get_openapi_import_by_path(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
) -> ApiResult {
    store_get_openapi_import_impl(state, name).await
}

async fn store_get_openapi_import_impl(state: Arc<ApiState>, name: String) -> ApiResult {
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

async fn store_import_openapi(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<OpenApiImportRequest>,
) -> ApiResult {
    let name = payload
        .name
        .ok_or_else(|| ApiError::missing_parameter("name"))?;
    let options = OpenApiImportOptions {
        headers: payload.headers,
        auth: payload.auth,
    };
    store_import_openapi_impl(state, name, payload.spec_url, payload.spec, options).await
}

async fn store_import_openapi_by_path(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
    Json(payload): Json<OpenApiImportRequest>,
) -> ApiResult {
    let options = OpenApiImportOptions {
        headers: payload.headers,
        auth: payload.auth,
    };
    store_import_openapi_impl(state, name, payload.spec_url, payload.spec, options).await
}

async fn store_import_openapi_impl(
    state: Arc<ApiState>,
    name: String,
    spec_url: String,
    spec: Option<Value>,
    options: OpenApiImportOptions,
) -> ApiResult {
    let import = match spec {
        Some(spec) => {
            state
                .store
                .import_openapi_service_from_spec_with_options(&name, &spec_url, spec, options)
                .await
        }
        None => {
            state
                .store
                .import_openapi_service_with_options(&name, &spec_url, options)
                .await
        }
    }
    .map_err(ApiError::from_store)?;
    Ok(success("OpenAPI 导入成功", json!({ "import": import })))
}

async fn store_list_resources(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let resources = state
        .store
        .list_resources_scoped(None, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "资源列表获取成功",
        json!({ "resources": resources, "total": resources.len() }),
    ))
}

async fn store_list_resource_templates(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let templates = state
        .store
        .list_resource_templates_scoped(None, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "资源模板列表获取成功",
        json!({ "resource_templates": templates, "total": templates.len() }),
    ))
}

async fn store_read_resource(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let uri = extract_resource_uri(&params)?;
    let result = state
        .store
        .read_resource_scoped(None, &uri, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("资源读取成功", result))
}

async fn store_list_prompts(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let prompts = state
        .store
        .list_prompts_scoped(None, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Prompt 列表获取成功",
        json!({ "prompts": prompts, "total": prompts.len() }),
    ))
}

async fn store_get_prompt(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let prompt_name = extract_prompt_name(&payload)?;
    let args = extract_prompt_args(&payload)?;
    let service_name = payload.get("service_name").and_then(Value::as_str);
    let result = state
        .store
        .get_prompt_scoped(None, &prompt_name, args, service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Prompt 获取成功", result))
}

async fn store_check_services(State(state): State<Arc<ApiState>>) -> ApiResult {
    let result = state
        .store
        .check_services_scoped(None)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务检查完成", result))
}

async fn store_service_info(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    let service = state
        .store
        .service_info_scoped(None, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务信息获取成功", service))
}

async fn store_service_status(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    let status = state
        .store
        .service_status_scoped(None, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务状态获取成功", status))
}

async fn store_show_config(State(state): State<Arc<ApiState>>) -> ApiResult {
    let config = state
        .store
        .show_config()
        .await
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
        .list_services_scoped(Some(&agent_id))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

async fn agent_add_service(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    for (name, config) in parse_named_service_payload(payload)? {
        state
            .store
            .add_service_for_agent(&agent_id, &name, config)
            .await
            .map_err(ApiError::from_store)?;
    }
    Ok(success("Agent 服务添加成功", json!({ "status": "ok" })))
}

async fn agent_update_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    let updates = ensure_json_object(payload, "payload")?;
    state
        .store
        .patch_service(&global_service_name, updates)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务更新成功", json!({ "status": "ok" })))
}

async fn agent_remove_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    state
        .store
        .remove_service(&global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务删除成功", json!({ "status": "ok" })))
}

async fn agent_connect_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    state
        .store
        .connect_service(&global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务连接成功", json!({ "status": "ok" })))
}

async fn agent_disconnect_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    state
        .store
        .disconnect_service(&global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务断开成功", json!({ "status": "ok" })))
}

async fn agent_restart_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    state
        .store
        .restart_service(&global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务重启成功", json!({ "status": "ok" })))
}

async fn agent_wait_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    let timeout = params
        .get("timeout")
        .map(String::as_str)
        .map(parse_positive_u64)
        .transpose()?
        .unwrap_or(10);
    let status = state
        .store
        .wait_service_ready(&global_service_name, timeout)
        .await
        .map_err(ApiError::from_store)?;
    let status = serde_json::to_value(status)
        .map_err(|error| ApiError::invalid_request(format!("服务状态序列化失败: {error}")))?;
    Ok(success("Agent 服务等待完成", status))
}

async fn agent_assign_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    state
        .store
        .assign_service_to_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务授权成功", json!({ "status": "ok" })))
}

async fn agent_unassign_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .unwrap_or(service_name);
    state
        .store
        .unassign_service_from_agent(&agent_id, &global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务解除授权成功", json!({ "status": "ok" })))
}

async fn agent_list_tools(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let tools = state
        .store
        .list_tools_scoped(
            Some(&agent_id),
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 工具列表获取成功",
        json!({ "tools": tools, "total": tools.len() }),
    ))
}

async fn agent_call_tool(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let tool_name = extract_tool_name(&payload)?;
    let args = extract_tool_args(&payload)?;
    let resolution = state
        .store
        .resolve_tool_for_agent(&agent_id, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    let result = state
        .store
        .call_tool(
            &resolution.global_service_name,
            &resolution.canonical_tool_name,
            args,
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 工具调用完成",
        serde_json::to_value(result).unwrap_or(Value::Null),
    ))
}

async fn agent_list_resources(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let resources = state
        .store
        .list_resources_scoped(
            Some(&agent_id),
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 资源列表获取成功",
        json!({ "resources": resources, "total": resources.len() }),
    ))
}

async fn agent_list_resource_templates(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let templates = state
        .store
        .list_resource_templates_scoped(
            Some(&agent_id),
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 资源模板列表获取成功",
        json!({ "resource_templates": templates, "total": templates.len() }),
    ))
}

async fn agent_read_resource(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let uri = extract_resource_uri(&params)?;
    let result = state
        .store
        .read_resource_scoped(
            Some(&agent_id),
            &uri,
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 资源读取成功", result))
}

async fn agent_list_prompts(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let prompts = state
        .store
        .list_prompts_scoped(
            Some(&agent_id),
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent Prompt 列表获取成功",
        json!({ "prompts": prompts, "total": prompts.len() }),
    ))
}

async fn agent_get_prompt(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let prompt_name = extract_prompt_name(&payload)?;
    let args = extract_prompt_args(&payload)?;
    let service_name = payload.get("service_name").and_then(Value::as_str);
    let result = state
        .store
        .get_prompt_scoped(Some(&agent_id), &prompt_name, args, service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent Prompt 获取成功", result))
}

async fn agent_service_info(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let service = state
        .store
        .service_info_scoped(Some(&agent_id), &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务信息获取成功", service))
}

async fn agent_service_status(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let status = state
        .store
        .service_status_scoped(Some(&agent_id), &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务状态获取成功", status))
}

async fn agent_show_config(
    State(state): State<Arc<ApiState>>,
    Path(_agent_id): Path<String>,
) -> ApiResult {
    let config = state
        .store
        .show_config()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 配置获取成功", config))
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
    let snapshot = serde_json::to_value(snapshot)
        .map_err(|error| ApiError::invalid_request(format!("缓存切换结果序列化失败: {error}")))?;
    Ok(success("缓存后端切换成功", snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpstore::{
        cache::models::{
            AgentServiceRelation, ServiceEntity, ServiceRelationItem, ServiceToolRelation,
            ToolEntity, ToolRelationItem,
        },
        CacheStorage, ServerConfig, SourceMode, StoreOptions,
    };
    use std::{collections::HashMap, net::SocketAddr, time::SystemTime};

    fn unique_namespace() -> String {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        format!("api-session-test-{nanos}")
    }

    fn stdio_config() -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("echo".to_string()),
            args: vec!["fixture".to_string()],
            env: HashMap::new(),
            headers: HashMap::new(),
            transport: Some("stdio".to_string()),
            working_dir: None,
            description: Some("fixture".to_string()),
        }
    }

    async fn seed_db_service(store: &MCPStore) {
        let config = stdio_config();
        let cache = store.cache();
        cache
            .put_entity(
                "services",
                "demo",
                serde_json::to_value(ServiceEntity {
                    service_global_name: "demo".to_string(),
                    service_original_name: "demo".to_string(),
                    source_agent: "global_agent_store".to_string(),
                    config: serde_json::to_value(config).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        cache
            .put_relation(
                "agent_services",
                "global_agent_store",
                serde_json::to_value(AgentServiceRelation {
                    services: vec![ServiceRelationItem {
                        service_original_name: "demo".to_string(),
                        service_global_name: "demo".to_string(),
                        client_id: "demo".to_string(),
                        established_time: 111,
                        last_access: None,
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();
        cache
            .put_entity(
                "tools",
                "demo_echo",
                serde_json::to_value(ToolEntity {
                    tool_global_name: "demo_echo".to_string(),
                    tool_original_name: "echo".to_string(),
                    service_global_name: "demo".to_string(),
                    service_original_name: "demo".to_string(),
                    source_agent: "global_agent_store".to_string(),
                    description: "echo tool".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Original text."},
                            "debug": {"type": "boolean"}
                        },
                        "required": ["text", "debug"]
                    }),
                    created_time: 111,
                    tool_hash: "fixture".to_string(),
                })
                .unwrap(),
            )
            .await
            .unwrap();
        cache
            .put_relation(
                "service_tools",
                "demo",
                serde_json::to_value(ServiceToolRelation {
                    service_global_name: "demo".to_string(),
                    service_original_name: "demo".to_string(),
                    source_agent: "global_agent_store".to_string(),
                    tools: vec![ToolRelationItem {
                        tool_global_name: "demo_echo".to_string(),
                        tool_original_name: "echo".to_string(),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();
    }

    async fn spawn_test_api(store: MCPStore) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(ApiState {
            store: Arc::new(store),
        });
        let app = router(state, "");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (addr, handle)
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
        assert_eq!(session_key, "store:global:api-core-session");

        let bind = client
            .post(format!("{base_url}/sessions/bind_service"))
            .json(&json!({"session_key": session_key, "service_name": "demo"}))
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
        assert_eq!(tools_payload["data"]["tools"][0]["name"], "demo_echo");

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

        let set_transform = client
            .post(format!(
                "{base_url}/for_store/tool_transforms/set/demo/echo"
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
            .get(format!("{base_url}/for_store/list_tools"))
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
            .get(format!("{base_url}/for_store/tool_transforms/get/demo/say"))
            .send()
            .await
            .unwrap();
        assert!(get_transform.status().is_success());
        let get_payload = get_transform.json::<Value>().await.unwrap();
        assert_eq!(
            get_payload["data"]["transform"]["original_tool_name"],
            "echo"
        );

        let list_transforms = client
            .get(format!("{base_url}/for_store/tool_transforms"))
            .send()
            .await
            .unwrap();
        assert!(list_transforms.status().is_success());
        let list_payload = list_transforms.json::<Value>().await.unwrap();
        assert_eq!(list_payload["data"]["total"], 1);

        let delete_transform = client
            .post(format!("{base_url}/for_store/tool_transforms/delete"))
            .json(&json!({"service_name": "demo", "tool_name": "say"}))
            .send()
            .await
            .unwrap();
        assert!(delete_transform.status().is_success());

        let list_tools_after_delete = client
            .get(format!("{base_url}/for_store/list_tools"))
            .send()
            .await
            .unwrap();
        assert!(list_tools_after_delete.status().is_success());
        let list_tools_after_delete_payload =
            list_tools_after_delete.json::<Value>().await.unwrap();
        assert_eq!(
            list_tools_after_delete_payload["data"]["tools"][0]["name"],
            "demo_echo"
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
            .post(format!(
                "{base_url}/for_store/openapi_imports/import/inventory"
            ))
            .json(&json!({
                "spec_url": "memory://inventory",
                "spec": spec,
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

        let get_response = client
            .get(format!(
                "{base_url}/for_store/openapi_imports/get/inventory"
            ))
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
            .get(format!("{base_url}/for_store/openapi_imports"))
            .send()
            .await
            .unwrap();
        assert!(list_response.status().is_success());
        let list_payload = list_response.json::<Value>().await.unwrap();
        assert_eq!(list_payload["data"]["total"], 1);

        handle.abort();
    }
}
