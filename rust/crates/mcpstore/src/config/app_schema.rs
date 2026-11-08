use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::cache_schema::CacheConfig;
use super::defaults::*;
use super::health_schema::HealthCheckConfig;
use super::service_schema::ServiceLifecycleDefaults;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub cache: CacheConfig,
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
            cache: CacheConfig::default(),
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
pub struct DiagnosticsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub runtime_log: RuntimeLogConfig,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            runtime_log: RuntimeLogConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLogConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_runtime_log_level")]
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
            level: default_runtime_log_level(),
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
}

impl Default for WebSettings {
    fn default() -> Self {
        Self {
            host: default_web_host(),
            port: default_web_port(),
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
