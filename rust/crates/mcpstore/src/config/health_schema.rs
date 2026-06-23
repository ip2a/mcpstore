use serde::{Deserialize, Serialize};

use super::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_startup_interval")]
    pub startup_interval: f64,
    #[serde(default = "default_startup_timeout")]
    pub startup_timeout: f64,
    #[serde(default = "default_startup_hard_timeout")]
    pub startup_hard_timeout: f64,
    #[serde(default = "default_readiness_interval")]
    pub readiness_interval: f64,
    #[serde(default = "default_readiness_success_threshold")]
    pub readiness_success_threshold: i32,
    #[serde(default = "default_readiness_failure_threshold")]
    pub readiness_failure_threshold: i32,
    #[serde(default = "default_liveness_interval")]
    pub liveness_interval: f64,
    #[serde(default = "default_liveness_failure_threshold")]
    pub liveness_failure_threshold: i32,
    #[serde(default = "default_ping_timeout_http")]
    pub ping_timeout_http: f64,
    #[serde(default = "default_ping_timeout_sse")]
    pub ping_timeout_sse: f64,
    #[serde(default = "default_ping_timeout_stdio")]
    pub ping_timeout_stdio: f64,
    #[serde(default = "default_warning_ping_timeout")]
    pub warning_ping_timeout: f64,
    #[serde(default = "default_window_size")]
    pub window_size: i32,
    #[serde(default = "default_window_min_calls")]
    pub window_min_calls: i32,
    #[serde(default = "default_error_rate_threshold")]
    pub error_rate_threshold: f64,
    #[serde(default = "default_latency_p95_warn")]
    pub latency_p95_warn: f64,
    #[serde(default = "default_latency_p99_critical")]
    pub latency_p99_critical: f64,
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: i32,
    #[serde(default = "default_backoff_base")]
    pub backoff_base: f64,
    #[serde(default = "default_backoff_max")]
    pub backoff_max: f64,
    #[serde(default = "default_backoff_jitter")]
    pub backoff_jitter: f64,
    #[serde(default = "default_backoff_max_duration")]
    pub backoff_max_duration: f64,
    #[serde(default = "default_half_open_max_calls")]
    pub half_open_max_calls: i32,
    #[serde(default = "default_half_open_success_rate_threshold")]
    pub half_open_success_rate_threshold: f64,
    #[serde(default = "default_reconnect_hard_timeout")]
    pub reconnect_hard_timeout: f64,
    #[serde(default = "default_lease_ttl")]
    pub lease_ttl: f64,
    #[serde(default = "default_lease_renew_interval")]
    pub lease_renew_interval: f64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            startup_interval: default_startup_interval(),
            startup_timeout: default_startup_timeout(),
            startup_hard_timeout: default_startup_hard_timeout(),
            readiness_interval: default_readiness_interval(),
            readiness_success_threshold: default_readiness_success_threshold(),
            readiness_failure_threshold: default_readiness_failure_threshold(),
            liveness_interval: default_liveness_interval(),
            liveness_failure_threshold: default_liveness_failure_threshold(),
            ping_timeout_http: default_ping_timeout_http(),
            ping_timeout_sse: default_ping_timeout_sse(),
            ping_timeout_stdio: default_ping_timeout_stdio(),
            warning_ping_timeout: default_warning_ping_timeout(),
            window_size: default_window_size(),
            window_min_calls: default_window_min_calls(),
            error_rate_threshold: default_error_rate_threshold(),
            latency_p95_warn: default_latency_p95_warn(),
            latency_p99_critical: default_latency_p99_critical(),
            max_reconnect_attempts: default_max_reconnect_attempts(),
            backoff_base: default_backoff_base(),
            backoff_max: default_backoff_max(),
            backoff_jitter: default_backoff_jitter(),
            backoff_max_duration: default_backoff_max_duration(),
            half_open_max_calls: default_half_open_max_calls(),
            half_open_success_rate_threshold: default_half_open_success_rate_threshold(),
            reconnect_hard_timeout: default_reconnect_hard_timeout(),
            lease_ttl: default_lease_ttl(),
            lease_renew_interval: default_lease_renew_interval(),
        }
    }
}
