use std::collections::HashMap;
use std::sync::RwLock as SyncRwLock;

pub(crate) use crate::cache::models::OpenApiImportContextState;
pub(crate) use crate::cache::CacheLayerManager;
pub(crate) use crate::config::{ConfigManager, ServerConfig, StartupPolicy};
use crate::event_reactor::{EventBackend, EventReactor, ReactorConfig, Rule};
pub(crate) use crate::events::{Event, EventBus};
pub(crate) use crate::registry::{
    ConfigRevision, ServiceDefinition, ServiceInstance, ServiceRegistry,
};
pub(crate) use crate::transport::client::ConnectionPool;
pub(crate) use crate::transport::{
    DiscoveredPrompt, DiscoveredResource, DiscoveredResourceTemplate,
};

pub(crate) use crate::error::{Error, ErrorContext, FailureCode, Result};

mod control_facade;
mod control_requests;
mod kernel;
mod openapi;
mod options;
pub(crate) mod payload;
mod runtime;
pub mod store_config;
pub mod swap;
mod tool_changes;
pub(crate) use kernel::{
    ControlPlane, ExecutionEngine, PersistenceRouter, RuntimeState, StoreKernel,
};
use runtime::StoreRuntimeConfig;

pub use crate::agent::models::{ScopedServiceEntry, ScopedToolEntry};
pub use crate::agent::tool_visibility::ToolVisibilityFilter;
pub use crate::cache::models::CacheHealthReport;
pub use crate::events::EventCapabilityReport;
pub use crate::openapi::{
    OpenApiBundleArtifact, OpenApiBundleDependency, OpenApiBundleDiagnostic, OpenApiBundleDocument,
    OpenApiImportOptions, OpenApiImportResult,
};
pub use openapi::{OpenApiImportInput, OpenApiImportSource};
pub use options::{NodeMode, SourceMode, StoreOptions};
pub use store_config::{JsonStoreConfig, MemoryStoreConfig, RedisStoreConfig, StoreConfig};
pub use tool_changes::{ToolChangeServiceResult, ToolChangeSummary};

pub const CONTROL_REQUEST_EVENT_TYPE: &str = "control_requests";

pub(crate) mod prelude {
    pub(crate) use crate::config_formats::{project_config, ConfigFormat};
    pub(crate) use crate::identity::{InstanceId, ScopeRef, ScopeView, ServiceInstanceKey};
    pub(crate) use crate::registry::{AgentInfo, ScopeSummary};
    pub(crate) use crate::store::payload::wrap_cache_item;
    pub(crate) use crate::store::{
        CacheHealthReport, ConfigRevision, DiscoveredPrompt, DiscoveredResource,
        DiscoveredResourceTemplate, Error, ErrorContext, Event, FailureCode, MCPStore,
        OpenApiImportContextState, Result, ScopedServiceEntry, ScopedToolEntry, ServerConfig,
        ServiceDefinition, ServiceInstance, SourceMode, StartupPolicy, ToolChangeServiceResult,
        ToolChangeSummary, CONTROL_REQUEST_EVENT_TYPE,
    };
}

pub struct MCPStore {
    pub(crate) kernel: StoreKernel,
}

impl MCPStore {
    pub fn setup(config_path: Option<&str>) -> Result<std::sync::Arc<Self>> {
        Self::setup_with_options(StoreOptions {
            config_path: config_path.map(ToString::to_string),
            ..StoreOptions::default()
        })
    }

    pub fn setup_with_options(options: StoreOptions) -> Result<std::sync::Arc<Self>> {
        // Local source + DataPlane is an invalid combination: queued control
        // requests would be written to a local in-process store that no other
        // node can consume. The user almost certainly meant ControlPlane.
        if options.node_mode == NodeMode::DataPlane
            && (options.source_mode == SourceMode::Local
                || options
                    .store
                    .as_ref()
                    .is_some_and(|store| store.store_name() == "memory"))
        {
            return Err(Error::new(
                FailureCode::Internal,
                concat!(
                    "DataPlane requires a shared persistent store (Redis/Valkey); ",
                    "Local source and in-process memory cannot be used."
                )
                .to_string(),
            ));
        }

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
        let mut store_config = options.store.clone().unwrap_or_else(|| {
            JsonStoreConfig::new(
                app_config.cache.store.as_str(),
                app_config.cache.config.clone(),
            )
        });
        #[cfg(test)]
        let store_name = if store_config.store_name() == "memory-test-shared" {
            "memory".to_string()
        } else {
            store_config.store_name().to_string()
        };
        #[cfg(not(test))]
        let store_name = store_config.store_name().to_string();
        if matches!(store_name.as_str(), "redis" | "valkey") {
            store_config.config["keyspace"] = serde_json::Value::String(namespace.clone());
        }
        let redis_url = store_config
            .config
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("redis://127.0.0.1/")
            .to_string();
        #[cfg(any(test, feature = "test-shared-memory"))]
        let store_name = if store_config.store_name() == "memory-test-shared" {
            "memory".to_string()
        } else {
            store_name
        };
        #[cfg(not(any(test, feature = "test-shared-memory")))]
        let store_name = store_name;
        let (cache_store, event_backend) = match store_name.as_str() {
            "memory" => {
                let (store, mem) = crate::cache::storage::memory_cache_store_with_handle();
                let handle = openkeyv::StoreHandle::with_capabilities(
                    std::sync::Arc::new(mem.clone()),
                    Some(std::sync::Arc::new(mem.clone())),
                    Some(std::sync::Arc::new(mem.clone())),
                    Some(std::sync::Arc::new(mem.clone())),
                    Some(std::sync::Arc::new(mem)),
                );
                (store, Some(EventBackend::from_store(handle)))
            }
            "redis" => {
                let store = Self::build_cache_store(&store_config, &redis_url, &namespace)?;
                (store, None) // Redis EventBackend created lazily in setup_event_reactor
            }
            backend => {
                return Err(Error::new(FailureCode::Internal, format!(
                    "OpenKeyv backend '{backend}' does not provide the CAS and ChangeFeed capabilities required by MCPStore"
                )))
            }
        };
        let registry = ServiceRegistry::new();
        let event_bus = EventBus::with_history(10_000);
        let cache = std::sync::Arc::new(CacheLayerManager::new(cache_store, namespace.clone()));
        let state_manager = std::sync::Arc::new(crate::state::ServiceStateManager::new(
            cache.clone(),
            event_bus.clone(),
        ));
        #[cfg(not(test))]
        let auth_coordinator = crate::auth::AuthCoordinator::new(state_manager.clone())?;
        #[cfg(test)]
        let auth_coordinator = crate::auth::AuthCoordinator::for_tests(
            crate::auth::test_support::test_keyring(),
            state_manager.clone(),
        )?;
        let pool = ConnectionPool::new(
            auth_coordinator.clone(),
            registry.clone(),
            event_bus.clone(),
            cache.clone(),
        );
        let supervisor = (options.node_mode == NodeMode::ControlPlane).then(|| {
            std::sync::Arc::new(crate::health::supervisor::InstanceSupervisor::new(
                runtime_config.supervisor_policy,
                state_manager.clone(),
            ))
        });
        if let Some(supervisor) = &supervisor {
            pool.attach_supervisor(supervisor.clone());
        }

        let store = std::sync::Arc::new(Self {
            kernel: StoreKernel {
                control: ControlPlane {
                    config_manager,
                    registry,
                    auth: auth_coordinator.clone(),
                    state: state_manager,
                },
                execution: ExecutionEngine {
                    pool,
                    supervisor,
                    event_bus: event_bus.clone(),
                },
                persistence: PersistenceRouter {
                    store_config: tokio::sync::RwLock::new(store_config),
                    cache,
                    event_backend: tokio::sync::RwLock::new(event_backend),
                },
                runtime: RuntimeState {
                    namespace: SyncRwLock::new(namespace),
                    applied_openapi_configs: tokio::sync::RwLock::new(HashMap::new()),
                    event_reactor: tokio::sync::RwLock::new(None),
                    source_mode: options.source_mode,
                    node_mode: options.node_mode,
                    runtime_config,
                },
            },
        });
        if let Some(supervisor) = &store.kernel.execution.supervisor {
            supervisor.attach_store(std::sync::Arc::downgrade(&store));
        }
        Ok(store)
    }

    pub fn config_manager(&self) -> &ConfigManager {
        &self.kernel.control.config_manager
    }

    pub fn cache(&self) -> &CacheLayerManager {
        &self.kernel.persistence.cache
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.kernel.execution.event_bus
    }

    pub fn namespace(&self) -> String {
        self.kernel
            .runtime
            .namespace
            .read()
            .expect("store namespace lock poisoned")
            .clone()
    }

    pub fn source_mode(&self) -> SourceMode {
        self.kernel.runtime.source_mode
    }

    pub fn node_mode(&self) -> NodeMode {
        self.kernel.runtime.node_mode
    }

    pub fn is_data_plane(&self) -> bool {
        self.kernel.runtime.node_mode == NodeMode::DataPlane
    }

    pub fn is_db_source(&self) -> bool {
        self.kernel.runtime.source_mode == SourceMode::Db
    }

    // ── EventReactor facade ──

    /// Initialize the EventReactor using the shared event backend. For Memory,
    /// the backend was created during construction (sharing the cache layer's
    /// MemoryStore). For Redis, it connects now (async) to the same Redis URL.
    pub async fn setup_event_reactor(&self, config: ReactorConfig) -> Result<()> {
        // Fast path: backend already initialized. Drop the read guard before
        // potentially taking the write guard below to avoid RwLock upgrade deadlock.
        if let Some(b) = self.kernel.persistence.event_backend.read().await.clone() {
            let reactor = std::sync::Arc::new(
                EventReactor::new(b, config)
                    .with_event_bus(self.kernel.execution.event_bus.clone()),
            );
            *self.kernel.runtime.event_reactor.write().await = Some(reactor);
            return Ok(());
        }

        // Slow path: build the backend (Redis needs async connect), then write.
        let backend = {
            let storage = self.kernel.persistence.store_config.read().await;
            match storage.store_name() {
                "redis" => {
                    let url = storage
                        .config
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("redis://127.0.0.1/");
                    let handle = openkeyv::factory::open_store(openkeyv::StoreConfig::redis(
                        serde_json::json!({
                            "url": url,
                            "keyspace": self.namespace(),
                        }),
                    ))
                    .await
                    .map_err(|e| Error::new(FailureCode::Internal, format!("event Store: {e}")))?;
                    EventBackend::from_store(handle)
                }
                backend => {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("OpenKeyv backend '{backend}' does not provide ChangeFeed support"),
                    ));
                }
            }
        };
        *self.kernel.persistence.event_backend.write().await = Some(backend.clone());

        let reactor = std::sync::Arc::new(
            EventReactor::new(backend, config)
                .with_event_bus(self.kernel.execution.event_bus.clone()),
        );
        *self.kernel.runtime.event_reactor.write().await = Some(reactor);
        Ok(())
    }

    /// Register a rule with the EventReactor. Requires `setup_event_reactor`.
    pub async fn register_rule(&self, rule: Rule) -> Result<()> {
        let guard = self.kernel.runtime.event_reactor.read().await;
        let reactor = guard
            .as_ref()
            .ok_or_else(|| Error::new(FailureCode::Internal, "event reactor not initialized"))?;
        reactor.register(rule).await;
        Ok(())
    }

    /// Start the EventReactor feed loop. Requires `setup_event_reactor`.
    pub async fn start_reactor(&self) -> Result<()> {
        let guard = self.kernel.runtime.event_reactor.read().await;
        let reactor = guard
            .as_ref()
            .ok_or_else(|| Error::new(FailureCode::Internal, "event reactor not initialized"))?;
        reactor
            .start()
            .await
            .map_err(|e| Error::new(FailureCode::Internal, format!("reactor start: {e}")))?;
        Ok(())
    }

    /// Stop the EventReactor feed loop gracefully.
    pub async fn stop_reactor(&self) {
        let guard = self.kernel.runtime.event_reactor.read().await;
        if let Some(reactor) = guard.as_ref() {
            reactor.shutdown().await;
        }
        if let Some(supervisor) = &self.kernel.execution.supervisor {
            supervisor.shutdown().await;
        }
    }

    /// Check whether the EventReactor is initialized.
    pub async fn has_reactor(&self) -> bool {
        self.kernel.runtime.event_reactor.read().await.is_some()
    }
}

#[cfg(test)]
mod tests;
