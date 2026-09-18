use crate::config::{DEFAULT_API_URL_PREFIX, DEFAULT_RUNTIME_LOG_LEVEL};

pub(in crate::config) fn default_ui_language() -> String {
    "en".to_string()
}

pub(in crate::config) fn default_log_max_size_bytes() -> u64 {
    5 * 1024 * 1024
}

pub(in crate::config) fn default_true() -> bool {
    true
}

pub(in crate::config) fn default_api_host() -> String {
    "127.0.0.1".to_string()
}

pub(in crate::config) fn default_api_port() -> u16 {
    1820
}

pub(in crate::config) fn default_web_host() -> String {
    "127.0.0.1".to_string()
}

pub(in crate::config) fn default_web_port() -> u16 {
    1828
}

pub(in crate::config) fn default_mcp_aggregate_transport() -> String {
    "stdio".to_string()
}

pub(in crate::config) fn default_mcp_aggregate_port() -> u16 {
    1830
}

pub(in crate::config) fn default_runtime_log_level() -> String {
    DEFAULT_RUNTIME_LOG_LEVEL.to_string()
}

pub(in crate::config) fn default_api_url_prefix() -> String {
    DEFAULT_API_URL_PREFIX.to_string()
}

pub(in crate::config) fn default_hosts_active() -> String {
    "local".to_string()
}

pub(in crate::config) fn default_host_url() -> String {
    "/api".to_string()
}
