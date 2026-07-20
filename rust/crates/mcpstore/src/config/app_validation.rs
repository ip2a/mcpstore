use super::{
    health_validation::validate_health_check_config,
    monitoring_validation::validate_monitoring_config, server_validation::validate_server_settings,
    standalone_validation::validate_standalone_config, AppConfig, ConfigError, Result,
};

pub(super) fn validate_app_config(config: &AppConfig) -> Result<()> {
    let mut errors = Vec::new();

    validate_ui_config(config, &mut errors);
    validate_server_settings(&config.server, &mut errors);
    validate_health_check_config(&config.health_check, &mut errors);
    validate_monitoring_config(&config.monitoring, &mut errors);
    validate_standalone_config(&config.standalone, &mut errors);
    validate_diagnostics_config(config, &mut errors);

    if !errors.is_empty() {
        return Err(ConfigError::Invalid(errors.join("; ")));
    }
    Ok(())
}

fn validate_diagnostics_config(config: &AppConfig, errors: &mut Vec<String>) {
    if config.diagnostics.runtime_log.max_size_bytes == 0 {
        errors.push("diagnostics.runtime_log.max_size_bytes must be greater than 0".to_string());
    }
    if config.diagnostics.history.max_records == 0 {
        errors.push("diagnostics.history.max_records must be greater than 0".to_string());
    }
    if config.diagnostics.history.max_size_bytes == 0 {
        errors.push("diagnostics.history.max_size_bytes must be greater than 0".to_string());
    }
}

fn validate_ui_config(config: &AppConfig, errors: &mut Vec<String>) {
    if !matches!(config.ui.language.as_str(), "auto" | "zh" | "zh-cn" | "en") {
        errors.push("ui.language must be one of auto, zh, zh-cn, en".to_string());
    }

    if config.ui.default_backup_dir.trim().is_empty() {
        errors.push("ui.default_backup_dir cannot be empty".to_string());
    }

    if config.ui.logging.max_size_bytes == 0 {
        errors.push("ui.logging.max_size_bytes must be greater than 0".to_string());
    }
}
