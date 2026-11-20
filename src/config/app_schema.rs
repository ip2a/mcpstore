use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::defaults::*;
use super::health_schema::HealthCheckConfig;
use super::kv_schema::KvConfig;
use super::service_schema::ServiceLifecycleDefaults;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub kv: KvConfig,
    #[serde(default)]
    pub api: ApiSettings,
    #[serde(default)]
    pub web: WebSettings,
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub gateway: GatewayConfig,
    #[serde(default)]
    pub health_check: HealthCheckConfig,
    #[serde(default)]
    pub service_defaults: ServiceDefaultsConfig,
    #[serde(default)]
    pub diagnostics: DiagnosticsConfig,
    /// Per-panel placement decisions, data-panel only. Keys are `"{scope}.{service}"`
    /// (e.g. `"agentA.github"`, `"store.context7"`); the only meaningful value is
    /// `"delegate"` (the control panel executes). Missing entries, an absent section or
    /// any other value mean local execution — never validated, never an error.
    /// The control panel never reads this section.
    #[serde(default)]
    pub placement: HashMap<String, String>,
    /// Client-plugin state: which installed plugins are enabled, and where the
    /// market index lives.
    #[serde(default)]
    pub plugins: PluginsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            kv: KvConfig::default(),
            api: ApiSettings::default(),
            web: WebSettings::default(),
            server: ServerSettings::default(),
            gateway: GatewayConfig::default(),
            health_check: HealthCheckConfig::default(),
            service_defaults: ServiceDefaultsConfig::default(),
            diagnostics: DiagnosticsConfig::default(),
            placement: HashMap::new(),
            plugins: PluginsConfig::default(),
        }
    }
}

/// Plugin enablement and market source. `enabled` lists plugin ids; an id
/// pointing at an uninstalled plugin is harmless and ignored by the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
    /// Market index base URLs, tried in order.
    #[serde(default = "default_market_urls")]
    pub market_urls: Vec<String>,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: Vec::new(),
            market_urls: default_market_urls(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostEntry {
    /// Candidate base URLs in preference order; the first reachable one wins.
    pub urls: Vec<String>,
}

// One-time config migration: entries written as `{ url = "..." }` (single URL)
// normalize to `{ urls = ["..."] }` at the deserialize boundary. The runtime
// type only knows the new shape; the file is rewritten in the new format on
// the next config save.
impl<'de> Deserialize<'de> for HostEntry {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Multi { urls: Vec<String> },
            Legacy { url: String },
        }
        match Raw::deserialize(deserializer)? {
            Raw::Multi { urls } => Ok(Self { urls }),
            Raw::Legacy { url } => Ok(Self { urls: vec![url] }),
        }
    }
}

// Custom Deserialize: serde ignores field defaults on #[serde(flatten)] maps,
// so a plain derive would start from an empty map and lose the default entry
// for keys omitted from disk by sparse persist.
impl<'de> Deserialize<'de> for HostsConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default = "default_hosts_active")]
            active: String,
            #[serde(flatten)]
            entries: HashMap<String, HostEntry>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let mut entries = default_hosts_entries();
        entries.extend(raw.entries);
        Ok(Self {
            active: raw.active,
            entries,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostsConfig {
    #[serde(default = "default_hosts_active")]
    pub active: String,
    #[serde(flatten)]
    pub entries: HashMap<String, HostEntry>,
}

pub(in crate::config) fn default_hosts_entries() -> HashMap<String, HostEntry> {
    let mut entries = HashMap::new();
    entries.insert(
        default_hosts_active(),
        HostEntry {
            urls: vec![default_host_url()],
        },
    );
    entries
}

impl Default for HostsConfig {
    fn default() -> Self {
        Self {
            active: default_hosts_active(),
            entries: default_hosts_entries(),
        }
    }
}

impl HostsConfig {
    /// The URL the serde defaults generate for the `local` entry (built from
    /// the default server port). Callers compare against this to detect an
    /// untouched default that should follow the daemon's real `[server].port`.
    pub fn generated_default_local_url() -> String {
        default_host_url()
    }

    /// URLs of the active host in preference order.
    pub fn active_urls(&self) -> Vec<String> {
        self.entries
            .get(&self.active)
            .map(|entry| entry.urls.clone())
            .or_else(|| self.entries.values().next().map(|entry| entry.urls.clone()))
            .unwrap_or_else(|| vec![default_host_url()])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default = "default_gateway_port")]
    pub port: u16,
    /// Bind host for gateway (default 127.0.0.1 for localhost-only; use 0.0.0.0 for remote clients)
    #[serde(default = "default_gateway_host")]
    pub host: String,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            port: default_gateway_port(),
            host: default_gateway_host(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagnosticsConfig {
    pub runtime_log: RuntimeLogConfig,
    pub requests: RequestTelemetryConfig,
    pub gateway_calls: GatewayCallsConfig,
    pub frames: FrameTelemetryConfig,
    pub event_history: EventHistoryConfig,
    pub stdio_stderr: StdioStderrConfig,
    pub daemon_output: DaemonOutputConfig,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            runtime_log: RuntimeLogConfig::default(),
            requests: RequestTelemetryConfig::default(),
            gateway_calls: GatewayCallsConfig::default(),
            frames: FrameTelemetryConfig::default(),
            event_history: EventHistoryConfig::default(),
            stdio_stderr: StdioStderrConfig::default(),
            daemon_output: DaemonOutputConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeLogConfig {
    pub enabled: bool,
    pub level: String,
    pub max_size_bytes: u64,
    #[serde(deserialize_with = "deserialize_optional_limit")]
    pub retention_days: Option<u64>,
}

impl Default for RuntimeLogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: default_runtime_log_level(),
            max_size_bytes: default_log_max_size_bytes(),
            retention_days: None,
        }
    }
}

fn deserialize_optional_limit<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(value) => value.as_u64().map(Some).ok_or_else(|| {
            serde::de::Error::custom("limit must be a non-negative integer or 'unlimited'")
        }),
        serde_json::Value::String(value) if value == "unlimited" => Ok(None),
        _ => Err(serde::de::Error::custom(
            "limit must be a non-negative integer or 'unlimited'",
        )),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RequestTelemetryConfig {
    pub enabled: bool,
    pub capture_tool: bool,
    pub capture_management: bool,
    pub capture_cli: bool,
    pub capture_payload: bool,
    pub max_payload_bytes: usize,
    #[serde(deserialize_with = "deserialize_optional_limit")]
    pub retention_days: Option<u64>,
    #[serde(deserialize_with = "deserialize_optional_limit")]
    pub max_records: Option<u64>,
}

impl Default for RequestTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_tool: true,
            capture_management: true,
            capture_cli: true,
            capture_payload: true,
            max_payload_bytes: 1024 * 1024,
            retention_days: Some(30),
            max_records: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayCallsConfig {
    pub enabled: bool,
    pub persist: bool,
    pub memory_capacity: usize,
    #[serde(deserialize_with = "deserialize_optional_limit")]
    pub retention_days: Option<u64>,
    #[serde(deserialize_with = "deserialize_optional_limit")]
    pub max_calls: Option<u64>,
}

impl Default for GatewayCallsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            persist: true,
            memory_capacity: 256,
            retention_days: Some(30),
            max_calls: Some(5_000),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FrameTelemetryConfig {
    pub enabled: bool,
    pub capture_gateway: bool,
    pub capture_upstream: bool,
    pub capture_payload: bool,
    pub max_payload_bytes: usize,
    #[serde(deserialize_with = "deserialize_optional_limit")]
    pub retention_days: Option<u64>,
    #[serde(deserialize_with = "deserialize_optional_limit")]
    pub max_records: Option<u64>,
}

impl Default for FrameTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_gateway: true,
            capture_upstream: true,
            capture_payload: true,
            max_payload_bytes: 1024 * 1024,
            retention_days: Some(7),
            max_records: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EventHistoryConfig {
    pub enabled: bool,
    pub capacity: usize,
}

impl Default for EventHistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            capacity: 10_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StdioStderrConfig {
    pub enabled: bool,
    pub tail_lines: usize,
    pub forward_to_runtime_log: bool,
    pub include_in_error: bool,
}

impl Default for StdioStderrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tail_lines: 50,
            forward_to_runtime_log: true,
            include_in_error: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonOutputConfig {
    pub enabled: bool,
}

impl Default for DaemonOutputConfig {
    fn default() -> Self {
        Self { enabled: true }
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
pub struct ApiSettings {
    #[serde(default = "default_api_host")]
    pub host: String,
    #[serde(default = "default_api_port")]
    pub port: u16,
    #[serde(default = "default_api_url_prefix")]
    pub url_prefix: String,
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: default_api_port(),
            url_prefix: default_api_url_prefix(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSettings {
    #[serde(default = "default_web_host")]
    pub host: String,
    #[serde(default = "default_web_port")]
    pub port: u16,
    /// Panel token authentication; off by default, opt in with `[web] auth = true`.
    #[serde(default)]
    pub auth: bool,
}

impl Default for WebSettings {
    fn default() -> Self {
        Self {
            host: default_web_host(),
            port: default_web_port(),
            auth: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    #[serde(default = "default_server_host")]
    pub host: String,
    #[serde(default = "default_server_port")]
    pub port: u16,
    /// Kernel RPC TCP listener port; 0 disables remote daemon access.
    #[serde(default)]
    pub rpc_port: u16,
    /// Required shared secret for Kernel RPC TCP connections.
    #[serde(default)]
    pub rpc_token: Option<String>,
    #[serde(default = "default_true")]
    pub core_enabled: bool,
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
            rpc_port: 0,
            rpc_token: None,
            core_enabled: true,
            reload: false,
            auto_open_browser: false,
            show_startup_info: true,
            log_level: default_server_log_level_value(),
            url_prefix: default_server_url_prefix_value(),
        }
    }
}
