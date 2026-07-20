use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use mcpstore::{CreateSessionRequest, InstanceId, SessionScope};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    envelope::{success, ApiError, ApiResult},
    parse::{extract_tool_args, extract_tool_name},
    ApiState,
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

#[derive(Deserialize)]
pub(super) struct SessionBindServiceRequest {
    session_key: Option<String>,
    instance_id: InstanceId,
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

pub(super) async fn session_get(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_get_impl(state, session_key).await
}

pub(super) async fn session_get_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_get_impl(state, query.session_key).await
}

pub(super) async fn session_get_impl(state: Arc<ApiState>, session_key: String) -> ApiResult {
    let session = state
        .store
        .get_session(&session_key)
        .await
        .map_err(ApiError::from_store)?;
    let session = require_present_session(session, &session_key, "session")?;
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

pub(super) async fn session_status(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_status_impl(state, session_key).await
}

pub(super) async fn session_status_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_status_impl(state, query.session_key).await
}

pub(super) async fn session_status_impl(state: Arc<ApiState>, session_key: String) -> ApiResult {
    let status = state
        .store
        .get_session_status(&session_key)
        .await
        .map_err(ApiError::from_store)?;
    let status = require_present_session(status, &session_key, "session_status")?;
    Ok(success("Session 状态获取成功", json!({ "status": status })))
}

pub(super) async fn session_close(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionCloseRequest>,
) -> ApiResult {
    session_close_impl(state, session_key, payload.reason).await
}

pub(super) async fn session_close_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionCloseRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_close_impl(state, session_key, payload.reason).await
}

pub(super) async fn session_close_impl(
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

pub(super) async fn session_extend(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionExtendRequest>,
) -> ApiResult {
    session_extend_impl(state, session_key, payload.lease_seconds).await
}

pub(super) async fn session_extend_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionExtendRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_extend_impl(state, session_key, payload.lease_seconds).await
}

pub(super) async fn session_extend_impl(
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

pub(super) async fn session_bind_service(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    session_bind_service_impl(state, session_key, payload.instance_id).await
}

pub(super) async fn session_bind_service_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_bind_service_impl(state, session_key, payload.instance_id).await
}

pub(super) async fn session_bind_service_impl(
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

pub(super) async fn session_unbind_service(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    session_unbind_service_impl(state, session_key, payload.instance_id).await
}

pub(super) async fn session_unbind_service_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionBindServiceRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_unbind_service_impl(state, session_key, payload.instance_id).await
}

pub(super) async fn session_unbind_service_impl(
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

pub(super) async fn session_list_services(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_list_services_impl(state, session_key).await
}

pub(super) async fn session_list_services_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_list_services_impl(state, query.session_key).await
}

pub(super) async fn session_list_services_impl(
    state: Arc<ApiState>,
    session_key: String,
) -> ApiResult {
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

pub(super) async fn session_list_tools(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_list_tools_impl(state, session_key).await
}

pub(super) async fn session_list_tools_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_list_tools_impl(state, query.session_key).await
}

pub(super) async fn session_list_tools_impl(
    state: Arc<ApiState>,
    session_key: String,
) -> ApiResult {
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

pub(super) async fn session_call_tool(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    session_call_tool_impl(state, session_key, payload).await
}

pub(super) async fn session_call_tool_by_body(
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

pub(super) async fn session_call_tool_impl(
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

pub(super) async fn session_list_state(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_list_state_impl(state, session_key).await
}

pub(super) async fn session_list_state_by_query(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SessionKeyQuery>,
) -> ApiResult {
    session_list_state_impl(state, query.session_key).await
}

pub(super) async fn session_list_state_impl(
    state: Arc<ApiState>,
    session_key: String,
) -> ApiResult {
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

pub(super) async fn session_set_state(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionStateSetRequest>,
) -> ApiResult {
    session_set_state_impl(state, session_key, payload.key, payload.value).await
}

pub(super) async fn session_set_state_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionStateSetRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_set_state_impl(state, session_key, payload.key, payload.value).await
}

pub(super) async fn session_set_state_impl(
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

pub(super) async fn session_delete_state(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
    Json(payload): Json<SessionStateDeleteRequest>,
) -> ApiResult {
    session_delete_state_impl(state, session_key, payload.key).await
}

pub(super) async fn session_delete_state_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionStateDeleteRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_delete_state_impl(state, session_key, payload.key).await
}

pub(super) async fn session_delete_state_impl(
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

pub(super) async fn session_clear_state(
    State(state): State<Arc<ApiState>>,
    Path(session_key): Path<String>,
) -> ApiResult {
    session_clear_state_impl(state, session_key).await
}

pub(super) async fn session_clear_state_by_body(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SessionStateClearRequest>,
) -> ApiResult {
    let session_key = payload
        .session_key
        .ok_or_else(|| ApiError::missing_parameter("session_key"))?;
    session_clear_state_impl(state, session_key).await
}

pub(super) async fn session_clear_state_impl(
    state: Arc<ApiState>,
    session_key: String,
) -> ApiResult {
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
