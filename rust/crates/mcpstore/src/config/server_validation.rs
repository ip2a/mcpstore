use super::{field_validation::*, ServerSettings};

pub(super) fn validate_server_settings(server: &ServerSettings, errors: &mut Vec<String>) {
    validate_non_empty("server.host", &server.host, errors);
    validate_allowed(
        "server.log_level",
        &server.log_level,
        &["debug", "info", "degraded", "error", "critical"],
        errors,
    );
}
