pub(in crate::config) fn default_standalone_heartbeat_interval_seconds() -> f64 {
    30.0
}

pub(in crate::config) fn default_standalone_http_timeout_seconds() -> f64 {
    10.0
}

pub(in crate::config) fn default_standalone_reconnection_interval_seconds() -> f64 {
    60.0
}

pub(in crate::config) fn default_standalone_cleanup_interval_seconds() -> f64 {
    300.0
}

pub(in crate::config) fn default_streamable_http_endpoint() -> String {
    "/mcp".to_string()
}

pub(in crate::config) fn default_standalone_transport() -> String {
    "stdio".to_string()
}

pub(in crate::config) fn default_standalone_log_level() -> String {
    "INFO".to_string()
}

pub(in crate::config) fn default_standalone_log_format() -> String {
    "json".to_string()
}
