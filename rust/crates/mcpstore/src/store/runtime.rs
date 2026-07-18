use crate::config::{AppConfig, ServiceLifecycleDefaults};
use crate::health::state_machine::SupervisorPolicy;

#[derive(Clone, Debug)]
pub(crate) struct StoreRuntimeConfig {
    pub(crate) max_connection_attempts: i32,
    pub(crate) connect_timeout_secs: i64,
    pub(crate) retry_backoff_base_secs: i64,
    pub(crate) retry_backoff_max_secs: i64,
    pub(crate) reconnect_hard_timeout_secs: i64,
    pub(crate) backoff_jitter_ratio: f64,
    pub(crate) liveness_interval_secs: f64,
    pub(crate) ping_timeout_http_secs: f64,
    pub(crate) ping_timeout_stdio_secs: f64,
    pub(crate) supervisor_policy: SupervisorPolicy,
    pub(crate) service_lifecycle_defaults: ServiceLifecycleDefaults,
}

impl StoreRuntimeConfig {
    pub(crate) fn from_app_config(config: &AppConfig) -> Self {
        let health = &config.health_check;
        let supervisor_policy = SupervisorPolicy {
            startup_interval_secs: health.startup_interval.max(0.1),
            startup_timeout_secs: health.startup_timeout.max(0.1),
            startup_hard_timeout_secs: health.startup_hard_timeout.max(1.0),
            window_secs: health.window_size.max(1) as f64,
            window_max_samples: health.window_size.max(1) as usize,
            window_min_calls: health.window_min_calls.max(1) as usize,
            error_rate_threshold: health.error_rate_threshold.clamp(0.0, 1.0),
            latency_p95_warn_ms: health.latency_p95_warn.max(0.01) * 1000.0,
            latency_p99_critical_ms: health.latency_p99_critical.max(0.01) * 1000.0,
            liveness_failure_threshold: health.liveness_failure_threshold.max(1) as usize,
            half_open_max_calls: health.half_open_max_calls.max(1) as usize,
            half_open_success_rate_threshold: health
                .half_open_success_rate_threshold
                .clamp(0.0, 1.0),
        };
        Self {
            max_connection_attempts: health.max_reconnect_attempts.max(1),
            connect_timeout_secs: ceil_seconds(health.startup_timeout, 1),
            retry_backoff_base_secs: ceil_seconds(health.backoff_base, 1),
            retry_backoff_max_secs: ceil_seconds(health.backoff_max, 1),
            reconnect_hard_timeout_secs: ceil_seconds(health.reconnect_hard_timeout, 1),
            backoff_jitter_ratio: health.backoff_jitter.max(0.0),
            liveness_interval_secs: health.liveness_interval.max(0.1),
            ping_timeout_http_secs: health.ping_timeout_http.max(0.1),
            ping_timeout_stdio_secs: health.ping_timeout_stdio.max(0.1),
            supervisor_policy,
            service_lifecycle_defaults: config.service_defaults.lifecycle.clone(),
        }
    }
}

fn ceil_seconds(value: f64, minimum: i64) -> i64 {
    let rounded = value.ceil() as i64;
    rounded.max(minimum)
}
