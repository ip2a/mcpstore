use super::{field_validation::*, StandaloneConfig};

pub(super) fn validate_standalone_config(standalone: &StandaloneConfig, errors: &mut Vec<String>) {
    validate_range_f64(
        "standalone.heartbeat_interval_seconds",
        standalone.heartbeat_interval_seconds,
        1.0,
        300.0,
        errors,
    );
    validate_range_f64(
        "standalone.http_timeout_seconds",
        standalone.http_timeout_seconds,
        1.0,
        300.0,
        errors,
    );
    validate_range_f64(
        "standalone.reconnection_interval_seconds",
        standalone.reconnection_interval_seconds,
        1.0,
        1800.0,
        errors,
    );
    validate_range_f64(
        "standalone.cleanup_interval_seconds",
        standalone.cleanup_interval_seconds,
        10.0,
        3600.0,
        errors,
    );
    validate_non_empty(
        "standalone.streamable_http_endpoint",
        &standalone.streamable_http_endpoint,
        errors,
    );
    validate_allowed(
        "standalone.default_transport",
        &standalone.default_transport,
        &["stdio", "streamable-http"],
        errors,
    );
    validate_allowed(
        "standalone.log_level",
        &standalone.log_level,
        &["DEBUG", "INFO", "DEGRADED", "ERROR"],
        errors,
    );
    validate_allowed(
        "standalone.log_format",
        &standalone.log_format,
        &["json", "text"],
        errors,
    );
}
