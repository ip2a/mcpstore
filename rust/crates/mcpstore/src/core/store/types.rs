use crate::cache::models::HealthStatus;
use crate::config::AppConfig;
use crate::registry::ServiceEntry;

#[derive(Clone, Debug)]
pub(super) struct StoreRuntimeConfig {
    pub(super) max_connection_attempts: i32,
    pub(super) retry_backoff_base_secs: i64,
    pub(super) retry_backoff_max_secs: i64,
    pub(super) reconnect_hard_timeout_secs: i64,
    pub(super) half_open_lease_secs: i64,
    pub(super) health_warn_latency_ms: f64,
}

#[derive(Debug, Clone)]
pub struct ScopedServiceEntry {
    pub service: ServiceEntry,
    pub tool_count: usize,
    pub global_name: Option<String>,
    pub client_id: String,
}

#[derive(Debug, Clone)]
pub struct ScopedToolEntry {
    pub name: String,
    pub original_name: String,
    pub description: String,
    pub schema: serde_json::Value,
    pub input_schema: serde_json::Value,
    pub service_name: String,
    pub global_service_name: String,
    pub service_global_name: String,
    pub global_tool_name: String,
    pub client_id: String,
}

#[derive(Debug, Clone)]
pub struct ScopedServiceHealth {
    pub service_name: String,
    pub health_status: HealthStatus,
}

#[derive(Debug, Clone)]
pub struct EventCapabilityReport {
    pub event_bus: bool,
    pub history: bool,
    pub history_capacity: usize,
    pub cache_event_layer: bool,
}

#[derive(Debug, Clone)]
pub struct CacheHealthReport {
    pub namespace: String,
    pub backend: String,
    pub entities: Vec<String>,
    pub relations: Vec<String>,
    pub states: Vec<String>,
    pub events: Vec<String>,
}

impl StoreRuntimeConfig {
    pub(super) fn from_app_config(config: &AppConfig) -> Self {
        let health = &config.health_check;
        Self {
            max_connection_attempts: health.max_reconnect_attempts.max(1),
            retry_backoff_base_secs: ceil_seconds(health.backoff_base, 1),
            retry_backoff_max_secs: ceil_seconds(health.backoff_max, 1),
            reconnect_hard_timeout_secs: ceil_seconds(health.reconnect_hard_timeout, 1),
            half_open_lease_secs: ceil_seconds(health.lease_ttl, 1),
            health_warn_latency_ms: health.latency_p95_warn.max(0.01) * 1000.0,
        }
    }
}

fn ceil_seconds(value: f64, minimum: i64) -> i64 {
    let rounded = value.ceil() as i64;
    rounded.max(minimum)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum SourceMode {
    #[default]
    Local,
    Db,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum CacheStorage {
    #[default]
    Memory,
    Redis,
    OpenKeyvMemory,
    OpenKeyvRedis,
}

impl CacheStorage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Redis => "redis",
            Self::OpenKeyvMemory => "openkeyv_memory",
            Self::OpenKeyvRedis => "openkeyv_redis",
        }
    }
}

pub type BackendKind = CacheStorage;

#[derive(Clone, Debug)]
pub struct StoreOptions {
    pub config_path: Option<String>,
    pub source_mode: SourceMode,
    pub backend: Option<CacheStorage>,
    pub redis_url: Option<String>,
    pub namespace: Option<String>,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            source_mode: SourceMode::Local,
            backend: None,
            redis_url: None,
            namespace: None,
        }
    }
}
