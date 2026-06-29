pub(in crate::config) fn default_monitoring_reconnection_seconds() -> i32 {
    60
}

pub(in crate::config) fn default_monitoring_cleanup_hours() -> f64 {
    24.0
}

pub(in crate::config) fn default_local_service_ping_timeout() -> i32 {
    3
}

pub(in crate::config) fn default_remote_service_ping_timeout() -> i32 {
    5
}

pub(in crate::config) fn default_adaptive_timeout_multiplier() -> f64 {
    2.0
}

pub(in crate::config) fn default_response_time_history_size() -> i32 {
    10
}
