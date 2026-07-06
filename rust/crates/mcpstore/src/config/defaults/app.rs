use crate::config::{DEFAULT_SERVER_LOG_LEVEL, DEFAULT_SERVER_URL_PREFIX};

pub(in crate::config) fn default_version() -> String {
    "1.0.0".to_string()
}

pub(in crate::config) fn default_app_description() -> String {
    "MCPStore global config file".to_string()
}

pub(in crate::config) fn default_created_by() -> String {
    "MCPStore CLI".to_string()
}

pub(in crate::config) fn default_created_at() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub(in crate::config) fn default_ui_language() -> String {
    "zh-cn".to_string()
}

pub(in crate::config) fn default_backup_dir() -> String {
    "./backups".to_string()
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
    18200
}

pub(in crate::config) fn default_server_log_level_value() -> String {
    DEFAULT_SERVER_LOG_LEVEL.to_string()
}

pub(in crate::config) fn default_server_url_prefix_value() -> String {
    DEFAULT_SERVER_URL_PREFIX.to_string()
}
