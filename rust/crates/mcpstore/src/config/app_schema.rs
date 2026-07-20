use serde::{Deserialize, Serialize};

use super::cache_schema::CacheConfig;
use super::defaults::*;
use super::health_schema::HealthCheckConfig;
use super::monitoring_schema::MonitoringConfig;
use super::service_schema::ServiceLifecycleDefaults;
use super::standalone_schema::StandaloneConfig;

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
    pub service_defaults: ServiceDefaultsConfig,
    #[serde(default)]
    pub standalone: StandaloneConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub diagnostics: DiagnosticsConfig,
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
            service_defaults: ServiceDefaultsConfig::default(),
            standalone: StandaloneConfig::default(),
            ui: UiConfig::default(),
            diagnostics: DiagnosticsConfig::default(),
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
    #[serde(default)]
    pub history: HistoryConfig,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            source_log: SourceLogConfig::default(),
            runtime_log: RuntimeLogConfig::default(),
            history: HistoryConfig::default(),
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
    #[serde(default)]
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
            enabled: false,
            level: default_server_log_level_value(),
            max_size_bytes: default_log_max_size_bytes(),
            retention_days: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub storage: HistoryStorage,
    #[serde(default = "default_history_max_records")]
    pub max_records: usize,
    #[serde(default = "default_history_max_size_bytes")]
    pub max_size_bytes: u64,
    #[serde(default)]
    pub retention_days: Option<u64>,
    #[serde(default)]
    pub payload: HistoryPayload,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            storage: HistoryStorage::Memory,
            max_records: default_history_max_records(),
            max_size_bytes: default_history_max_size_bytes(),
            retention_days: None,
            payload: HistoryPayload::Metadata,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HistoryStorage {
    #[default]
    Memory,
    Disk,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HistoryPayload {
    None,
    #[default]
    Metadata,
    Full,
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
    #[serde(default = "default_backup_dir")]
    pub default_backup_dir: String,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: default_ui_language(),
            default_backup_dir: default_backup_dir(),
            logging: LoggingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_max_size_bytes")]
    pub max_size_bytes: u64,
    #[serde(default)]
    pub retention_days: Option<u64>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: default_log_max_size_bytes(),
            retention_days: None,
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
