use std::sync::Arc;

use axum::{extract::State, Json};
use mcpstore::JsonStoreConfig;
use serde::Deserialize;

use super::{
    envelope::{success, ApiError, ApiResult},
    ApiState,
};

#[derive(Deserialize)]
pub(super) struct KvSwitchRequest {
    store: String,
    config: serde_json::Value,
}

pub(super) async fn inspect(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .kv_inspect()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("KV view fetched successfully", report))
}

pub(super) async fn health(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .kv_health_check()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("KV health check succeeded", report))
}

pub(super) async fn switch(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<KvSwitchRequest>,
) -> ApiResult {
    let config = JsonStoreConfig::new(&payload.store, payload.config);
    let snapshot = state
        .store
        .swap_store(&config)
        .await
        .map_err(ApiError::from_store)?;
    let snapshot = serde_json::to_value(snapshot).map_err(|error| {
        ApiError::invalid_request(format!("Failed to serialize kv-switch result: {error}"))
    })?;
    Ok(success("KV store switched successfully", snapshot))
}
