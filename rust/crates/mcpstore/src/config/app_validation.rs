use super::{
    health_validation::validate_health_check_config,
    monitoring_validation::validate_monitoring_config, server_validation::validate_server_settings,
    standalone_validation::validate_standalone_config, AppConfig, ConfigError, Result,
};

pub(super) fn validate_app_config(config: &AppConfig) -> Result<()> {
    let mut errors = Vec::new();

    validate_server_settings(&config.server, &mut errors);
    validate_health_check_config(&config.health_check, &mut errors);
    validate_monitoring_config(&config.monitoring, &mut errors);
    validate_standalone_config(&config.standalone, &mut errors);

    if !errors.is_empty() {
        return Err(ConfigError::Invalid(errors.join("; ")));
    }
    Ok(())
}
