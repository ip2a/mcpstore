use std::collections::HashMap;
use std::sync::RwLock as SyncRwLock;

pub(crate) use crate::cache::models::OpenApiImportContextState;
pub(crate) use crate::cache::KvLayerManager;
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
mod projection_feed;
mod runtime;
pub mod store_config;
pub mod swap;
mod tool_changes;
pub(crate) use kernel::{
    ControlPlane, EventLog, EventLogSnapshot, ExecutionEngine, PersistenceRouter,
    RuntimeState, StoreKernel,
};
use runtime::StoreRuntimeConfig;

pub use crate::agent::models::{ScopedServiceEntry, ScopedToolEntry};
pub use crate::agent::tool_visibility::ToolVisibilityFilter;
pub use crate::cache::models::KvHealthReport;
pub use crate::events::EventCapabilityReport;
pub use crate::openapi::{
    OpenApiBundleArtifact, OpenApiBundleDependency, OpenApiBundleDiagnostic, OpenApiBundleDocument,
    OpenApiImportOptions, OpenApiImportResult,
};
pub use openapi::{OpenApiImportInput, OpenApiImportSource};
pub use options::{PanelRole, StoreOptions};
pub use store_config::{
        JsonStoreConfig, MemoryStoreConfig, RedisStoreConfig, StoreSpec,
    };
    #[cfg(feature = "valkey")]
    pub use store_config::ValkeyStoreConfig;
#[cfg(feature = "shm")]
pub use store_config::ShmStoreConfig;
pub use tool_changes::{ToolChangeServiceResult, ToolChangeSummary};

pub(crate) mod prelude {
    pub(crate) use crate::config_formats::{project_config, ConfigFormat};
    pub(crate) use crate::identity::{InstanceId, ScopeRef, ScopeView, ServiceInstanceKey};
    pub(crate) use crate::registry::{AgentInfo, ScopeSummary};
    pub(crate) use crate::store::payload::wrap_cache_item;
    pub(crate) use crate::store::{
        KvHealthReport, ConfigRevision, DiscoveredPrompt, DiscoveredResource,
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
        // Data panels must use a shared store: events/requests in a process-private memory store are never
        // consumed by any control panel and just sit silently orphaned (test name memory-test-shared is exempt).
        if matches!(options.panel, PanelRole::DataPanel { .. })
            && store_config.store_name() == "memory"
        {
            return Err(Error::new(
                FailureCode::ConfigInvalid,
                "DataPanel requires a shared store (e.g. redis); a process-private memory store cannot be consumed by any control panel".to_string(),
            ));
        }
        // Seed/flush rules: explicit config_path, or a memory store (an in-process store has no other
        // persistence layer, the file is it — default single-machine changes must be flushed to disk).
        // Shared store + no explicit path = never read or write any local file, so a personal default file can't
        // override the shared store.
        // Note: decide by the raw store name; "memory" after memory-test-shared remapping doesn't count.
        let file_backed = options.config_path.is_some() || store_config.store_name() == "memory";
        #[cfg(any(test, feature = "test-shared-memory"))]
        let store_name = if store_config.store_name() == "memory-test-shared" {
            "memory".to_string()
        } else {
            store_name
        };
        #[cfg(not(any(test, feature = "test-shared-memory")))]
        let store_name = store_name;
        let (kv_store, event_log) = match store_name.as_str() {
            "memory" => {
                let (store, mem) = if store_config.store_name() == "memory-test-shared" {
                    crate::cache::storage::shared_memory_kv_store_with_handle()
                } else {
                    crate::cache::storage::memory_kv_store_with_handle()
                };
                let handle = openkeyv::StoreHandle::with_capabilities(
                    std::sync::Arc::new(mem.clone()),
                    Some(std::sync::Arc::new(mem.clone())),
                    Some(std::sync::Arc::new(mem.clone())),
                    Some(std::sync::Arc::new(mem.clone())),
                    Some(std::sync::Arc::new(mem)),
                );
                (store, Some(EventLog::from_store(handle)))
            }
            "redis" => {
                let store = Self::build_kv_store(&store_config, &redis_url, &namespace)?;
                (store, None) // Redis EventLog opened lazily on migration
            }
            "valkey" => {
                let store = Self::build_kv_store(&store_config, &redis_url, &namespace)?;
                (store, None) // Valkey EventLog opened lazily on migration
            }
            #[cfg(all(feature = "shm", unix))]
            "shm" => {
                let (store, handle) =
                    crate::cache::storage::shm_kv_store_with_handle(&store_config.config)
                        .map_err(|e| Error::new(FailureCode::Internal, format!("shm store: {e}")))?;
                (store, Some(EventLog::from_store(handle)))
            }
            unsupported => {
                return Err(Error::new(FailureCode::Internal, format!(
                    "OpenKeyv store '{unsupported}' does not provide the CAS and ChangeFeed capabilities required by MCPStore"
                )))
            }
        };
        let registry = ServiceRegistry::new();
        let event_bus = EventBus::with_history(10_000);
        let cache = std::sync::Arc::new(KvLayerManager::new(kv_store, namespace.clone()));
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
                    refresh_mutex: tokio::sync::Mutex::new(()),
                },
                execution: ExecutionEngine {
                    pool,
                    supervisor: Default::default(),
                    event_bus: event_bus.clone(),
                    telemetry: std::sync::OnceLock::new(),
                },
                persistence: PersistenceRouter {
                    store_config: tokio::sync::RwLock::new(store_config),
                    cache,
                    event_log: tokio::sync::RwLock::new(event_log),
                    event_epoch: tokio::sync::watch::channel(0u64).0,
                },
                runtime: RuntimeState {
                    namespace: SyncRwLock::new(namespace),
                    applied_openapi_configs: tokio::sync::RwLock::new(HashMap::new()),
                    local_connections: tokio::sync::RwLock::new(std::collections::HashSet::new()),
                    panel_role: options.panel.clone(),
                    panel_placement: app_config.placement.clone(),
                    runtime_config,
                    sync_config_file: file_backed,
                    service_event_feed_started: std::sync::atomic::AtomicBool::new(false),
                    service_event_feed_failed: std::sync::atomic::AtomicBool::new(false),
                    tool_call_feed_started: std::sync::atomic::AtomicBool::new(false),
                    self_weak: std::sync::OnceLock::new(),
                },
            },
        });
        // Roles are internalized: the control panel mounts the self-healing supervisor at setup (idempotent);
        // the data panel doesn't (structurally skips startup probe / health self-healing).
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

    /// This process's panel role.
    pub fn panel_role(&self) -> &PanelRole {
        &self.kernel.runtime.panel_role
    }

    /// Store configuration (for gateway spawn inheritance).
    pub async fn store_config(&self) -> JsonStoreConfig {
        self.kernel.persistence.store_config.read().await.clone()
    }

    /// Attach the request-telemetry sink (host opts in; SDK default = off).
    /// Idempotent: the first attach wins.
    pub fn set_telemetry(
        &self,
        telemetry: Option<std::sync::Arc<dyn crate::telemetry::RequestTelemetry>>,
    ) {
        let _ = self.kernel.execution.telemetry.set(telemetry);
    }

    /// The attached telemetry sink, if any.
    pub fn telemetry(&self) -> Option<std::sync::Arc<dyn crate::telemetry::RequestTelemetry>> {
        self.kernel
            .execution
            .telemetry
            .get()
            .and_then(|slot| slot.clone())
    }

    /// Attach the frame-telemetry sink (host opts in; SDK default = off).
    /// Idempotent: the first attach wins. Read lazily by upstream connections
    /// at connect time.
    pub fn set_frame_telemetry(
        &self,
        sink: Option<std::sync::Arc<dyn crate::telemetry::FrameTelemetry>>,
    ) {
        self.kernel.execution.pool.attach_frame_telemetry(sink);
    }

    /// The attached frame-telemetry sink, if any.
    pub fn frame_telemetry(&self) -> Option<std::sync::Arc<dyn crate::telemetry::FrameTelemetry>> {
        self.kernel.execution.pool.frame_telemetry()
    }

    /// Mount the control panel's self-healing supervisor (idempotent).
    /// Called by ControlPanel; not mounted means no self-healing.
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

    /// The currently mounted self-healing supervisor (may be unmounted).
    #[cfg(test)]
    pub(crate) fn control_supervisor(
        &self,
    ) -> Option<std::sync::Arc<crate::health::supervisor::InstanceSupervisor>> {
        self.kernel.execution.supervisor.get().cloned()
    }

    pub fn config_manager(&self) -> &ConfigManager {
        &self.kernel.control.config_manager
    }

    pub fn cache(&self) -> &KvLayerManager {
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

    /// Close only transports started by this process (process-lifetime teardown:
    /// short-lived hosts clean up only the connections they started, leaving other nodes' transports alone).
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

    /// Resolve the event log together with the epoch generation it belongs
    /// to. The subscribe → read → resolve → re-read loop guarantees a cutover
    /// between two steps is noticed: the caller either gets a log that is
    /// still current, or the retry starts over on the new generation.
    pub(crate) async fn event_log_snapshot(&self) -> Result<EventLogSnapshot> {
        let persistence = &self.kernel.persistence;
        loop {
            let mut changes = persistence.event_epoch.subscribe();
            let generation = *changes.borrow_and_update();
            let log = self.ensure_event_log().await?;
            if *changes.borrow_and_update() == generation {
                return Ok(EventLogSnapshot {
                    log,
                    generation,
                    changes,
                });
            }
        }
    }

    /// Shared ChangeFeed-capable handle over the active store. Memory shares
    /// the cache layer's store; Redis opens a handle to the same URL. Used by
    /// the service event feed and online migration.
    pub(crate) async fn ensure_event_log(&self) -> Result<EventLog> {
        if let Some(log) = self.kernel.persistence.event_log.read().await.clone() {
            return Ok(log);
        }
        let log = {
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
                    EventLog::from_store(handle)
                }
                "valkey" => {
                    let url = storage
                        .config
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("redis://127.0.0.1/");
                    let handle = openkeyv::factory::open_store(openkeyv::StoreConfig::new(
                        "valkey",
                        serde_json::json!({
                            "url": url,
                            "keyspace": self.namespace(),
                        }),
                    ))
                    .await
                    .map_err(|e| Error::new(FailureCode::Internal, format!("event Store: {e}")))?;
                    EventLog::from_store(handle)
                }
                unsupported => {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("OpenKeyv store '{unsupported}' does not provide ChangeFeed support"),
                    ));
                }
            }
        };
        *self.kernel.persistence.event_log.write().await = Some(log.clone());
        Ok(log)
    }
}

#[cfg(test)]
#[cfg(test)]
mod role_tests;
#[cfg(test)]
mod tests;

/// Serializes tests that share the process-global `memory-test-shared` store. Concurrent
/// async tests contending on that single store can deadlock (hold-and-wait across their
/// per-test runtimes), which surfaced as hangs in `cargo test --workspace` while
/// `cargo test -p mcpstore --lib` stayed green.
#[cfg(test)]
pub(crate) fn shared_store_test_guard() -> &'static tokio::sync::Mutex<()> {
    static GUARD: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    GUARD.get_or_init(|| tokio::sync::Mutex::new(()))
}
