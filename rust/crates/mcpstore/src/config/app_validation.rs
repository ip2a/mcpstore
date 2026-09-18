use super::{
    api_web_validation::{validate_api_settings, validate_web_settings},
    field_validation::validate_allowed,
    health_validation::validate_health_check_config,
    AppConfig, ConfigError, Result,
};

pub(super) fn validate_app_config(config: &AppConfig) -> Result<()> {
    let mut errors = Vec::new();

    validate_ui_config(config, &mut errors);
    validate_api_settings(&config.api, &mut errors);
    validate_web_settings(&config.web, &mut errors);
    validate_mcp_aggregate_config(config, &mut errors);
    validate_health_check_config(&config.health_check, &mut errors);
    validate_diagnostics_config(config, &mut errors);
    validate_hosts_config(config, &mut errors);

    if !errors.is_empty() {
        return Err(ConfigError::Invalid(errors.join("; ")));
    }
    Ok(())
}

fn validate_mcp_aggregate_config(config: &AppConfig, errors: &mut Vec<String>) {
    if !matches!(
        config.mcp_aggregate.transport.as_str(),
        "stdio" | "streamable-http"
    ) {
        errors.push("mcp_aggregate.transport must be one of stdio, streamable-http".to_string());
    }
    if config.mcp_aggregate.port == 0 {
        errors.push("mcp_aggregate.port must be greater than 0".to_string());
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

fn validate_hosts_config(config: &AppConfig, errors: &mut Vec<String>) {
    let hosts = &config.hosts;
    if hosts.entries.is_empty() {
        errors.push("hosts must contain at least one host".to_string());
        return;
    }

    for (name, entry) in &hosts.entries {
        if name.trim().is_empty() {
            errors.push("hosts entry name cannot be empty".to_string());
        }
        if entry.url.trim().is_empty() {
            errors.push(format!("hosts.\"{name}\".url cannot be empty"));
        }
    }

    if hosts.active.trim().is_empty() {
        errors.push("hosts.active cannot be empty".to_string());
    } else if !hosts.entries.contains_key(&hosts.active) {
        errors.push(format!(
            "hosts.active \"{}\" does not match any configured host",
            hosts.active
        ));
    }
}

fn validate_ui_config(config: &AppConfig, errors: &mut Vec<String>) {
    if !matches!(config.ui.language.as_str(), "auto" | "zh" | "zh-cn" | "en") {
        errors.push("ui.language must be one of auto, zh, zh-cn, en".to_string());
    }
}
