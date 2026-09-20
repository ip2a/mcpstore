use super::{field_validation::*, ApiSettings, WebSettings};

pub(super) fn validate_api_settings(api: &ApiSettings, errors: &mut Vec<String>) {
    validate_non_empty("api.host", &api.host, errors);
    if api.port == 0 {
        errors.push("api.port must be greater than 0".to_string());
    }
}

pub(super) fn validate_web_settings(web: &WebSettings, errors: &mut Vec<String>) {
    validate_non_empty("web.host", &web.host, errors);
    if web.port == 0 {
        errors.push("web.port must be greater than 0".to_string());
    }
}
