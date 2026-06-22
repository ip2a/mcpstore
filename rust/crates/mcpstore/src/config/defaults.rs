use super::{DEFAULT_SERVER_LOG_LEVEL, DEFAULT_SERVER_URL_PREFIX};

pub(super) fn default_version() -> String {
    "1.0.0".to_string()
}

pub(super) fn default_app_description() -> String {
    "MCPStore global config file".to_string()
}

pub(super) fn default_created_by() -> String {
    "MCPStore CLI".to_string()
}

pub(super) fn default_created_at() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub(super) fn default_ui_language() -> String {
    "zh-cn".to_string()
}

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_server_host() -> String {
    "0.0.0.0".to_string()
}

pub(super) fn default_server_port() -> u16 {
    18200
}

pub(super) fn default_server_log_level_value() -> String {
    DEFAULT_SERVER_LOG_LEVEL.to_string()
}

pub(super) fn default_server_url_prefix_value() -> String {
    DEFAULT_SERVER_URL_PREFIX.to_string()
}

pub(super) fn default_startup_interval() -> f64 {
    1.0
}

pub(super) fn default_startup_timeout() -> f64 {
    30.0
}

pub(super) fn default_startup_hard_timeout() -> f64 {
    120.0
}

pub(super) fn default_readiness_interval() -> f64 {
    5.0
}

pub(super) fn default_readiness_success_threshold() -> i32 {
    1
}

pub(super) fn default_readiness_failure_threshold() -> i32 {
    1
}

pub(super) fn default_liveness_interval() -> f64 {
    10.0
}

pub(super) fn default_liveness_failure_threshold() -> i32 {
    2
}

pub(super) fn default_ping_timeout_http() -> f64 {
    20.0
}

pub(super) fn default_ping_timeout_sse() -> f64 {
    20.0
}

pub(super) fn default_ping_timeout_stdio() -> f64 {
    40.0
}

pub(super) fn default_warning_ping_timeout() -> f64 {
    30.0
}

pub(super) fn default_window_size() -> i32 {
    20
}

pub(super) fn default_window_min_calls() -> i32 {
    5
}

pub(super) fn default_error_rate_threshold() -> f64 {
    0.3
}

pub(super) fn default_latency_p95_warn() -> f64 {
    2.0
}

pub(super) fn default_latency_p99_critical() -> f64 {
    5.0
}

pub(super) fn default_max_reconnect_attempts() -> i32 {
    10
}

pub(super) fn default_backoff_base() -> f64 {
    1.0
}

pub(super) fn default_backoff_max() -> f64 {
    60.0
}

pub(super) fn default_backoff_jitter() -> f64 {
    0.1
}

pub(super) fn default_backoff_max_duration() -> f64 {
    600.0
}

pub(super) fn default_half_open_max_calls() -> i32 {
    3
}

pub(super) fn default_half_open_success_rate_threshold() -> f64 {
    0.6
}

pub(super) fn default_reconnect_hard_timeout() -> f64 {
    900.0
}

pub(super) fn default_lease_ttl() -> f64 {
    60.0
}

pub(super) fn default_lease_renew_interval() -> f64 {
    20.0
}

pub(super) fn default_monitoring_reconnection_seconds() -> i32 {
    60
}

pub(super) fn default_monitoring_cleanup_hours() -> f64 {
    24.0
}

pub(super) fn default_local_service_ping_timeout() -> i32 {
    3
}

pub(super) fn default_remote_service_ping_timeout() -> i32 {
    5
}

pub(super) fn default_adaptive_timeout_multiplier() -> f64 {
    2.0
}

pub(super) fn default_response_time_history_size() -> i32 {
    10
}

pub(super) fn default_standalone_heartbeat_interval_seconds() -> f64 {
    30.0
}

pub(super) fn default_standalone_http_timeout_seconds() -> f64 {
    10.0
}

pub(super) fn default_standalone_reconnection_interval_seconds() -> f64 {
    60.0
}

pub(super) fn default_standalone_cleanup_interval_seconds() -> f64 {
    300.0
}

pub(super) fn default_streamable_http_endpoint() -> String {
    "/mcp".to_string()
}

pub(super) fn default_standalone_transport() -> String {
    "stdio".to_string()
}

pub(super) fn default_standalone_log_level() -> String {
    "INFO".to_string()
}

pub(super) fn default_standalone_log_format() -> String {
    "json".to_string()
}

pub(super) fn default_namespace() -> String {
    "mcpstore".to_string()
}
