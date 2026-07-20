use std::sync::Arc;

use axum::{extract::State, Json};
use mcpstore::MCPStore;
use serde::Deserialize;

use super::{
    envelope::{success, ApiError, ApiResult},
    parse::parse_cache_storage,
    ApiState,
};

#[derive(Deserialize)]
pub(super) struct CacheSwitchRequest {
    backend: String,
    redis_url: Option<String>,
    namespace: Option<String>,
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

pub(super) async fn inspect(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .cache_inspect()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("缓存视图获取成功", report))
}

pub(super) async fn health(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .cache_health_check()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("缓存健康检查成功", report))
}

pub(super) async fn switch(
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
