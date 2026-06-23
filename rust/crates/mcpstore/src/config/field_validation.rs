pub(super) fn validate_range_f64(
    name: &str,
    value: f64,
    min: f64,
    max: f64,
    errors: &mut Vec<String>,
) {
    if value < min || value > max {
        errors.push(format!("{name}={value} out of range [{min}, {max}]"));
    }
}

pub(super) fn validate_range_i32(
    name: &str,
    value: i32,
    min: i32,
    max: i32,
    errors: &mut Vec<String>,
) {
    if value < min || value > max {
        errors.push(format!("{name}={value} out of range [{min}, {max}]"));
    }
}

pub(super) fn validate_non_empty(name: &str, value: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{name} cannot be empty"));
    }
}

pub(super) fn validate_allowed(
    name: &str,
    value: &str,
    allowed: &[&str],
    errors: &mut Vec<String>,
) {
    if !allowed.iter().any(|candidate| candidate == &value) {
        errors.push(format!(
            "{name}='{value}' is invalid, allowed values: {}",
            allowed.join(", ")
        ));
    }
}
