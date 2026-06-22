use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

mod defaults;
pub mod models;
pub mod resolver;
pub mod validator;

use defaults::*;

pub const DEFAULT_SERVER_LOG_LEVEL: &str = "info";
pub const DEFAULT_SERVER_URL_PREFIX: &str = "";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    NotFound(PathBuf),
    #[error("JSON processing failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML parse failed: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("TOML serialization failed: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid config: {0}")]
    Invalid(String),
    #[error("Config validation failed: {0:?}")]
    Validation(Vec<validator::ValidationError>),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

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
        if let Some(ref t) = self.transport {
            return t.as_str();
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

pub struct ConfigManager {
    mcp_path: PathBuf,
    app_config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let mcp_path = resolver::resolve_mcp_config_path();
        Self {
            app_config_path: resolver::app_config_path_for_mcp_path(&mcp_path),
            mcp_path,
        }
    }

    pub fn with_path(path: impl AsRef<Path>) -> Self {
        let mcp_path = path.as_ref().to_path_buf();
        Self {
            app_config_path: resolver::app_config_path_for_mcp_path(&mcp_path),
            mcp_path,
        }
    }

    pub fn path(&self) -> &Path {
        self.mcp_path()
    }

    pub fn mcp_path(&self) -> &Path {
        &self.mcp_path
    }

    pub fn app_config_path(&self) -> &Path {
        &self.app_config_path
    }

    pub fn exists(&self) -> bool {
        self.mcp_path.exists()
    }

    pub fn app_config_exists(&self) -> bool {
        self.app_config_path.exists()
    }

    pub fn load(&self) -> Result<McpConfig> {
        if !self.mcp_path.exists() {
            return Err(ConfigError::NotFound(self.mcp_path.clone()));
        }
        let content = std::fs::read_to_string(&self.mcp_path)?;
        let config: McpConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_default(&self) -> McpConfig {
        self.load().unwrap_or_default()
    }

    pub fn save(&self, config: &McpConfig) -> Result<()> {
        if let Some(parent) = self.mcp_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(config)?;
        std::fs::write(&self.mcp_path, content)?;
        Ok(())
    }

    pub fn load_app_config(&self) -> Result<AppConfig> {
        if !self.app_config_path.exists() {
            return Err(ConfigError::NotFound(self.app_config_path.clone()));
        }
        let content = std::fs::read_to_string(&self.app_config_path)?;
        let config: AppConfig = toml::from_str(&content)?;
        validate_app_config(&config)?;
        Ok(config)
    }

    pub fn load_app_config_or_default(&self) -> Result<AppConfig> {
        match self.load_app_config() {
            Ok(config) => Ok(config),
            Err(ConfigError::NotFound(_)) => Ok(AppConfig::default()),
            Err(err) => Err(err),
        }
    }

    pub fn save_app_config(&self, config: &AppConfig) -> Result<()> {
        validate_app_config(config)?;
        if let Some(parent) = self.app_config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(config)?;
        std::fs::write(&self.app_config_path, content)?;
        Ok(())
    }

    pub fn default_app_config_toml(&self) -> Result<String> {
        toml::to_string_pretty(&AppConfig::default()).map_err(Into::into)
    }

    pub fn load_raw_app_config_value(&self) -> Result<Value> {
        if !self.app_config_path.exists() {
            return Ok(Value::Object(Map::new()));
        }
        let content = std::fs::read_to_string(&self.app_config_path)?;
        let value: toml::Value = toml::from_str(&content)?;
        serde_json::to_value(value).map_err(Into::into)
    }

    pub fn flatten_raw_app_config(&self) -> Result<HashMap<String, Value>> {
        let value = self.load_raw_app_config_value()?;
        Ok(flatten_config_value(&value))
    }

    pub fn validate(&self) -> Result<()> {
        let config = self.load()?;
        let servers: HashMap<String, models::ServerConfigFull> = config
            .mcp_servers
            .into_iter()
            .map(|(k, v)| {
                let full = models::ServerConfigFull {
                    url: v.url,
                    command: v.command,
                    args: v.args,
                    env: v.env,
                    headers: v.headers,
                    transport: v
                        .transport
                        .and_then(|t| serde_json::from_value(serde_json::Value::String(t)).ok()),
                    working_dir: v.working_dir,
                };
                (k, full)
            })
            .collect();
        validator::validate(&servers).map_err(ConfigError::Validation)?;
        self.validate_app_config()?;
        Ok(())
    }

    pub fn validate_app_config(&self) -> Result<()> {
        self.load_app_config_or_default().map(|_| ())
    }

    pub fn init(&self, with_examples: bool, redis_url: Option<String>) -> Result<()> {
        let mut config = McpConfig::default();
        let mut app_config = AppConfig::default();
        if let Some(redis_url) = redis_url {
            app_config.cache = CacheConfig {
                backend: CacheBackend::Redis,
                redis_url: Some(redis_url),
                namespace: "mcpstore".to_string(),
            };
        }
        if with_examples {
            config.mcp_servers.extend(example_services());
        }
        self.save(&config)?;
        self.save_app_config(&app_config)
    }

    pub fn add_examples(&self) -> Result<usize> {
        let mut config = self.load_or_default();
        let mut added = 0usize;

        for (name, service) in example_services() {
            if config.mcp_servers.contains_key(&name) {
                continue;
            }
            config.mcp_servers.insert(name, service);
            added += 1;
        }

        self.save(&config)?;
        Ok(added)
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

fn default_server_config() -> ServerConfig {
    ServerConfig {
        url: None,
        command: None,
        args: Vec::new(),
        env: HashMap::new(),
        headers: HashMap::new(),
        transport: None,
        working_dir: None,
        description: None,
    }
}

fn example_services() -> HashMap<String, ServerConfig> {
    let mut services = HashMap::new();
    services.insert(
        "remote-http-service".to_string(),
        ServerConfig {
            url: Some("https://example.com/mcp".to_string()),
            transport: Some("streamable-http".to_string()),
            description: Some("Example remote HTTP MCP service".to_string()),
            ..default_server_config()
        },
    );
    services.insert(
        "local-command-service".to_string(),
        ServerConfig {
            command: Some("python".to_string()),
            args: vec!["-m".to_string(), "your_mcp_server".to_string()],
            env: HashMap::new(),
            working_dir: Some(".".to_string()),
            description: Some("Example local command MCP service".to_string()),
            ..default_server_config()
        },
    );
    services.insert(
        "npm-package-service".to_string(),
        ServerConfig {
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "some-mcp-package".to_string()],
            transport: Some("stdio".to_string()),
            description: Some("Example NPM package MCP service".to_string()),
            ..default_server_config()
        },
    );
    services
}

fn flatten_config_value(value: &Value) -> HashMap<String, Value> {
    let mut out = HashMap::new();

    fn visit(prefix: &str, value: &Value, out: &mut HashMap<String, Value>) {
        match value {
            Value::Object(map) => {
                for (key, item) in map {
                    let next = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    visit(&next, item, out);
                }
            }
            other => {
                if !prefix.is_empty() {
                    out.insert(prefix.to_string(), other.clone());
                }
            }
        }
    }

    visit("", value, &mut out);
    out
}

fn validate_app_config(config: &AppConfig) -> Result<()> {
    let mut errors = Vec::new();
    let health = &config.health_check;
    let monitoring = &config.monitoring;
    let standalone = &config.standalone;

    validate_non_empty("server.host", &config.server.host, &mut errors);
    validate_range_f64(
        "health_check.startup_interval",
        health.startup_interval,
        0.1,
        60.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.startup_timeout",
        health.startup_timeout,
        1.0,
        1800.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.startup_hard_timeout",
        health.startup_hard_timeout,
        1.0,
        7200.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.readiness_interval",
        health.readiness_interval,
        1.0,
        300.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.readiness_success_threshold",
        health.readiness_success_threshold,
        1,
        10,
        &mut errors,
    );
    validate_range_i32(
        "health_check.readiness_failure_threshold",
        health.readiness_failure_threshold,
        1,
        10,
        &mut errors,
    );
    validate_range_f64(
        "health_check.liveness_interval",
        health.liveness_interval,
        1.0,
        300.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.liveness_failure_threshold",
        health.liveness_failure_threshold,
        1,
        10,
        &mut errors,
    );
    validate_range_f64(
        "health_check.ping_timeout_http",
        health.ping_timeout_http,
        0.1,
        600.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.ping_timeout_sse",
        health.ping_timeout_sse,
        0.1,
        600.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.ping_timeout_stdio",
        health.ping_timeout_stdio,
        0.1,
        1200.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.warning_ping_timeout",
        health.warning_ping_timeout,
        0.1,
        1200.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.window_size",
        health.window_size,
        1,
        1000,
        &mut errors,
    );
    validate_range_i32(
        "health_check.window_min_calls",
        health.window_min_calls,
        1,
        1000,
        &mut errors,
    );
    validate_range_f64(
        "health_check.error_rate_threshold",
        health.error_rate_threshold,
        0.0,
        1.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.latency_p95_warn",
        health.latency_p95_warn,
        0.01,
        30.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.latency_p99_critical",
        health.latency_p99_critical,
        0.01,
        60.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.max_reconnect_attempts",
        health.max_reconnect_attempts,
        1,
        100,
        &mut errors,
    );
    validate_range_f64(
        "health_check.backoff_base",
        health.backoff_base,
        0.1,
        300.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.backoff_max",
        health.backoff_max,
        1.0,
        3600.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.backoff_jitter",
        health.backoff_jitter,
        0.0,
        1.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.backoff_max_duration",
        health.backoff_max_duration,
        1.0,
        7200.0,
        &mut errors,
    );
    validate_range_i32(
        "health_check.half_open_max_calls",
        health.half_open_max_calls,
        1,
        100,
        &mut errors,
    );
    validate_range_f64(
        "health_check.half_open_success_rate_threshold",
        health.half_open_success_rate_threshold,
        0.0,
        1.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.reconnect_hard_timeout",
        health.reconnect_hard_timeout,
        1.0,
        7200.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.lease_ttl",
        health.lease_ttl,
        1.0,
        3600.0,
        &mut errors,
    );
    validate_range_f64(
        "health_check.lease_renew_interval",
        health.lease_renew_interval,
        0.5,
        3600.0,
        &mut errors,
    );

    validate_range_i32(
        "monitoring.reconnection_seconds",
        monitoring.reconnection_seconds,
        5,
        1800,
        &mut errors,
    );
    validate_range_f64(
        "monitoring.cleanup_hours",
        monitoring.cleanup_hours,
        0.1,
        168.0,
        &mut errors,
    );
    validate_range_i32(
        "monitoring.local_service_ping_timeout",
        monitoring.local_service_ping_timeout,
        1,
        60,
        &mut errors,
    );
    validate_range_i32(
        "monitoring.remote_service_ping_timeout",
        monitoring.remote_service_ping_timeout,
        1,
        120,
        &mut errors,
    );
    validate_range_f64(
        "monitoring.adaptive_timeout_multiplier",
        monitoring.adaptive_timeout_multiplier,
        1.0,
        5.0,
        &mut errors,
    );
    validate_range_i32(
        "monitoring.response_time_history_size",
        monitoring.response_time_history_size,
        5,
        100,
        &mut errors,
    );

    validate_range_f64(
        "standalone.heartbeat_interval_seconds",
        standalone.heartbeat_interval_seconds,
        1.0,
        300.0,
        &mut errors,
    );
    validate_range_f64(
        "standalone.http_timeout_seconds",
        standalone.http_timeout_seconds,
        1.0,
        300.0,
        &mut errors,
    );
    validate_range_f64(
        "standalone.reconnection_interval_seconds",
        standalone.reconnection_interval_seconds,
        1.0,
        1800.0,
        &mut errors,
    );
    validate_range_f64(
        "standalone.cleanup_interval_seconds",
        standalone.cleanup_interval_seconds,
        10.0,
        3600.0,
        &mut errors,
    );
    validate_non_empty(
        "standalone.streamable_http_endpoint",
        &standalone.streamable_http_endpoint,
        &mut errors,
    );
    validate_allowed(
        "standalone.default_transport",
        &standalone.default_transport,
        &["stdio", "sse", "websocket"],
        &mut errors,
    );
    validate_allowed(
        "standalone.log_level",
        &standalone.log_level,
        &["DEBUG", "INFO", "DEGRADED", "ERROR"],
        &mut errors,
    );
    validate_allowed(
        "standalone.log_format",
        &standalone.log_format,
        &["json", "text"],
        &mut errors,
    );
    validate_allowed(
        "server.log_level",
        &config.server.log_level,
        &["debug", "info", "degraded", "error", "critical"],
        &mut errors,
    );

    if !errors.is_empty() {
        return Err(ConfigError::Invalid(errors.join("; ")));
    }
    Ok(())
}

fn validate_range_f64(name: &str, value: f64, min: f64, max: f64, errors: &mut Vec<String>) {
    if value < min || value > max {
        errors.push(format!("{name}={value} out of range [{min}, {max}]"));
    }
}

fn validate_range_i32(name: &str, value: i32, min: i32, max: i32, errors: &mut Vec<String>) {
    if value < min || value > max {
        errors.push(format!("{name}={value} out of range [{min}, {max}]"));
    }
}

fn validate_non_empty(name: &str, value: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{name} cannot be empty"));
    }
}

fn validate_allowed(name: &str, value: &str, allowed: &[&str], errors: &mut Vec<String>) {
    if !allowed.iter().any(|candidate| candidate == &value) {
        errors.push(format!(
            "{name}='{value}' is invalid, allowed values: {}",
            allowed.join(", ")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_save_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mcp.json");

        let mgr = ConfigManager::with_path(&path);
        let mut config = McpConfig::default();
        config.mcp_servers.insert(
            "test".to_string(),
            ServerConfig {
                url: Some("http://localhost:8080/mcp".to_string()),
                transport: Some("streamable-http".to_string()),
                ..default_server_config()
            },
        );

        mgr.save(&config).unwrap();
        let loaded = mgr.load().unwrap();
        assert_eq!(loaded.mcp_servers.len(), 1);
        assert!(loaded.mcp_servers.contains_key("test"));
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("cache"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_app_config_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));

        let mut config = AppConfig::default();
        config.cache.backend = CacheBackend::Redis;
        config.cache.redis_url = Some("redis://127.0.0.1/".to_string());
        config.server.log_level = "debug".to_string();
        config.server.url_prefix = "/demo".to_string();

        mgr.save_app_config(&config).unwrap();
        let loaded = mgr.load_app_config().unwrap();
        assert_eq!(loaded.cache.backend, CacheBackend::Redis);
        assert_eq!(loaded.cache.namespace, "mcpstore");
        assert_eq!(loaded.server.log_level, "debug");
        assert_eq!(loaded.server.url_prefix, "/demo");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_default_template_contains_runtime_sections() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));
        let template = mgr.default_app_config_toml().unwrap();

        assert!(template.contains("[server]"));
        assert!(template.contains("[health_check]"));
        assert!(template.contains("[monitoring]"));
        assert!(template.contains("[standalone]"));
        assert!(template.contains("[ui]"));
        assert!(template.contains("language = \"zh-cn\""));
        assert!(template.contains("log_level = \"info\""));
    }

    #[test]
    fn test_flatten_raw_app_config_only_contains_explicit_keys() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));
        std::fs::write(
            mgr.app_config_path(),
            "[server]\nhost = \"127.0.0.1\"\n[health_check]\nstartup_interval = 2.5\n",
        )
        .unwrap();

        let flattened = mgr.flatten_raw_app_config().unwrap();
        assert_eq!(
            flattened.get("server.host"),
            Some(&Value::String("127.0.0.1".to_string()))
        );
        assert_eq!(
            flattened.get("health_check.startup_interval"),
            Some(&serde_json::json!(2.5))
        );
        assert!(!flattened.contains_key("server.port"));
    }

    #[test]
    fn test_invalid_app_config_fails_fast() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));
        std::fs::write(
            mgr.app_config_path(),
            "[health_check]\nstartup_interval = 0.0\n",
        )
        .unwrap();

        let err = mgr.load_app_config().unwrap_err();
        assert!(err.to_string().contains("health_check.startup_interval"));
    }

    #[test]
    fn test_init_splits_mcp_json_and_app_toml() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));

        mgr.init(true, Some("redis://127.0.0.1/".to_string()))
            .unwrap();

        let mcp_raw = std::fs::read_to_string(mgr.mcp_path()).unwrap();
        assert!(mcp_raw.contains("mcpServers"));
        assert!(!mcp_raw.contains("cache"));

        let app_config = mgr.load_app_config().unwrap();
        assert_eq!(app_config.cache.backend, CacheBackend::Redis);
        assert_eq!(
            app_config.cache.redis_url.as_deref(),
            Some("redis://127.0.0.1/")
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_infer_transport() {
        let http = ServerConfig {
            url: Some("http://x".into()),
            ..default_server_config()
        };
        assert_eq!(http.infer_transport(), "streamable-http");

        let stdio = ServerConfig {
            command: Some("python".into()),
            ..default_server_config()
        };
        assert_eq!(stdio.infer_transport(), "stdio");
    }

    #[test]
    fn test_add_examples_only_inserts_missing_entries() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        let path = dir.join("mcp.json");
        let mgr = ConfigManager::with_path(&path);

        let mut config = McpConfig::default();
        config.mcp_servers.insert(
            "remote-http-service".to_string(),
            ServerConfig {
                url: Some("https://override.example.com/mcp".to_string()),
                transport: Some("streamable-http".to_string()),
                description: Some("Existing service".to_string()),
                ..default_server_config()
            },
        );
        config.mcp_servers.insert(
            "custom-service".to_string(),
            ServerConfig {
                command: Some("echo".to_string()),
                args: vec!["ok".to_string()],
                transport: Some("stdio".to_string()),
                ..default_server_config()
            },
        );

        mgr.save(&config).unwrap();
        let added = mgr.add_examples().unwrap();
        let loaded = mgr.load().unwrap();

        assert_eq!(added, 2);
        assert_eq!(loaded.mcp_servers.len(), 4);
        assert_eq!(
            loaded
                .mcp_servers
                .get("remote-http-service")
                .and_then(|svc| svc.url.as_deref()),
            Some("https://override.example.com/mcp")
        );
        assert!(loaded.mcp_servers.contains_key("local-command-service"));
        assert!(loaded.mcp_servers.contains_key("npm-package-service"));
        assert!(loaded.mcp_servers.contains_key("custom-service"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
