use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::cache_schema::CacheConfig;
use super::defaults::*;
use super::health_schema::HealthCheckConfig;
use super::monitoring_schema::MonitoringConfig;
use super::service_schema::ServiceLifecycleDefaults;
use super::standalone_schema::StandaloneConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub mcp_aggregate: McpAggregateConfig,
    #[serde(default)]
    pub health_check: HealthCheckConfig,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub service_defaults: ServiceDefaultsConfig,
    #[serde(default)]
    pub standalone: StandaloneConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub diagnostics: DiagnosticsConfig,
    #[serde(default)]
    pub hosts: HostsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cache: CacheConfig::default(),
            server: ServerSettings::default(),
            mcp_aggregate: McpAggregateConfig::default(),
            health_check: HealthCheckConfig::default(),
            monitoring: MonitoringConfig::default(),
            service_defaults: ServiceDefaultsConfig::default(),
            standalone: StandaloneConfig::default(),
            ui: UiConfig::default(),
            diagnostics: DiagnosticsConfig::default(),
            hosts: HostsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostsConfig {
    #[serde(default = "default_hosts_active")]
    pub active: String,
    #[serde(flatten)]
    #[serde(default)]
    pub entries: HashMap<String, HostEntry>,
}

impl Default for HostsConfig {
    fn default() -> Self {
        let mut entries = HashMap::new();
        entries.insert(
            default_hosts_active(),
            HostEntry {
                url: default_host_url(),
            },
        );
        Self {
            active: default_hosts_active(),
            entries,
        }
    }
}

impl HostsConfig {
    pub fn active_url(&self) -> String {
        self.entries
            .get(&self.active)
            .map(|entry| entry.url.clone())
            .or_else(|| {
                self.entries
                    .values()
                    .next()
                    .map(|entry| entry.url.clone())
            })
            .unwrap_or_else(default_host_url)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpAggregateConfig {
    #[serde(default = "default_mcp_aggregate_transport")]
    pub transport: String,
    #[serde(default = "default_mcp_aggregate_port")]
    pub port: u16,
}

impl Default for McpAggregateConfig {
    fn default() -> Self {
        Self {
            transport: default_mcp_aggregate_transport(),
            port: default_mcp_aggregate_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub source_log: SourceLogConfig,
    #[serde(default)]
    pub runtime_log: RuntimeLogConfig,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            source_log: SourceLogConfig::default(),
            runtime_log: RuntimeLogConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLogConfig {
    #[serde(default = "default_server_log_level_value")]
    pub level: String,
}

impl Default for SourceLogConfig {
    fn default() -> Self {
        Self {
            level: default_server_log_level_value(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLogConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_server_log_level_value")]
    pub level: String,
    #[serde(default = "default_log_max_size_bytes")]
    pub max_size_bytes: u64,
    #[serde(default)]
    pub retention_days: Option<u64>,
}

impl Default for RuntimeLogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: default_server_log_level_value(),
            max_size_bytes: default_log_max_size_bytes(),
            retention_days: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefaultsConfig {
    #[serde(default)]
    pub lifecycle: ServiceLifecycleDefaults,
}

impl Default for ServiceDefaultsConfig {
    fn default() -> Self {
        Self {
            lifecycle: ServiceLifecycleDefaults::default(),
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
    #[serde(default = "default_web_port")]
    pub web_port: u16,
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
            web_port: default_web_port(),
            reload: false,
            auto_open_browser: false,
            show_startup_info: true,
            log_level: default_server_log_level_value(),
            url_prefix: default_server_url_prefix_value(),
        }
    }
}
