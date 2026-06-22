use std::sync::{atomic::AtomicU64, RwLock as SyncRwLock};

pub(crate) use crate::cache::models::{
    AgentServiceRelation, HealthStatus, ServiceEntity, ServiceRelationItem, ServiceStatus,
    ServiceToolRelation, ToolAvailability, ToolEntity, ToolRelationItem, ToolStatusItem,
};
pub(crate) use crate::cache::CacheLayerManager;
pub(crate) use crate::config::{CacheBackend, ConfigManager, ServerConfig};
pub(crate) use crate::events::{Event, EventBus};
pub(crate) use crate::registry::{ConnectionStatus, ServiceEntry, ServiceRegistry};
pub(crate) use crate::transport::client::ConnectionPool;
pub(crate) use crate::transport::ToolDescription;

pub(crate) use crate::perspective::{
    generate_tool_global_name, normalize_service_name, parse_agent_scoped, resolve_tool,
    AvailableTool, ToolResolution, GLOBAL_AGENT_STORE,
};
pub(crate) use crate::{Result, StoreError};

mod options;
pub(crate) mod payload;
mod runtime;
use runtime::StoreRuntimeConfig;

pub use crate::agent::models::{ScopedServiceEntry, ScopedServiceHealth, ScopedToolEntry};
pub use crate::cache::models::CacheHealthReport;
pub use crate::events::EventCapabilityReport;
pub use options::{BackendKind, CacheStorage, SourceMode, StoreOptions};

pub(crate) const CONTROL_REQUEST_EVENT_TYPE: &str = "control_requests";
pub(crate) static CONTROL_EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) mod prelude {
    pub(crate) use crate::store::payload::wrap_cache_item;
    pub(crate) use crate::store::{
        generate_tool_global_name, normalize_service_name, parse_agent_scoped, resolve_tool,
        AgentServiceRelation, AvailableTool, BackendKind, CacheHealthReport, CacheStorage,
        ConnectionStatus, Event, EventCapabilityReport, HealthStatus, MCPStore, Result,
        ScopedServiceEntry, ScopedServiceHealth, ScopedToolEntry, ServerConfig, ServiceEntity,
        ServiceEntry, ServiceRelationItem, ServiceStatus, ServiceToolRelation, SourceMode,
        StoreError, ToolAvailability, ToolDescription, ToolEntity, ToolRelationItem,
        ToolResolution, ToolStatusItem, CONTROL_EVENT_SEQUENCE, CONTROL_REQUEST_EVENT_TYPE,
        GLOBAL_AGENT_STORE,
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
        let cache_store = Self::build_backend(&cache_storage, &redis_url, &namespace)?;

        Ok(Self {
            config_manager,
            source_mode: options.source_mode,
            runtime_config,
            cache_storage: tokio::sync::RwLock::new(cache_storage),
            redis_url: tokio::sync::RwLock::new(Some(redis_url)),
            namespace: SyncRwLock::new(namespace.clone()),
            registry: ServiceRegistry::new(),
            pool: ConnectionPool::new(),
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
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn temp_config_path() -> String {
        std::env::temp_dir()
            .join(format!("mcpstore-store-{}.json", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .to_string()
    }

    fn stdio_config() -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("echo".to_string()),
            args: vec!["fixture".to_string()],
            env: HashMap::new(),
            headers: HashMap::new(),
            transport: Some("stdio".to_string()),
            working_dir: None,
            description: Some("fixture".to_string()),
        }
    }

    fn broken_stdio_config() -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("__mcpstore_missing_binary__".to_string()),
            args: Vec::new(),
            env: HashMap::new(),
            headers: HashMap::new(),
            transport: Some("stdio".to_string()),
            working_dir: None,
            description: Some("broken".to_string()),
        }
    }

    #[tokio::test]
    async fn add_service_writes_cache_layers() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let entity = store.cache().get_entity("services", "svc").await.unwrap();
        assert!(entity.is_some());

        let relation = store
            .cache()
            .get_relation("agent_services", "global_agent_store")
            .await
            .unwrap();
        assert!(relation.is_some());

        let status = store
            .cache()
            .get_state("service_status", "svc")
            .await
            .unwrap();
        assert!(status.is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn add_service_for_agent_uses_global_identity() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        let global_name = store
            .add_service_for_agent("agent-a", "svc", stdio_config())
            .await
            .unwrap();

        assert_eq!(global_name, "svc_byagent_agent-a");
        let entry = store.find_service(&global_name).await.unwrap();
        assert_eq!(entry.original_name, "svc");
        assert_eq!(entry.agent_id, "agent-a");
        assert_eq!(
            store.list_agent_service_names("agent-a").await.unwrap(),
            vec![global_name.clone()]
        );

        let entity = store
            .cache()
            .get_entity("services", &global_name)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(entity["service_original_name"], "svc");
        assert_eq!(entity["source_agent"], "agent-a");

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn remove_service_clears_service_cache() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();
        store.remove_service("svc").await.unwrap();

        let entity = store.cache().get_entity("services", "svc").await.unwrap();
        assert!(entity.is_none());

        let status = store
            .cache()
            .get_state("service_status", "svc")
            .await
            .unwrap();
        assert!(status.is_none());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn db_source_does_not_write_config_file() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source".to_string()),
        })
        .unwrap();

        store.add_service("svc", stdio_config()).await.unwrap();

        assert!(!std::path::Path::new(&path).exists());
        assert!(store
            .cache()
            .get_entity("services", "svc")
            .await
            .unwrap()
            .is_none());
        let events = store
            .cache()
            .get_all_events_async(CONTROL_REQUEST_EVENT_TYPE)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        let event = events.values().next().unwrap();
        assert_eq!(event["type"], serde_json::json!("ServiceAddRequested"));
        assert_eq!(event["status"], serde_json::json!("pending"));
        assert_eq!(event["payload"]["service_name"], serde_json::json!("svc"));
    }

    #[tokio::test]
    async fn db_source_refreshes_cache_on_scoped_reads() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source-refresh".to_string()),
        })
        .unwrap();
        let config = stdio_config();

        assert!(store.list_services_scoped(None).await.unwrap().is_empty());
        store
            .cache()
            .put_entity(
                "services",
                "svc",
                serde_json::to_value(ServiceEntity {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    config: serde_json::to_value(config).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_entity(
                "tools",
                "svc_echo",
                serde_json::to_value(ToolEntity {
                    tool_global_name: "svc_echo".to_string(),
                    tool_original_name: "echo".to_string(),
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    description: "echo tool".to_string(),
                    input_schema: serde_json::json!({"type": "object"}),
                    created_time: 111,
                    tool_hash: "fixture".to_string(),
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_relation(
                "service_tools",
                "svc",
                serde_json::to_value(ServiceToolRelation {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    tools: vec![ToolRelationItem {
                        tool_global_name: "svc_echo".to_string(),
                        tool_original_name: "echo".to_string(),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();

        let services = store.list_services_scoped(None).await.unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0]["name"], serde_json::json!("svc"));
        let tools = store.list_tools_scoped(None, None).await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], serde_json::json!("svc_echo"));

        store
            .cache()
            .delete_entity("services", "svc")
            .await
            .unwrap();
        let services = store.list_services_scoped(None).await.unwrap();
        assert!(services.is_empty());
    }

    #[tokio::test]
    async fn db_source_refreshes_public_reads_and_show_config() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source-public-reads".to_string()),
        })
        .unwrap();
        let config = stdio_config();

        assert!(store.list_services().await.is_empty());
        assert!(store.find_service("svc").await.is_none());
        assert!(store.list_all_tools().await.is_empty());

        store
            .cache()
            .put_entity(
                "services",
                "svc",
                serde_json::to_value(ServiceEntity {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    config: serde_json::to_value(config.clone()).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_entity(
                "tools",
                "svc_echo",
                serde_json::to_value(ToolEntity {
                    tool_global_name: "svc_echo".to_string(),
                    tool_original_name: "echo".to_string(),
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    description: "echo tool".to_string(),
                    input_schema: serde_json::json!({"type": "object"}),
                    created_time: 111,
                    tool_hash: "fixture".to_string(),
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_relation(
                "service_tools",
                "svc",
                serde_json::to_value(ServiceToolRelation {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    tools: vec![ToolRelationItem {
                        tool_global_name: "svc_echo".to_string(),
                        tool_original_name: "echo".to_string(),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_relation(
                "agent_services",
                "agent-a",
                serde_json::to_value(AgentServiceRelation {
                    services: vec![ServiceRelationItem {
                        service_global_name: "svc".to_string(),
                        service_original_name: "svc".to_string(),
                        established_time: 111,
                        last_access: Some(111),
                        client_id: "svc".to_string(),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_state(
                "service_status",
                "svc",
                serde_json::to_value(store.service_status_payload(
                    "svc",
                    HealthStatus::Healthy,
                    None,
                    vec![ToolStatusItem {
                        tool_global_name: "svc_echo".to_string(),
                        tool_original_name: "echo".to_string(),
                        status: ToolAvailability::Available,
                    }],
                ))
                .unwrap(),
            )
            .await
            .unwrap();

        let services = store.list_services().await;
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, "svc");
        assert_eq!(services[0].status, ConnectionStatus::Connected);

        let service = store.find_service("svc").await.unwrap();
        assert_eq!(service.original_name, "svc");
        assert_eq!(service.status, ConnectionStatus::Connected);

        let tools = store.list_all_tools().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");

        let scoped_services = store.list_services_scoped(None).await.unwrap();
        assert_eq!(scoped_services[0]["status"], serde_json::json!("connected"));
        let scoped_service = store.service_info_scoped(None, "svc").await.unwrap();
        assert_eq!(scoped_service["status"], serde_json::json!("connected"));

        let config_view = store.show_config().await.unwrap();
        assert_eq!(
            config_view["mcpServers"]["svc"]["args"],
            serde_json::json!(["fixture"])
        );
        assert_eq!(config_view["agents"]["agent-a"], serde_json::json!(["svc"]));
        assert!(config_view.get("global_agent_store").is_none());

        store
            .cache()
            .delete_entity("services", "svc")
            .await
            .unwrap();
        assert!(store.find_service("svc").await.is_none());
        assert!(store.list_services().await.is_empty());
        assert!(store.list_all_tools().await.is_empty());
    }

    #[tokio::test]
    async fn db_source_queues_control_requests_for_mutation_variants() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source-queue-variants".to_string()),
        })
        .unwrap();

        let global_name = store
            .add_service_for_agent("agent-a", "svc", stdio_config())
            .await
            .unwrap();
        assert_eq!(global_name, "svc_byagent_agent-a");

        let mut updated = stdio_config();
        updated.args = vec!["updated".to_string()];
        store
            .update_service("svc-update", updated.clone())
            .await
            .unwrap();
        store
            .patch_service("svc-patch", serde_json::json!({"description": "patched"}))
            .await
            .unwrap();
        store.remove_service("svc-remove").await.unwrap();
        store
            .assign_service_to_agent("agent-a", "svc-assign")
            .await
            .unwrap();
        store
            .unassign_service_from_agent("agent-a", "svc-unassign")
            .await
            .unwrap();
        store.connect_service("svc-connect").await.unwrap();
        store.disconnect_service("svc-disconnect").await.unwrap();
        store.restart_service("svc-restart").await.unwrap();
        store.reset_config().await.unwrap();

        let events = store
            .cache()
            .get_all_events_async(CONTROL_REQUEST_EVENT_TYPE)
            .await
            .unwrap();
        assert_eq!(events.len(), 10);

        let queued_add = events
            .values()
            .find(|event| {
                event["type"] == serde_json::json!("ServiceAddRequested")
                    && event["payload"]["service_name"] == serde_json::json!("svc_byagent_agent-a")
            })
            .unwrap();
        assert_eq!(
            queued_add["payload"]["service_original_name"],
            serde_json::json!("svc")
        );
        assert_eq!(
            queued_add["payload"]["agent_id"],
            serde_json::json!("agent-a")
        );
        assert_eq!(queued_add["status"], serde_json::json!("pending"));

        let queued_update = events
            .values()
            .find(|event| {
                event["type"] == serde_json::json!("ServiceUpdateRequested")
                    && event["payload"]["service_name"] == serde_json::json!("svc-update")
            })
            .unwrap();
        assert_eq!(
            queued_update["payload"]["config"]["args"],
            serde_json::json!(["updated"])
        );

        let expected = vec![
            ("ServicePatchRequested", Some("svc-patch")),
            ("ServiceRemoveRequested", Some("svc-remove")),
            ("ServiceAssignRequested", Some("svc-assign")),
            ("ServiceUnassignRequested", Some("svc-unassign")),
            ("ServiceConnectRequested", Some("svc-connect")),
            ("ServiceDisconnectRequested", Some("svc-disconnect")),
            ("ServiceRestartRequested", Some("svc-restart")),
            ("StoreResetRequested", None),
        ];
        for (event_type, service_name) in expected {
            let event = events
                .values()
                .find(|event| {
                    if event["type"] != serde_json::json!(event_type) {
                        return false;
                    }
                    match service_name {
                        Some(name) => event["payload"]["service_name"] == serde_json::json!(name),
                        None => true,
                    }
                })
                .unwrap();
            assert_eq!(event["status"], serde_json::json!("pending"));
            assert_eq!(event["source"], serde_json::json!("onlydb"));
        }

        assert!(!std::path::Path::new(&path).exists());
        assert!(store
            .cache()
            .get_entity("services", "svc-update")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_relation("agent_services", "agent-a")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn db_source_runtime_use_does_not_write_shared_runtime_state() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source-runtime-read-only".to_string()),
        })
        .unwrap();

        store
            .cache()
            .put_entity(
                "services",
                "svc",
                serde_json::to_value(ServiceEntity {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    config: serde_json::to_value(stdio_config()).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store.list_services().await;

        let cached_before = store.service_status_payload(
            "svc",
            HealthStatus::Disconnected,
            Some("main node state".to_string()),
            Vec::new(),
        );
        store
            .cache()
            .put_state(
                "service_status",
                "svc",
                serde_json::to_value(cached_before.clone()).unwrap(),
            )
            .await
            .unwrap();

        let healthy = store
            .record_health_check_result("svc", true, Some(1.0), None)
            .await
            .unwrap();
        assert_eq!(healthy.health_status, HealthStatus::Healthy);

        let failed = store
            .mark_service_retryable_failure("svc", "control request local failure".to_string())
            .await
            .unwrap();
        assert!(matches!(
            failed.health_status,
            HealthStatus::CircuitOpen | HealthStatus::Disconnected
        ));

        store
            .cache_service_connected(
                "svc",
                &[crate::registry::ToolInfo {
                    name: "echo".to_string(),
                    description: "echo".to_string(),
                    schema: serde_json::json!({"type": "object"}),
                }],
            )
            .await
            .unwrap();

        let cached_after: ServiceStatus = serde_json::from_value(
            store
                .cache()
                .get_state("service_status", "svc")
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(cached_after, cached_before);
        assert!(store
            .cache()
            .get_entity("tools", "svc_echo")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_relation("service_tools", "svc")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_all_events_async("service")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn local_source_processes_control_requests() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-control-request-worker".to_string()),
        })
        .unwrap();
        store
            .cache()
            .put_event(
                CONTROL_REQUEST_EVENT_TYPE,
                "evt-add",
                serde_json::json!({
                    "id": "evt-add",
                    "type": "ServiceAddRequested",
                    "payload": {
                        "service_name": "queued",
                        "service_original_name": "queued",
                        "agent_id": GLOBAL_AGENT_STORE,
                        "config": stdio_config(),
                    },
                    "source": "onlydb",
                    "created_at": 111,
                    "dedup_key": "ServiceAddRequested:global_agent_store:queued",
                    "trace_id": "evt-add",
                    "status": "pending",
                }),
            )
            .await
            .unwrap();

        let processed = store.process_control_requests().await.unwrap();
        assert_eq!(processed, 1);
        assert!(store
            .cache()
            .get_entity("services", "queued")
            .await
            .unwrap()
            .is_some());
        let event = store
            .cache()
            .get_event(CONTROL_REQUEST_EVENT_TYPE, "evt-add")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event["status"], serde_json::json!("completed"));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn global_relation_keeps_multiple_services() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc-a", stdio_config()).await.unwrap();
        store.add_service("svc-b", stdio_config()).await.unwrap();

        let relation = store
            .cache()
            .get_relation("agent_services", "global_agent_store")
            .await
            .unwrap()
            .unwrap();
        let relation: AgentServiceRelation = serde_json::from_value(relation).unwrap();
        let names: Vec<String> = relation
            .services
            .into_iter()
            .map(|item| item.service_global_name)
            .collect();
        assert!(names.contains(&"svc-a".to_string()));
        assert!(names.contains(&"svc-b".to_string()));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn switch_cache_storage_migrates_runtime_cache() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();
        store
            .assign_service_to_agent("agent-a", "svc")
            .await
            .unwrap();

        let snapshot = store
            .switch_cache_storage(CacheStorage::Memory, None, None)
            .await
            .unwrap();
        assert!(snapshot.entities["services"].contains_key("svc"));
        assert!(snapshot.relations["agent_services"].contains_key("global_agent_store"));
        assert!(snapshot.relations["agent_services"].contains_key("agent-a"));
        assert!(snapshot.states["service_status"].contains_key("svc"));

        assert!(store
            .cache()
            .get_entity("services", "svc")
            .await
            .unwrap()
            .is_some());
        let agent_services = store.list_agent_service_names("agent-a").await.unwrap();
        assert_eq!(agent_services, vec!["svc".to_string()]);

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn switch_cache_storage_updates_namespace() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("before-switch".to_string()),
        })
        .unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let snapshot = store
            .switch_cache_storage(CacheStorage::Memory, None, Some("after-switch".to_string()))
            .await
            .unwrap();

        assert!(snapshot.entities["services"].contains_key("svc"));
        assert_eq!(store.namespace(), "after-switch");

        let inspect = store.cache_inspect().await.unwrap();
        assert_eq!(inspect["namespace"], serde_json::json!("after-switch"));
        let collections = inspect["collections"].as_array().unwrap();
        assert!(collections.iter().any(|value| {
            value
                .as_str()
                .map(|text| text.starts_with("after-switch:entity:services"))
                .unwrap_or(false)
        }));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn memory_cache_storage_writes_cache_layers_through_openkeyv() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-openkeyv-memory".to_string()),
        })
        .unwrap();

        store.add_service("svc", stdio_config()).await.unwrap();

        assert_eq!(
            store.current_cache_storage().await,
            CacheStorage::OpenKeyvMemory
        );
        assert!(store
            .cache()
            .get_entity("services", "svc")
            .await
            .unwrap()
            .is_some());
        assert!(store
            .cache()
            .get_state("service_status", "svc")
            .await
            .unwrap()
            .is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn switch_cache_storage_to_openkeyv_memory_migrates_runtime_cache() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let snapshot = store
            .switch_cache_storage(CacheStorage::OpenKeyvMemory, None, None)
            .await
            .unwrap();

        assert!(snapshot.entities["services"].contains_key("svc"));
        assert_eq!(
            store.current_cache_storage().await,
            CacheStorage::OpenKeyvMemory
        );
        assert!(store
            .cache()
            .get_entity("services", "svc")
            .await
            .unwrap()
            .is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn update_and_patch_service_update_runtime_cache() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-update-patch".to_string()),
        })
        .unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let mut updated = stdio_config();
        updated.args = vec!["updated".to_string()];
        store.update_service("svc", updated).await.unwrap();
        let config = store.get_service_config("svc").await.unwrap().unwrap();
        assert_eq!(config["args"], serde_json::json!(["updated"]));

        store
            .patch_service("svc", serde_json::json!({"description": "patched"}))
            .await
            .unwrap();
        let config = store.get_service_config("svc").await.unwrap().unwrap();
        assert_eq!(config["description"], serde_json::json!("patched"));
    }

    #[tokio::test]
    async fn event_history_and_cache_health_are_reported() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-event-health".to_string()),
        })
        .unwrap();

        store
            .publish_event("TEST_EVENT", serde_json::json!({"ok": true}), true)
            .await
            .unwrap();
        let history = store.event_history(10).await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].event_type, "TEST_EVENT");

        store.add_service("svc", stdio_config()).await.unwrap();
        let health = store.cache_health_check().await.unwrap();
        assert_eq!(health["backend"], serde_json::json!("openkeyv_memory"));
        assert!(health["entities"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("services")));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn list_tools_uses_registry_without_transport_connection() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-list-tools-registry".to_string()),
        })
        .unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let tools = store.list_tools("svc").await.unwrap();
        assert!(tools.is_empty());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn connect_service_failure_opens_circuit_and_schedules_retry() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-connect-failure-status".to_string()),
        })
        .unwrap();
        store
            .add_service("broken", broken_stdio_config())
            .await
            .unwrap();

        let err = store
            .connect_service("broken")
            .await
            .unwrap_err()
            .to_string();
        let status = store
            .cached_service_status("broken")
            .await
            .unwrap()
            .unwrap();
        let service = store.find_service("broken").await.unwrap();

        assert!(err.contains("Connection failed"));
        assert_eq!(status.health_status, HealthStatus::CircuitOpen);
        assert_eq!(status.connection_attempts, 1);
        assert!(status.current_error.is_some());
        assert!(status.next_retry_time.is_some());
        assert_eq!(service.status, ConnectionStatus::Error);

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn automatic_retry_respects_backoff_and_enters_half_open_when_due() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-retry-backoff".to_string()),
        })
        .unwrap();
        store
            .add_service("broken", broken_stdio_config())
            .await
            .unwrap();
        store.connect_service("broken").await.unwrap_err();

        let blocked = store
            .connect_service_internal("broken", true)
            .await
            .unwrap_err()
            .to_string();
        assert!(blocked.contains("backoff active"));

        let mut due = store
            .cached_service_status("broken")
            .await
            .unwrap()
            .unwrap();
        due.health_status = HealthStatus::CircuitOpen;
        due.next_retry_time = Some(MCPStore::now_timestamp_f64() - 1.0);
        store.put_service_status_payload(&due).await.unwrap();

        let transitioned = store.health_check("broken").await.unwrap();
        assert_eq!(transitioned.health_status, HealthStatus::HalfOpen);
        assert!(transitioned.lease_deadline.is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn successful_health_check_clears_retry_state() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-retry-reset".to_string()),
        })
        .unwrap();
        store
            .add_service("broken", broken_stdio_config())
            .await
            .unwrap();
        store.connect_service("broken").await.unwrap_err();

        let recovered = store
            .record_health_check_result("broken", true, Some(12.0), None)
            .await
            .unwrap();

        assert_eq!(recovered.health_status, HealthStatus::Healthy);
        assert_eq!(recovered.connection_attempts, 0);
        assert_eq!(recovered.current_error, None);
        assert_eq!(recovered.next_retry_time, None);
        assert_eq!(recovered.hard_deadline, None);
        assert_eq!(recovered.latency_p95, Some(12.0));
        assert_eq!(recovered.latency_p99, Some(12.0));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn db_load_does_not_rewrite_cached_agent_relations() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-db-load-readonly".to_string()),
        })
        .unwrap();
        let service_name = "svc_byagent_agent-a";
        let config = stdio_config();
        store
            .cache()
            .put_entity(
                "services",
                service_name,
                serde_json::to_value(ServiceEntity {
                    service_global_name: service_name.to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: "agent-a".to_string(),
                    config: serde_json::to_value(config).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_relation(
                "agent_services",
                "agent-a",
                serde_json::to_value(AgentServiceRelation {
                    services: vec![ServiceRelationItem {
                        service_original_name: "svc".to_string(),
                        service_global_name: service_name.to_string(),
                        client_id: service_name.to_string(),
                        established_time: 111,
                        last_access: Some(222),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();

        store.load_from_config().await.unwrap();

        let relation = store
            .cache()
            .get_relation("agent_services", "agent-a")
            .await
            .unwrap()
            .unwrap();
        let relation: AgentServiceRelation = serde_json::from_value(relation).unwrap();
        assert_eq!(relation.services[0].established_time, 111);
        assert_eq!(relation.services[0].last_access, Some(222));
    }
}
