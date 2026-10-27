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

#[derive(Deserialize)]
pub(super) struct DailyStatsQuery {
    days: Option<u32>,
}

/// GET /gateway/stats/daily — per-day tool-call totals (UTC day buckets).
pub(super) async fn usage_stats_daily(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<DailyStatsQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let days = telemetry
        .daily_stats(query.days.unwrap_or(35).min(400))
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    Ok(success(
        "每日用量统计获取成功",
        serde_json::to_value(days).unwrap_or(Value::Null),
    ))
}

#[derive(Deserialize)]
pub(super) struct FrameSessionsQuery {
    face: Option<String>,
    hours: Option<i64>,
}

/// GET /gateway/frames/sessions — sessions with recorded frames, newest first.
pub(super) async fn frame_sessions(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<FrameSessionsQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let sessions = telemetry
        .frame_sessions(query.face.as_deref(), query.hours.unwrap_or(24))
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    Ok(success("帧会话获取成功", serde_json::to_value(sessions).unwrap_or(Value::Null)))
}

#[derive(Deserialize)]
pub(super) struct FrameHistoryQuery {
    session_id: String,
    limit: Option<u32>,
}

/// GET /gateway/frames/history — one session's frame timeline, oldest first.
pub(super) async fn frame_history(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<FrameHistoryQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let frames = telemetry
        .frame_history(&query.session_id, query.limit.unwrap_or(200))
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    Ok(success("帧时间线获取成功", serde_json::to_value(frames).unwrap_or(Value::Null)))
}

/// GET /gateway/frames/detail/:id — one frame including the full envelope.
pub(super) async fn frame_detail(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let record = telemetry
        .frame_detail(id)
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?
        .ok_or_else(|| {
            ApiError::not_found(
                mcpstore::error::FailureCode::Internal,
                format!("frame record {id} not found"),
                None,
                None,
            )
        })?;
    Ok(success("帧详情获取成功", serde_json::to_value(record).unwrap_or(Value::Null)))
}

fn require_telemetry(
    state: &ApiState,
) -> Result<std::sync::Arc<crate::telemetry::TelemetryLog>, ApiError> {
    state.telemetry().ok_or_else(|| {
        ApiError::invalid_request("request telemetry is not attached to this daemon")
    })
}
