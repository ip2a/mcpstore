use std::collections::HashMap;
use std::sync::RwLock as SyncRwLock;

pub(crate) use crate::cache::models::OpenApiImportContextState;
pub(crate) use crate::cache::CacheLayerManager;
pub(crate) use crate::config::{ConfigManager, ServerConfig, StartupPolicy};
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
mod kernel;
mod openapi;
mod options;
pub(crate) mod payload;
mod runtime;
pub mod store_config;
pub mod swap;
mod tool_changes;
pub(crate) use kernel::{
    ControlPlane, EventBackend, ExecutionEngine, PersistenceRouter, RuntimeState, StoreKernel,
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
pub use options::{PanelRole, StoreOptions};
pub use store_config::{JsonStoreConfig, MemoryStoreConfig, RedisStoreConfig, StoreConfig};
pub use tool_changes::{ToolChangeServiceResult, ToolChangeSummary};

pub(crate) mod prelude {
    pub(crate) use crate::config_formats::{project_config, ConfigFormat};
    pub(crate) use crate::identity::{InstanceId, ScopeRef, ScopeView, ServiceInstanceKey};
    pub(crate) use crate::registry::{AgentInfo, ScopeSummary};
    pub(crate) use crate::store::payload::wrap_cache_item;
    pub(crate) use crate::store::{
        CacheHealthReport, ConfigRevision, DiscoveredPrompt, DiscoveredResource,
        DiscoveredResourceTemplate, Error, ErrorContext, Event, FailureCode, MCPStore,
        OpenApiImportContextState, Result, ScopedServiceEntry, ScopedToolEntry, ServerConfig,
        ServiceDefinition, ServiceInstance, StartupPolicy, ToolChangeServiceResult,
        ToolChangeSummary,
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
                let (store, mem) = if store_config.store_name() == "memory-test-shared" {
                    crate::cache::storage::shared_memory_cache_store_with_handle()
                } else {
                    crate::cache::storage::memory_cache_store_with_handle()
                };
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
                (store, None) // Redis EventBackend opened lazily on migration
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
        let state_panel_id = match &options.panel {
            PanelRole::ControlPanel => crate::state::CONTROL_NODE_ID.to_string(),
            PanelRole::DataPanel { panel_id } => panel_id.clone(),
        };
        let state_manager = std::sync::Arc::new(crate::state::ServiceStateManager::new(
            cache.clone(),
            event_bus.clone(),
            state_panel_id,
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
                    supervisor: Default::default(),
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
                    local_connections: tokio::sync::RwLock::new(std::collections::HashSet::new()),
                    panel_role: options.panel.clone(),
                    runtime_config,
                    sync_config_file: options.config_path.is_some(),
                    service_event_feed_started: std::sync::atomic::AtomicBool::new(false),
                    tool_call_feed_started: std::sync::atomic::AtomicBool::new(false),
                    self_weak: std::sync::OnceLock::new(),
                },
            },
        });
        // 角色内化：控制面板 setup 即挂载自愈监督器（幂等）；
        // 数据面板不挂（结构性跳过 startup probe / 健康自愈）。
        if matches!(options.panel, PanelRole::ControlPanel) {
            store.attach_control_supervisor()?;
        }
        let _ = store
            .kernel
            .runtime
            .self_weak
            .set(std::sync::Arc::downgrade(&store));
        store.spawn_service_event_feed();
        store.spawn_tool_call_request_feed();
        Ok(store)
    }

    /// 本进程的面板角色。
    pub fn panel_role(&self) -> &PanelRole {
        &self.kernel.runtime.panel_role
    }

    /// 挂载控制面板的自愈监督器（幂等）。
    /// 由 ControlPanel 调用；未挂载即无自愈行为。
    pub fn attach_control_supervisor(self: &std::sync::Arc<Self>) -> Result<()> {
        if self.kernel.execution.supervisor.get().is_some() {
            return Ok(());
        }
        let supervisor = std::sync::Arc::new(crate::health::supervisor::InstanceSupervisor::new(
            self.kernel.runtime.runtime_config.supervisor_policy,
            self.kernel.control.state.clone(),
        ));
        supervisor.attach_store(std::sync::Arc::downgrade(self));
        self.kernel
            .execution
            .pool
            .attach_supervisor(supervisor.clone());
        let _ = self.kernel.execution.supervisor.set(supervisor);
        Ok(())
    }

    /// 当前挂载的自愈监督器（可能未挂载）。
    pub(crate) fn control_supervisor(
        &self,
    ) -> Option<std::sync::Arc<crate::health::supervisor::InstanceSupervisor>> {
        self.kernel.execution.supervisor.get().cloned()
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

    pub fn panel_id(&self) -> String {
        self.kernel.control.state.panel_id().to_string()
    }

    /// Close only transports started by this process（进程生命周期收尾：
    /// 短命宿主退出时只清理自己启动的连接，不动其他节点拥有的传输）。
    pub async fn close_local_connections(&self) {
        let instance_ids: Vec<crate::identity::InstanceId> = self
            .kernel
            .runtime
            .local_connections
            .write()
            .await
            .drain()
            .collect();
        for instance_id in instance_ids {
            self.kernel.execution.pool.remove(instance_id).await.ok();
            self.kernel
                .control
                .state
                .dispatch(
                    instance_id,
                    crate::state::ServiceStateEvent::TransportStopped,
                    Self::now_timestamp(),
                )
                .await
                .ok();
        }
    }

    /// Shared ChangeFeed-capable handle over the active store. Memory shares
    /// the cache layer's store; Redis opens a handle to the same URL. Used by
    /// the service event feed and online migration.
    pub(crate) async fn ensure_event_backend(&self) -> Result<EventBackend> {
        if let Some(backend) = self.kernel.persistence.event_backend.read().await.clone() {
            return Ok(backend);
        }
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
        Ok(backend)
    }
}

#[cfg(test)]
mod role_tests;
#[cfg(test)]
mod tests;
