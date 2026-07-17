use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, RwLock as SyncRwLock};

pub(crate) use crate::cache::models::{
    HealthStatus, OpenApiImportContextState, ServiceLifecycleState, ToolAvailability,
};
pub(crate) use crate::cache::CacheLayerManager;
pub(crate) use crate::config::{CacheBackend, ConfigManager, ServerConfig, StartupPolicy};
use crate::event_reactor::{EventBackend, EventReactor, ReactorConfig, Rule};
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
        CONTROL_EVENT_SEQUENCE, CONTROL_REQUEST_EVENT_TYPE,
    };
}

pub struct MCPStore {
    pub(crate) auth_coordinator: crate::auth::AuthCoordinator,
    pub(crate) config_manager: ConfigManager,
    pub(crate) source_mode: SourceMode,
    pub(crate) runtime_config: StoreRuntimeConfig,
    pub(crate) supervisor: Option<std::sync::Arc<crate::health::supervisor::InstanceSupervisor>>,
    pub(crate) cache_storage: tokio::sync::RwLock<CacheStorage>,
    pub(crate) redis_url: tokio::sync::RwLock<Option<String>>,
    pub(crate) namespace: SyncRwLock<String>,
    pub(crate) registry: ServiceRegistry,
    pub(crate) pool: ConnectionPool,
    pub(crate) applied_openapi_configs: tokio::sync::RwLock<
        HashMap<crate::identity::InstanceId, serde_json::Map<String, serde_json::Value>>,
    >,
    pub(crate) event_bus: EventBus,
    pub(crate) cache: std::sync::Arc<CacheLayerManager>,
    pub(crate) event_reactor:
        tokio::sync::RwLock<Option<std::sync::Arc<EventReactor<EventBackend>>>>,
    /// Shared backend for EventReactor. For Memory, this shares the same
    /// `Arc<MemoryClient>` as the cache layer. For Redis, a separate connection
    /// to the same Redis server (data shared naturally).
    pub(crate) event_backend: tokio::sync::RwLock<Option<EventBackend>>,
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
        let (cache_store, event_backend) = match cache_storage {
            crate::store::CacheStorage::Memory | crate::store::CacheStorage::OpenKeyvMemory => {
                let (store, mem) = crate::cache::storage::memory_cache_store_with_handle();
                (store, Some(EventBackend::from_memory(mem)))
            }
            crate::store::CacheStorage::Redis | crate::store::CacheStorage::OpenKeyvRedis => {
                let store = Self::build_cache_store(&cache_storage, &redis_url, &namespace)?;
                (store, None) // Redis EventBackend created lazily in setup_event_reactor
            }
        };
        let auth_coordinator = crate::auth::AuthCoordinator::new()?;
        let supervisor = (options.source_mode == SourceMode::Local).then(|| {
            std::sync::Arc::new(crate::health::supervisor::InstanceSupervisor::new(
                runtime_config.supervisor_policy,
            ))
        });

        let registry = ServiceRegistry::new();
        let event_bus = EventBus::with_history(1000);
        let cache = std::sync::Arc::new(CacheLayerManager::new(cache_store, namespace.clone()));
        let pool = ConnectionPool::new(
            auth_coordinator.clone(),
            registry.clone(),
            event_bus.clone(),
            cache.clone(),
        );

        Ok(Self {
            auth_coordinator: auth_coordinator.clone(),
            config_manager,
            source_mode: options.source_mode,
            runtime_config,
            supervisor,
            cache_storage: tokio::sync::RwLock::new(cache_storage),
            redis_url: tokio::sync::RwLock::new(Some(redis_url)),
            namespace: SyncRwLock::new(namespace.clone()),
            registry,
            pool,
            applied_openapi_configs: tokio::sync::RwLock::new(HashMap::new()),
            event_bus,
            cache,
            event_reactor: tokio::sync::RwLock::new(None),
            event_backend: tokio::sync::RwLock::new(event_backend),
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

    // ── EventReactor facade ──

    /// Initialize the EventReactor using the shared event backend. For Memory,
    /// the backend was created during construction (sharing the cache layer's
    /// MemoryStore). For Redis, it connects now (async) to the same Redis URL.
    pub async fn setup_event_reactor(&self, config: ReactorConfig) -> Result<()> {
        let backend = match self.event_backend.read().await.clone() {
            Some(b) => b,
            None => {
                // Redis: construct now (was deferred because it's async).
                let storage = self.cache_storage.read().await.clone();
                match storage {
                    crate::store::CacheStorage::Redis
                    | crate::store::CacheStorage::OpenKeyvRedis => {
                        let url = self
                            .redis_url
                            .read()
                            .await
                            .clone()
                            .unwrap_or_else(|| "redis://127.0.0.1/".to_string());
                        let b = EventBackend::from_redis_url(&url)
                            .await
                            .map_err(|e| StoreError::Other(format!("redis event backend: {e}")))?;
                        *self.event_backend.write().await = Some(b.clone());
                        b
                    }
                    _ => {
                        return Err(StoreError::Other(
                            "no event backend available for this storage".into(),
                        ));
                    }
                }
            }
        };
        let reactor = std::sync::Arc::new(
            EventReactor::new(backend, config).with_event_bus(self.event_bus.clone()),
        );
        *self.event_reactor.write().await = Some(reactor);
        Ok(())
    }

    /// Register a rule with the EventReactor. Requires `setup_event_reactor`.
    pub async fn register_rule(&self, rule: Rule) -> Result<()> {
        let guard = self.event_reactor.read().await;
        let reactor = guard
            .as_ref()
            .ok_or_else(|| StoreError::Other("event reactor not initialized".into()))?;
        reactor.register(rule).await;
        Ok(())
    }

    /// Start the EventReactor feed loop. Requires `setup_event_reactor`.
    pub async fn start_reactor(&self) -> Result<()> {
        let guard = self.event_reactor.read().await;
        let reactor = guard
            .as_ref()
            .ok_or_else(|| StoreError::Other("event reactor not initialized".into()))?;
        reactor
            .start()
            .await
            .map_err(|e| StoreError::Other(format!("reactor start: {e}")))?;
        Ok(())
    }

    /// Stop the EventReactor feed loop gracefully.
    pub async fn stop_reactor(&self) {
        let guard = self.event_reactor.read().await;
        if let Some(reactor) = guard.as_ref() {
            reactor.shutdown().await;
        }
    }

    /// Check whether the EventReactor is initialized.
    pub async fn has_reactor(&self) -> bool {
        self.event_reactor.read().await.is_some()
    }
}

#[cfg(test)]
mod tests;
