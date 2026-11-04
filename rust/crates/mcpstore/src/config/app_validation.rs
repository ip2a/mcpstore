use super::{
    api_web_validation::{validate_api_settings, validate_web_settings},
    field_validation::validate_allowed,
    health_validation::validate_health_check_config,
    server_validation::validate_server_settings,
    AppConfig, ConfigError, Result,
};

pub(super) fn validate_app_config(config: &AppConfig) -> Result<()> {
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
    if config.diagnostics.runtime_log.max_size_bytes == 0 {
        errors.push("diagnostics.runtime_log.max_size_bytes must be greater than 0".to_string());
    }
    validate_allowed(
        "diagnostics.runtime_log.level",
        &config.diagnostics.runtime_log.level,
        &["trace", "debug", "info", "warn", "error"],
        errors,
    );
}
