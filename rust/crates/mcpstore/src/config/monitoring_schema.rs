use serde::{Deserialize, Serialize};

use super::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    #[serde(default = "default_monitoring_reconnection_seconds")]
    pub reconnection_seconds: i32,
    #[serde(default = "default_monitoring_cleanup_hours")]
    pub cleanup_hours: f64,
    #[serde(default = "default_true")]
    pub enable_reconnection: bool,
    #[serde(default = "default_local_service_ping_timeout")]
    pub local_service_ping_timeout: i32,
    #[serde(default = "default_remote_service_ping_timeout")]
    pub remote_service_ping_timeout: i32,
    #[serde(default = "default_true")]
    pub enable_adaptive_timeout: bool,
    #[serde(default = "default_adaptive_timeout_multiplier")]
    pub adaptive_timeout_multiplier: f64,
    #[serde(default = "default_response_time_history_size")]
    pub response_time_history_size: i32,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            reconnection_seconds: default_monitoring_reconnection_seconds(),
            cleanup_hours: default_monitoring_cleanup_hours(),
            enable_reconnection: true,
            local_service_ping_timeout: default_local_service_ping_timeout(),
            remote_service_ping_timeout: default_remote_service_ping_timeout(),
            enable_adaptive_timeout: true,
            adaptive_timeout_multiplier: default_adaptive_timeout_multiplier(),
            response_time_history_size: default_response_time_history_size(),
        }
    }
}
