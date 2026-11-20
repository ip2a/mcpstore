pub(in crate::config) fn default_startup_interval() -> f64 {
    1.0
}

pub(in crate::config) fn default_startup_timeout() -> f64 {
    30.0
}

pub(in crate::config) fn default_startup_hard_timeout() -> f64 {
    120.0
}

pub(in crate::config) fn default_liveness_interval() -> f64 {
    10.0
}

pub(in crate::config) fn default_liveness_failure_threshold() -> i32 {
    2
}

pub(in crate::config) fn default_ping_timeout_http() -> f64 {
    20.0
}

pub(in crate::config) fn default_ping_timeout_stdio() -> f64 {
    40.0
}

pub(in crate::config) fn default_window_size() -> i32 {
    20
}

pub(in crate::config) fn default_window_min_calls() -> i32 {
    5
}

pub(in crate::config) fn default_error_rate_threshold() -> f64 {
    0.3
}

pub(in crate::config) fn default_latency_p95_warn() -> f64 {
    2.0
}

pub(in crate::config) fn default_latency_p99_critical() -> f64 {
    5.0
}

pub(in crate::config) fn default_backoff_base() -> f64 {
    1.0
}

pub(in crate::config) fn default_backoff_max() -> f64 {
    60.0
}

pub(in crate::config) fn default_backoff_jitter() -> f64 {
    0.1
}

pub(in crate::config) fn default_half_open_max_calls() -> i32 {
    3
}

pub(in crate::config) fn default_half_open_success_rate_threshold() -> f64 {
    0.6
}

pub(in crate::config) fn default_reconnect_hard_timeout() -> f64 {
    900.0
}
