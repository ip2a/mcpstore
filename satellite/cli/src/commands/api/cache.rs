use std::sync::Arc;

use axum::{extract::State, Json};
use mcpstore::JsonStoreConfig;
use serde::Deserialize;

use super::{
    envelope::{success, ApiError, ApiResult},
    ApiState,
};

#[derive(Deserialize)]
pub(super) struct CacheSwitchRequest {
    store: String,
    config: serde_json::Value,
}

pub(super) async fn inspect(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .cache_inspect()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Cache view fetched successfully", report))
}

pub(super) async fn health(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .cache_health_check()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Cache health check succeeded", report))
}

pub(super) async fn switch(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<CacheSwitchRequest>,
) -> ApiResult {
    let config = JsonStoreConfig::new(&payload.store, payload.config);
    let snapshot = state
        .store
        .swap_store(&config)
        .await
        .map_err(ApiError::from_store)?;
    let snapshot = serde_json::to_value(snapshot).map_err(|error| {
        ApiError::invalid_request(format!("Failed to serialize cache-switch result: {error}"))
    })?;
    Ok(success("Cache backend switched successfully", snapshot))
}
