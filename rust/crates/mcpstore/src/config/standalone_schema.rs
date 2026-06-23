use serde::{Deserialize, Serialize};

use super::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandaloneConfig {
    #[serde(default = "default_standalone_heartbeat_interval_seconds")]
    pub heartbeat_interval_seconds: f64,
    #[serde(default = "default_standalone_http_timeout_seconds")]
    pub http_timeout_seconds: f64,
    #[serde(default = "default_standalone_reconnection_interval_seconds")]
    pub reconnection_interval_seconds: f64,
    #[serde(default = "default_standalone_cleanup_interval_seconds")]
    pub cleanup_interval_seconds: f64,
    #[serde(default = "default_streamable_http_endpoint")]
    pub streamable_http_endpoint: String,
    #[serde(default = "default_standalone_transport")]
    pub default_transport: String,
    #[serde(default = "default_standalone_log_level")]
    pub log_level: String,
    #[serde(default = "default_standalone_log_format")]
    pub log_format: String,
    #[serde(default)]
    pub enable_debug: bool,
}

impl Default for StandaloneConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_seconds: default_standalone_heartbeat_interval_seconds(),
            http_timeout_seconds: default_standalone_http_timeout_seconds(),
            reconnection_interval_seconds: default_standalone_reconnection_interval_seconds(),
            cleanup_interval_seconds: default_standalone_cleanup_interval_seconds(),
            streamable_http_endpoint: default_streamable_http_endpoint(),
            default_transport: default_standalone_transport(),
            log_level: default_standalone_log_level(),
            log_format: default_standalone_log_format(),
            enable_debug: false,
        }
    }
}
