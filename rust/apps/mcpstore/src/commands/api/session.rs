use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use mcpstore::{CreateSessionRequest, SessionScope};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    envelope::{success, ApiError, ApiResult},
    parse::{extract_tool_args, extract_tool_name, parse_scope_ref},
    resolve_instance, ApiState,
};

#[derive(Deserialize)]
pub(super) struct SessionCreateRequest {
    session_id: String,
    scope: Option<String>,
    agent_id: Option<String>,
    lease_seconds: Option<i64>,
    metadata: Option<Value>,
}

#[derive(Deserialize)]
pub(super) struct SessionKeyQuery {
    session_key: String,
}

#[derive(Deserialize)]
pub(super) struct SessionFindQuery {
    session_id: String,
    scope: Option<String>,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct SessionListQuery {
    scope: Option<String>,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct SessionCloseRequest {
    session_key: Option<String>,
    reason: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct SessionExtendRequest {
    session_key: Option<String>,
    lease_seconds: i64,
}

/// 绑定/解绑服务：以 `service_name + scope` 寻址（不再要求调用方传 instance_id）。
#[derive(Deserialize)]
pub(super) struct SessionBindServiceRequest {
    session_key: Option<String>,
    service_name: String,
    scope: Option<String>,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct SessionStateValueQuery {
    session_key: String,
    key: String,
}

#[derive(Deserialize)]
pub(super) struct SessionStateSetRequest {
    session_key: Option<String>,
    key: String,
    value: Value,
}

#[derive(Deserialize)]
pub(super) struct SessionStateDeleteRequest {
    session_key: Option<String>,
    key: String,
}

#[derive(Deserialize)]
pub(super) struct SessionStateClearRequest {
    session_key: Option<String>,
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

pub(super) async fn session_create(
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

/// `GET /sessions/get?session_key=`
pub(super) async fn session_get(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    let session = state
        .store
        .get_session(&query.session_key)
        .await
        .map_err(ApiError::from_store)?;
    let session = require_present_session(session, &query.session_key, "session")?;
    Ok(success("Session 获取成功", json!({ "session": session })))
}

pub(super) async fn session_find(
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

pub(super) async fn session_list(
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

pub(super) async fn session_export_snapshot(State(state): State<Arc<ApiState>>) -> ApiResult {
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

pub(super) async fn session_import_snapshot(
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

/// `GET /sessions/status?session_key=`
pub(super) async fn session_status(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    let status = state
        .store
        .get_session_status(&query.session_key)
        .await
        .map_err(ApiError::from_store)?;
    let status = require_present_session(status, &query.session_key, "session_status")?;
    Ok(success("Session 状态获取成功", json!({ "status": status })))
}

/// `POST /sessions/close` —— body: `{ session_key, reason? }`
pub(super) async fn session_close(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionCloseRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    let status = state
        .store
        .close_session(&session_key, payload.reason)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Session 已关闭", json!({ "status": status })))
}

/// `POST /sessions/extend` —— body: `{ session_key, lease_seconds }`
pub(super) async fn session_extend(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionExtendRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    let session = state
        .store
        .extend_session(&session_key, payload.lease_seconds)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Session 已续期", json!({ "session": session })))
}

/// `POST /sessions/bind_service` —— body: `{ session_key, service_name, scope?, agent_id? }`
pub(super) async fn session_bind_service(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    let scope = parse_scope_ref(payload.scope.as_deref(), payload.agent_id.as_deref())?;
    let instance_id = resolve_instance(&state, &payload.service_name, &scope).await?;
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

/// `POST /sessions/unbind_service` —— body: `{ session_key, service_name, scope?, agent_id? }`
pub(super) async fn session_unbind_service(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    let scope = parse_scope_ref(payload.scope.as_deref(), payload.agent_id.as_deref())?;
    let instance_id = resolve_instance(&state, &payload.service_name, &scope).await?;
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

/// `GET /sessions/list_services?session_key=`
pub(super) async fn session_list_services(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    let services = state
        .store
        .list_session_services(&query.session_key)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session 服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

/// `GET /sessions/list_tools?session_key=`
pub(super) async fn session_list_tools(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    let tools = state
        .store
        .list_tools_in_session(&query.session_key)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session 工具列表获取成功",
        json!({ "tools": tools, "total": tools.len() }),
    ))
}

/// `POST /sessions/call_tool` —— body: `{ session_key, service_name, scope?, agent_id?, tool_name, args? }`
pub(super) async fn session_call_tool(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let session_key = payload
        .get("session_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    let service_name = payload
        .get("service_name")
        .and_then(Value::as_str)
        .or_else(|| payload.get("service").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::missing_parameter("service_name"))?;
    let scope = parse_scope_ref(
        payload.get("scope").and_then(Value::as_str),
        payload.get("agent_id").and_then(Value::as_str),
    )?;
    let instance_id = resolve_instance(&state, service_name, &scope).await?;
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

/// `GET /sessions/state/list?session_key=`
pub(super) async fn session_list_state(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    let session_state = state
        .store
        .list_session_state(&query.session_key)
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

/// `GET /sessions/state/value?session_key=&key=`
pub(super) async fn session_get_state_value(
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

/// `POST /sessions/state/set` —— body: `{ session_key, key, value }`
pub(super) async fn session_set_state(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionStateSetRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    let session_state = state
        .store
        .set_session_state(&session_key, &payload.key, payload.value)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session state 设置成功",
        json!({ "state": session_state }),
    ))
}

/// `POST /sessions/state/delete` —— body: `{ session_key, key }`
pub(super) async fn session_delete_state(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionStateDeleteRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    let session_state = state
        .store
        .delete_session_state(&session_key, &payload.key)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Session state 删除成功",
        json!({ "state": session_state }),
    ))
}

/// `POST /sessions/state/clear` —— body: `{ session_key }`
pub(super) async fn session_clear_state(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionStateClearRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
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
