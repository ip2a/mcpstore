use super::{
    api_web_validation::{validate_api_settings, validate_web_settings},
    field_validation::validate_allowed,
    health_validation::validate_health_check_config,
    server_validation::validate_server_settings,
    AppConfig, ConfigError, Result,
};

pub fn validate_app_config(config: &AppConfig) -> Result<()> {
    let mut errors = Vec::new();

    validate_api_settings(&config.api, &mut errors);
    validate_web_settings(&config.web, &mut errors);
    validate_server_settings(&config.server, &mut errors);
    validate_gateway_config(config, &mut errors);
    validate_health_check_config(&config.health_check, &mut errors);
    validate_diagnostics_config(config, &mut errors);

    if !errors.is_empty() {
        return Err(ConfigError::Invalid(errors.join("; ")));
    }
    Ok(())
}

fn validate_gateway_config(config: &AppConfig, errors: &mut Vec<String>) {
    if config.gateway.port == 0 {
        errors.push("gateway.port must be greater than 0".to_string());
    }
}

fn validate_diagnostics_config(config: &AppConfig, errors: &mut Vec<String>) {
    let diagnostics = &config.diagnostics;
    if diagnostics.runtime_log.max_size_bytes == 0 {
        errors.push("diagnostics.runtime_log.max_size_bytes must be greater than 0".to_string());
    }
    validate_allowed(
        "diagnostics.runtime_log.level",
        &diagnostics.runtime_log.level,
        &["trace", "debug", "info", "warn", "error"],
        errors,
    );
    for (name, value) in [
        (
            "diagnostics.runtime_log.retention_days",
            diagnostics.runtime_log.retention_days,
        ),
        (
            "diagnostics.requests.retention_days",
            diagnostics.requests.retention_days,
        ),
        (
            "diagnostics.requests.max_records",
            diagnostics.requests.max_records,
        ),
        (
            "diagnostics.gateway_calls.retention_days",
            diagnostics.gateway_calls.retention_days,
        ),
        (
            "diagnostics.gateway_calls.max_calls",
            diagnostics.gateway_calls.max_calls,
        ),
        (
            "diagnostics.frames.retention_days",
            diagnostics.frames.retention_days,
        ),
        (
            "diagnostics.frames.max_records",
            diagnostics.frames.max_records,
        ),
    ] {
        if value == Some(0) {
            errors.push(format!("{name} must be greater than 0"));
        }
    }
    for (name, value) in [
        (
            "diagnostics.requests.max_payload_bytes",
            diagnostics.requests.max_payload_bytes,
        ),
        (
            "diagnostics.frames.max_payload_bytes",
            diagnostics.frames.max_payload_bytes,
        ),
        (
            "diagnostics.gateway_calls.memory_capacity",
            diagnostics.gateway_calls.memory_capacity,
        ),
        (
            "diagnostics.event_history.capacity",
            diagnostics.event_history.capacity,
        ),
    ] {
        if value == 0 {
            errors.push(format!("{name} must be greater than 0"));
        }
    }
}
