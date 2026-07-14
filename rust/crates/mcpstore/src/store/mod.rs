use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, RwLock as SyncRwLock};

pub(crate) use crate::cache::models::{
    HealthStatus, OpenApiImportContextState, ServiceLifecycleState, ToolAvailability, ToolEntity,
    ToolStatusItem,
};
pub(crate) use crate::cache::CacheLayerManager;
pub(crate) use crate::config::{CacheBackend, ConfigManager, ServerConfig, StartupPolicy};
pub(crate) use crate::events::{Event, EventBus};
pub(crate) use crate::registry::{
    ConfigRevision, ConnectionStatus, ServiceDefinition, ServiceInstance, ServiceRegistry,
};
pub(crate) use crate::transport::client::ConnectionPool;
pub(crate) use crate::transport::{
    DiscoveredPrompt, DiscoveredResource, DiscoveredResourceTemplate,
};

pub(crate) use crate::{Result, StoreError};

mod openapi;
mod options;
pub(crate) mod payload;
mod runtime;
mod tool_changes;
use runtime::StoreRuntimeConfig;

pub use crate::agent::models::{ScopedServiceEntry, ScopedServiceHealth, ScopedToolEntry};
pub use crate::agent::tool_visibility::ToolVisibilityFilter;
pub use crate::cache::models::CacheHealthReport;
pub use crate::events::EventCapabilityReport;
pub use crate::openapi::{
    OpenApiBundleArtifact, OpenApiBundleDependency, OpenApiBundleDiagnostic, OpenApiBundleDocument,
    OpenApiImportOptions, OpenApiImportResult,
};
pub use openapi::{OpenApiImportInput, OpenApiImportSource};
pub use options::{BackendKind, CacheStorage, SourceMode, StoreOptions};
pub use tool_changes::{ToolChangeServiceResult, ToolChangeSummary};

pub(crate) const CONTROL_REQUEST_EVENT_TYPE: &str = "control_requests";
pub(crate) static CONTROL_EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) mod prelude {
    pub(crate) use crate::config_formats::{project_config, ConfigFormat};
    pub(crate) use crate::identity::{InstanceId, ScopeRef, ServiceInstanceKey};
    pub(crate) use crate::store::payload::wrap_cache_item;
    pub(crate) use crate::store::{
        BackendKind, CacheHealthReport, CacheStorage, ConfigRevision, ConnectionStatus,
        DiscoveredPrompt, DiscoveredResource, DiscoveredResourceTemplate, Event, HealthStatus,
        MCPStore, OpenApiImportContextState, Result, ScopedServiceEntry, ScopedToolEntry,
        ServerConfig, ServiceDefinition, ServiceInstance, ServiceLifecycleState, SourceMode,
        StartupPolicy, StoreError, ToolAvailability, ToolChangeServiceResult, ToolChangeSummary,
        ToolEntity, ToolStatusItem, CONTROL_EVENT_SEQUENCE, CONTROL_REQUEST_EVENT_TYPE,
    };
}

pub struct MCPStore {
    pub(crate) config_manager: ConfigManager,
    pub(crate) source_mode: SourceMode,
    pub(crate) runtime_config: StoreRuntimeConfig,
    pub(crate) cache_storage: tokio::sync::RwLock<CacheStorage>,
    pub(crate) redis_url: tokio::sync::RwLock<Option<String>>,
    pub(crate) namespace: SyncRwLock<String>,
    pub(crate) registry: ServiceRegistry,
    pub(crate) pool: ConnectionPool,
    pub(crate) applied_openapi_configs: tokio::sync::RwLock<
        HashMap<crate::identity::InstanceId, serde_json::Map<String, serde_json::Value>>,
    >,
    pub(crate) event_bus: EventBus,
    pub(crate) cache: CacheLayerManager,
}

impl MCPStore {
    pub fn setup(config_path: Option<&str>) -> Result<Self> {
        Self::setup_with_options(StoreOptions {
            config_path: config_path.map(ToString::to_string),
            ..StoreOptions::default()
        })
    }

    pub fn setup_with_options(options: StoreOptions) -> Result<Self> {
        let config_manager = match options.config_path.as_deref() {
            Some(p) => ConfigManager::with_path(p),
            None => ConfigManager::new(),
        };

        let app_config = config_manager.load_app_config_or_default()?;
        let runtime_config = StoreRuntimeConfig::from_app_config(&app_config);
        let namespace = options
            .namespace
            .clone()
            .unwrap_or_else(|| app_config.cache.namespace.clone());
        let cache_storage = options
            .backend
            .clone()
            .unwrap_or(match &app_config.cache.backend {
                CacheBackend::Redis => CacheStorage::Redis,
                CacheBackend::Memory => CacheStorage::Memory,
                CacheBackend::OpenKeyvMemory => CacheStorage::OpenKeyvMemory,
                CacheBackend::OpenKeyvRedis => CacheStorage::OpenKeyvRedis,
            });
        let redis_url = options
            .redis_url
            .clone()
            .or_else(|| app_config.cache.redis_url.clone())
            .unwrap_or_else(|| "redis://127.0.0.1/".to_string());
        let cache_store = Self::build_cache_store(&cache_storage, &redis_url, &namespace)?;

        Ok(Self {
            config_manager,
            source_mode: options.source_mode,
            runtime_config,
            cache_storage: tokio::sync::RwLock::new(cache_storage),
            redis_url: tokio::sync::RwLock::new(Some(redis_url)),
            namespace: SyncRwLock::new(namespace.clone()),
            registry: ServiceRegistry::new(),
            pool: ConnectionPool::new(),
            applied_openapi_configs: tokio::sync::RwLock::new(HashMap::new()),
            event_bus: EventBus::with_history(1000),
            cache: CacheLayerManager::new(cache_store, namespace),
        })
    }

    pub fn config_manager(&self) -> &ConfigManager {
        &self.config_manager
    }

    pub fn cache(&self) -> &CacheLayerManager {
        &self.cache
    }

    pub fn namespace(&self) -> String {
        self.namespace
            .read()
            .expect("store namespace lock poisoned")
            .clone()
    }

    pub fn source_mode(&self) -> SourceMode {
        self.source_mode
    }

    pub fn is_db_source(&self) -> bool {
        self.source_mode == SourceMode::Db
    }
}

#[cfg(test)]
mod tests;
