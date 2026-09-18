use crate::config::{DEFAULT_SERVER_LOG_LEVEL, DEFAULT_SERVER_URL_PREFIX};

pub(in crate::config) fn default_ui_language() -> String {
    "en".to_string()
}

pub(in crate::config) fn default_log_max_size_bytes() -> u64 {
    5 * 1024 * 1024
}

pub(in crate::config) fn default_true() -> bool {
    true
}

pub(in crate::config) fn default_server_host() -> String {
    "0.0.0.0".to_string()
}

pub(in crate::config) fn default_server_port() -> u16 {
    1820
}

pub(in crate::config) fn default_mcp_aggregate_transport() -> String {
    "stdio".to_string()
}

pub(in crate::config) fn default_mcp_aggregate_port() -> u16 {
    1830
}

pub(in crate::config) fn default_web_port() -> u16 {
    1828
}

pub(in crate::config) fn default_server_log_level_value() -> String {
    DEFAULT_SERVER_LOG_LEVEL.to_string()
}

pub(in crate::config) fn default_server_url_prefix_value() -> String {
    DEFAULT_SERVER_URL_PREFIX.to_string()
}

pub(in crate::config) fn default_hosts_active() -> String {
    "local".to_string()
}

pub(in crate::config) fn default_host_url() -> String {
    "/api".to_string()
}
