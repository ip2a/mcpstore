use super::*;
use axum::extract::{Path, Query};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub(super) struct HistoryQuery {
    caller: Option<String>,
    service: Option<String>,
    tool: Option<String>,
    success: Option<bool>,
    from_ms: Option<i64>,
    to_ms: Option<i64>,
    before_id: Option<i64>,
    limit: Option<u32>,
}

/// GET /gateway/requests/history — persisted request records, newest first.
pub(super) async fn requests_history(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<HistoryQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let records = telemetry
        .history(crate::telemetry::HistoryFilter {
            caller: query.caller,
            service: query.service,
            tool: query.tool,
            success: query.success,
            from_unix_ms: query.from_ms,
            to_unix_ms: query.to_ms,
            before_id: query.before_id,
            limit: query.limit.unwrap_or(50),
        })
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    let count = records.len();
    Ok(success(
        "请求历史获取成功",
        json!({ "requests": records, "has_more": count == query.limit.unwrap_or(50) as usize }),
    ))
}

/// GET /gateway/requests/detail/:id — one record including full args/result payloads.
pub(super) async fn request_detail(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let record = telemetry
        .get(id)
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?
        .ok_or_else(|| {
            ApiError::not_found(
                mcpstore::error::FailureCode::Internal,
                format!("request record {id} not found"),
                None,
                None,
            )
        })?;
    Ok(success("请求详情获取成功", serde_json::to_value(record).unwrap_or(Value::Null)))
}

#[derive(Deserialize)]
pub(super) struct StatsQuery {
    hours: Option<u32>,
}

/// GET /gateway/stats — tool-call usage over the trailing window, hourly buckets.
pub(super) async fn usage_stats(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<StatsQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let stats = telemetry
        .usage_stats(query.hours.unwrap_or(24))
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    Ok(success("用量统计获取成功", serde_json::to_value(stats).unwrap_or(Value::Null)))
}

fn require_telemetry(
    state: &ApiState,
) -> Result<std::sync::Arc<crate::telemetry::TelemetryLog>, ApiError> {
    state.telemetry().ok_or_else(|| {
        ApiError::invalid_request("request telemetry is not attached to this daemon")
    })
}
