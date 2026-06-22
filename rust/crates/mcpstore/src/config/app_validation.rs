use super::{AppConfig, ConfigError, Result};

pub(super) fn validate_app_config(config: &AppConfig) -> Result<()> {
    let mut errors = Vec::new();
    let health = &config.health_check;
    let monitoring = &config.monitoring;
    let standalone = &config.standalone;

    validate_non_empty("server.host", &config.server.host, &mut errors);
    validate_range_f64(
        "health_check.startup_interval",
        health.startup_interval,
        0.1,
        60.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.startup_timeout",
        health.startup_timeout,
        1.0,
        1800.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.startup_hard_timeout",
        health.startup_hard_timeout,
        1.0,
        7200.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.readiness_interval",
        health.readiness_interval,
        1.0,
        300.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.readiness_success_threshold",
        health.readiness_success_threshold,
        1,
        10,
        &mut errors,
    );
    validate_range_i32(
        "health_check.readiness_failure_threshold",
        health.readiness_failure_threshold,
        1,
        10,
        &mut errors,
    );
    validate_range_f64(
        "health_check.liveness_interval",
        health.liveness_interval,
        1.0,
        300.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.liveness_failure_threshold",
        health.liveness_failure_threshold,
        1,
        10,
        &mut errors,
    );
    validate_range_f64(
        "health_check.ping_timeout_http",
        health.ping_timeout_http,
        0.1,
        600.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.ping_timeout_sse",
        health.ping_timeout_sse,
        0.1,
        600.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.ping_timeout_stdio",
        health.ping_timeout_stdio,
        0.1,
        1200.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.warning_ping_timeout",
        health.warning_ping_timeout,
        0.1,
        1200.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.window_size",
        health.window_size,
        1,
        1000,
        &mut errors,
    );
    validate_range_i32(
        "health_check.window_min_calls",
        health.window_min_calls,
        1,
        1000,
        &mut errors,
    );
    validate_range_f64(
        "health_check.error_rate_threshold",
        health.error_rate_threshold,
        0.0,
        1.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.latency_p95_warn",
        health.latency_p95_warn,
        0.01,
        30.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.latency_p99_critical",
        health.latency_p99_critical,
        0.01,
        60.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.max_reconnect_attempts",
        health.max_reconnect_attempts,
        1,
        100,
        &mut errors,
    );
    validate_range_f64(
        "health_check.backoff_base",
        health.backoff_base,
        0.1,
        300.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.backoff_max",
        health.backoff_max,
        1.0,
        3600.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.backoff_jitter",
        health.backoff_jitter,
        0.0,
        1.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.backoff_max_duration",
        health.backoff_max_duration,
        1.0,
        7200.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.half_open_max_calls",
        health.half_open_max_calls,
        1,
        100,
        &mut errors,
    );
    validate_range_f64(
        "health_check.half_open_success_rate_threshold",
        health.half_open_success_rate_threshold,
        0.0,
        1.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.reconnect_hard_timeout",
        health.reconnect_hard_timeout,
        1.0,
        7200.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.lease_ttl",
        health.lease_ttl,
        1.0,
        3600.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.lease_renew_interval",
        health.lease_renew_interval,
        0.5,
        3600.0,
        &mut errors,
    );

    validate_range_i32(
        "monitoring.reconnection_seconds",
        monitoring.reconnection_seconds,
        5,
        1800,
        &mut errors,
    );
    validate_range_f64(
        "monitoring.cleanup_hours",
        monitoring.cleanup_hours,
        0.1,
        168.0,
        &mut errors,
    );
    validate_range_i32(
        "monitoring.local_service_ping_timeout",
        monitoring.local_service_ping_timeout,
        1,
        60,
        &mut errors,
    );
    validate_range_i32(
        "monitoring.remote_service_ping_timeout",
        monitoring.remote_service_ping_timeout,
        1,
        120,
        &mut errors,
    );
    validate_range_f64(
        "monitoring.adaptive_timeout_multiplier",
        monitoring.adaptive_timeout_multiplier,
        1.0,
        5.0,
        &mut errors,
    );
    validate_range_i32(
        "monitoring.response_time_history_size",
        monitoring.response_time_history_size,
        5,
        100,
        &mut errors,
    );

    validate_range_f64(
        "standalone.heartbeat_interval_seconds",
        standalone.heartbeat_interval_seconds,
        1.0,
        300.0,
        &mut errors,
    );
    validate_range_f64(
        "standalone.http_timeout_seconds",
        standalone.http_timeout_seconds,
        1.0,
        300.0,
        &mut errors,
    );
    validate_range_f64(
        "standalone.reconnection_interval_seconds",
        standalone.reconnection_interval_seconds,
        1.0,
        1800.0,
        &mut errors,
    );
    validate_range_f64(
        "standalone.cleanup_interval_seconds",
        standalone.cleanup_interval_seconds,
        10.0,
        3600.0,
        &mut errors,
    );
    validate_non_empty(
        "standalone.streamable_http_endpoint",
        &standalone.streamable_http_endpoint,
        &mut errors,
    );
    validate_allowed(
        "standalone.default_transport",
        &standalone.default_transport,
        &["stdio", "sse", "websocket"],
        &mut errors,
    );
    validate_allowed(
        "standalone.log_level",
        &standalone.log_level,
        &["DEBUG", "INFO", "DEGRADED", "ERROR"],
        &mut errors,
    );
    validate_allowed(
        "standalone.log_format",
        &standalone.log_format,
        &["json", "text"],
        &mut errors,
    );
    validate_allowed(
        "server.log_level",
        &config.server.log_level,
        &["debug", "info", "degraded", "error", "critical"],
        &mut errors,
    );

    if !errors.is_empty() {
        return Err(ConfigError::Invalid(errors.join("; ")));
    }
    Ok(())
}

fn validate_range_f64(name: &str, value: f64, min: f64, max: f64, errors: &mut Vec<String>) {
    if value < min || value > max {
        errors.push(format!("{name}={value} out of range [{min}, {max}]"));
    }
}

fn validate_range_i32(name: &str, value: i32, min: i32, max: i32, errors: &mut Vec<String>) {
    if value < min || value > max {
        errors.push(format!("{name}={value} out of range [{min}, {max}]"));
    }
}

fn validate_non_empty(name: &str, value: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{name} cannot be empty"));
    }
}

fn validate_allowed(name: &str, value: &str, allowed: &[&str], errors: &mut Vec<String>) {
    if !allowed.iter().any(|candidate| candidate == &value) {
        errors.push(format!(
            "{name}='{value}' is invalid, allowed values: {}",
            allowed.join(", ")
        ));
    }
}
