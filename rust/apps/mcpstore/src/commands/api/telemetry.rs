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
    Ok(success(
        "请求详情获取成功",
        serde_json::to_value(record).unwrap_or(Value::Null),
    ))
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
    Ok(success(
        "用量统计获取成功",
        serde_json::to_value(stats).unwrap_or(Value::Null),
    ))
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

/// GET /gateway/stats/heatmap — per-day MCP-primitive call situation from the
/// frame capture (gateway face), for the usage heatmap.
pub(super) async fn usage_stats_heatmap(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<DailyStatsQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let days = telemetry
        .heatmap_stats(query.days.unwrap_or(371).min(400))
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    Ok(success(
        "热力图统计获取成功",
        serde_json::to_value(days).unwrap_or(Value::Null),
    ))
}

#[derive(Deserialize)]
pub(super) struct FrameActivityQuery {
    hours: Option<u32>,
}

/// GET /gateway/stats/frame-activity — frame-based activity over a time range.
pub(super) async fn frame_activity_stats(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<FrameActivityQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let hours = query.hours.unwrap_or(24).min(24 * 7);
    
    // Choose bucket size based on time range
    let bucket_ms = if hours <= 1 {
        60_000 // 1 minute buckets for 1 hour
    } else if hours <= 24 {
        3_600_000 // 1 hour buckets for 24 hours
    } else if hours <= 72 {
        3_600_000 // 1 hour buckets for 3 days
    } else {
        86_400_000 // 1 day buckets for 7 days
    };
    
    let stats = telemetry
        .frame_activity_stats(hours, bucket_ms)
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    Ok(success(
        "帧活动统计获取成功",
        serde_json::to_value(stats).unwrap_or(Value::Null),
    ))
}

/// GET /gateway/stats/frame-activity-by-service — frame activity grouped by service.
pub(super) async fn frame_activity_by_service(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<FrameActivityQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let hours = query.hours.unwrap_or(24).min(24 * 7);
    
    let bucket_ms = if hours <= 1 {
        60_000
    } else if hours <= 24 {
        3_600_000
    } else if hours <= 72 {
        3_600_000
    } else {
        86_400_000
    };
    
    let stats = telemetry
        .frame_activity_by_service(hours, bucket_ms)
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    Ok(success(
        "按服务分组的帧活动统计获取成功",
        serde_json::to_value(stats).unwrap_or(Value::Null),
    ))
}

#[derive(Deserialize)]
pub(super) struct FrameSessionsQuery {
    face: Option<String>,
    hours: Option<i64>,
    limit: Option<u32>,
    offset: Option<u32>,
}

/// GET /gateway/frames/sessions — sessions with recorded frames, newest first.
/// Supports `face` / `hours` filters and `limit` / `offset` pagination in SQL.
pub(super) async fn frame_sessions(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<FrameSessionsQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let page = telemetry
        .frame_sessions(
            query.face.as_deref(),
            query.hours.unwrap_or(24),
            query.limit.unwrap_or(20),
            query.offset.unwrap_or(0),
        )
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    Ok(success(
        "帧会话获取成功",
        serde_json::to_value(page).unwrap_or(Value::Null),
    ))
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
    Ok(success(
        "帧时间线获取成功",
        serde_json::to_value(frames).unwrap_or(Value::Null),
    ))
}

#[derive(Deserialize)]
pub(super) struct FrameRecentQuery {
    face: Option<String>,
    limit: Option<u32>,
}

/// GET /gateway/frames/recent — recent frames (any session), chronological LTR window.
pub(super) async fn frame_recent(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<FrameRecentQuery>,
) -> ApiResult {
    let telemetry = require_telemetry(&state)?;
    let frames = telemetry
        .frame_recent(query.face.as_deref(), query.limit.unwrap_or(400))
        .map_err(|error| ApiError::invalid_request(format!("telemetry read failed: {error}")))?;
    Ok(success(
        "最近帧获取成功",
        serde_json::to_value(frames).unwrap_or(Value::Null),
    ))
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
    Ok(success(
        "帧详情获取成功",
        serde_json::to_value(record).unwrap_or(Value::Null),
    ))
}

fn require_telemetry(
    state: &ApiState,
) -> Result<std::sync::Arc<crate::telemetry::TelemetryLog>, ApiError> {
    state.telemetry().ok_or_else(|| {
        ApiError::invalid_request("request telemetry is not attached to this daemon")
    })
}
