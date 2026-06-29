use crate::config::AppConfig;

#[derive(Clone, Debug)]
pub(crate) struct StoreRuntimeConfig {
    pub(crate) max_connection_attempts: i32,
    pub(crate) connect_timeout_secs: i64,
    pub(crate) retry_backoff_base_secs: i64,
    pub(crate) retry_backoff_max_secs: i64,
    pub(crate) reconnect_hard_timeout_secs: i64,
    pub(crate) half_open_lease_secs: i64,
    pub(crate) health_warn_latency_ms: f64,
}

impl StoreRuntimeConfig {
    pub(crate) fn from_app_config(config: &AppConfig) -> Self {
        let health = &config.health_check;
        Self {
            max_connection_attempts: health.max_reconnect_attempts.max(1),
            connect_timeout_secs: ceil_seconds(health.startup_timeout, 1),
            retry_backoff_base_secs: ceil_seconds(health.backoff_base, 1),
            retry_backoff_max_secs: ceil_seconds(health.backoff_max, 1),
            reconnect_hard_timeout_secs: ceil_seconds(health.reconnect_hard_timeout, 1),
            half_open_lease_secs: ceil_seconds(health.lease_ttl, 1),
            health_warn_latency_ms: health.latency_p95_warn.max(0.01) * 1000.0,
        }
    }
}

fn ceil_seconds(value: f64, minimum: i64) -> i64 {
    let rounded = value.ceil() as i64;
    rounded.max(minimum)
}
