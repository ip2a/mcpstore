use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use super::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, ServerConfig>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub agents: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_app_description")]
    pub description: String,
    #[serde(default = "default_created_by")]
    pub created_by: String,
    #[serde(default = "default_created_at")]
    pub created_at: String,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub health_check: HealthCheckConfig,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub standalone: StandaloneConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            description: default_app_description(),
            created_by: default_created_by(),
            created_at: default_created_at(),
            cache: CacheConfig::default(),
            server: ServerSettings::default(),
            health_check: HealthCheckConfig::default(),
            monitoring: MonitoringConfig::default(),
            standalone: StandaloneConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_ui_language")]
    pub language: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: default_ui_language(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    #[serde(default = "default_server_host")]
    pub host: String,
    #[serde(default = "default_server_port")]
    pub port: u16,
    #[serde(default)]
    pub reload: bool,
    #[serde(default)]
    pub auto_open_browser: bool,
    #[serde(default = "default_true")]
    pub show_startup_info: bool,
    #[serde(default = "default_server_log_level_value")]
    pub log_level: String,
    #[serde(default = "default_server_url_prefix_value")]
    pub url_prefix: String,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            host: default_server_host(),
            port: default_server_port(),
            reload: false,
            auto_open_browser: false,
            show_startup_info: true,
            log_level: default_server_log_level_value(),
            url_prefix: default_server_url_prefix_value(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_startup_interval")]
    pub startup_interval: f64,
    #[serde(default = "default_startup_timeout")]
    pub startup_timeout: f64,
    #[serde(default = "default_startup_hard_timeout")]
    pub startup_hard_timeout: f64,
    #[serde(default = "default_readiness_interval")]
    pub readiness_interval: f64,
    #[serde(default = "default_readiness_success_threshold")]
    pub readiness_success_threshold: i32,
    #[serde(default = "default_readiness_failure_threshold")]
    pub readiness_failure_threshold: i32,
    #[serde(default = "default_liveness_interval")]
    pub liveness_interval: f64,
    #[serde(default = "default_liveness_failure_threshold")]
    pub liveness_failure_threshold: i32,
    #[serde(default = "default_ping_timeout_http")]
    pub ping_timeout_http: f64,
    #[serde(default = "default_ping_timeout_sse")]
    pub ping_timeout_sse: f64,
    #[serde(default = "default_ping_timeout_stdio")]
    pub ping_timeout_stdio: f64,
    #[serde(default = "default_warning_ping_timeout")]
    pub warning_ping_timeout: f64,
    #[serde(default = "default_window_size")]
    pub window_size: i32,
    #[serde(default = "default_window_min_calls")]
    pub window_min_calls: i32,
    #[serde(default = "default_error_rate_threshold")]
    pub error_rate_threshold: f64,
    #[serde(default = "default_latency_p95_warn")]
    pub latency_p95_warn: f64,
    #[serde(default = "default_latency_p99_critical")]
    pub latency_p99_critical: f64,
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: i32,
    #[serde(default = "default_backoff_base")]
    pub backoff_base: f64,
    #[serde(default = "default_backoff_max")]
    pub backoff_max: f64,
    #[serde(default = "default_backoff_jitter")]
    pub backoff_jitter: f64,
    #[serde(default = "default_backoff_max_duration")]
    pub backoff_max_duration: f64,
    #[serde(default = "default_half_open_max_calls")]
    pub half_open_max_calls: i32,
    #[serde(default = "default_half_open_success_rate_threshold")]
    pub half_open_success_rate_threshold: f64,
    #[serde(default = "default_reconnect_hard_timeout")]
    pub reconnect_hard_timeout: f64,
    #[serde(default = "default_lease_ttl")]
    pub lease_ttl: f64,
    #[serde(default = "default_lease_renew_interval")]
    pub lease_renew_interval: f64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            startup_interval: default_startup_interval(),
            startup_timeout: default_startup_timeout(),
            startup_hard_timeout: default_startup_hard_timeout(),
            readiness_interval: default_readiness_interval(),
            readiness_success_threshold: default_readiness_success_threshold(),
            readiness_failure_threshold: default_readiness_failure_threshold(),
            liveness_interval: default_liveness_interval(),
            liveness_failure_threshold: default_liveness_failure_threshold(),
            ping_timeout_http: default_ping_timeout_http(),
            ping_timeout_sse: default_ping_timeout_sse(),
            ping_timeout_stdio: default_ping_timeout_stdio(),
            warning_ping_timeout: default_warning_ping_timeout(),
            window_size: default_window_size(),
            window_min_calls: default_window_min_calls(),
            error_rate_threshold: default_error_rate_threshold(),
            latency_p95_warn: default_latency_p95_warn(),
            latency_p99_critical: default_latency_p99_critical(),
            max_reconnect_attempts: default_max_reconnect_attempts(),
            backoff_base: default_backoff_base(),
            backoff_max: default_backoff_max(),
            backoff_jitter: default_backoff_jitter(),
            backoff_max_duration: default_backoff_max_duration(),
            half_open_max_calls: default_half_open_max_calls(),
            half_open_success_rate_threshold: default_half_open_success_rate_threshold(),
            reconnect_hard_timeout: default_reconnect_hard_timeout(),
            lease_ttl: default_lease_ttl(),
            lease_renew_interval: default_lease_renew_interval(),
        }
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum CacheBackend {
    #[default]
    Memory,
    Redis,
    #[serde(rename = "openkeyv_memory", alias = "openkeyv-memory")]
    OpenKeyvMemory,
    #[serde(rename = "openkeyv_redis", alias = "openkeyv-redis")]
    OpenKeyvRedis,
}

impl CacheBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Redis => "redis",
            Self::OpenKeyvMemory => "openkeyv_memory",
            Self::OpenKeyvRedis => "openkeyv_redis",
        }
    }
}

impl fmt::Display for CacheBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default)]
    pub backend: CacheBackend,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis_url: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            backend: CacheBackend::Memory,
            redis_url: None,
            namespace: default_namespace(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub url: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(rename = "workingDir", default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl ServerConfig {
    pub fn infer_transport(&self) -> &str {
        if let Some(ref transport) = self.transport {
            return transport.as_str();
        }
        if self.url.is_some() {
            "streamable-http"
        } else if self.command.is_some() {
            "stdio"
        } else {
            "unknown"
        }
    }
}
