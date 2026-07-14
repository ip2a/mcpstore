use super::layer::*;
use crate::cache::memory_cache_store;
use crate::cache::models::{
    ContextToolVisibilityState, HealthStatus, InstanceStatus, OpenApiImportContextState,
    ServiceLifecycleState, SessionContextState, SessionEntity, SessionEvent, SessionEventType,
    SessionScope, SessionServiceItem, SessionServiceRelation, SessionStatus, SessionStatusState,
    SessionToolItem, SessionToolVisibility, ToolAvailability, ToolPreferenceState, ToolStatusItem,
    ToolVisibilityMode,
};
use crate::config::ScopeDeclarations;
use crate::identity::{ScopeRef, ServiceInstanceKey};
use crate::registry::{ConfigRevision, ConnectionStatus, ServiceDefinition, ServiceInstance};
use crate::store::{CacheStorage, MCPStore, StoreOptions};

fn store_instance_id() -> crate::identity::InstanceId {
    ServiceInstanceKey::new("svc", ScopeRef::Store).instance_id()
}

#[tokio::test]
async fn test_openkeyv_memory_store_basic() {
    let store = memory_cache_store();
    store
        .put("k1", serde_json::json!({"a": 1}), "c1")
        .await
        .unwrap();
    let v = store.get("k1", "c1").await.unwrap();
    assert_eq!(v, Some(serde_json::json!({"a": 1})));
}

#[tokio::test]
async fn test_cache_layer_manager_entity() {
    let store = memory_cache_store();
    let mgr = CacheLayerManager::new(store, "test");

    mgr.put_entity(
        "service_definitions",
        "svc1",
        serde_json::json!({"service_name": "svc1"}),
    )
    .await
    .unwrap();
    let v = mgr.get_entity("service_definitions", "svc1").await.unwrap();
    assert_eq!(v, Some(serde_json::json!({"service_name": "svc1"})));
}

#[tokio::test]
async fn test_cache_layer_manager_session_entity() {
    let store = memory_cache_store();
    let mgr = CacheLayerManager::new(store, "test");

    let session = SessionEntity {
        session_key: "store:s1".to_string(),
        session_id: "s1".to_string(),
        scope: SessionScope::Store,
        agent_id: None,
        created_at: 100,
        updated_at: 100,
        last_active: 100,
        lease_seconds: None,
        expires_at: None,
        version: 1,
        metadata: serde_json::json!({"purpose": "test"}),
    };

    mgr.put_entity(
        "sessions",
        &session.session_key,
        serde_json::to_value(&session).unwrap(),
    )
    .await
    .unwrap();

    let value = mgr
        .get_entity("sessions", "store:s1")
        .await
        .unwrap()
        .unwrap();
    let restored: SessionEntity = serde_json::from_value(value).unwrap();
    assert_eq!(restored, session);
}

#[tokio::test]
async fn test_cache_layer_rejects_removed_client_configs_entity() {
    let store = memory_cache_store();
    let mgr = CacheLayerManager::new(store, "test");

    let err = mgr
        .get_entity("client_configs", "client")
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("Unknown entity_type 'client_configs'"));
}

#[tokio::test]
async fn test_cache_layer_clears_unversioned_schema() {
    let store = memory_cache_store();
    store
        .put(
            "svc",
            serde_json::json!({"service_global_name": "svc"}),
            "test:entity:services",
        )
        .await
        .unwrap();
    let mgr = CacheLayerManager::new(store.clone(), "test");

    assert!(mgr
        .get_entity("service_definitions", "svc")
        .await
        .unwrap()
        .is_none());
    assert!(store
        .get("svc", "test:entity:services")
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        store
            .get("current", "test:state:cache_schema")
            .await
            .unwrap(),
        Some(serde_json::json!({"version": CACHE_SCHEMA_VERSION}))
    );
}

#[tokio::test]
async fn test_cache_layer_clears_old_versioned_schema() {
    let store = memory_cache_store();
    store
        .put(
            "current",
            serde_json::json!({"version": 1}),
            "test:state:cache_schema",
        )
        .await
        .unwrap();
    store
        .put(
            "svc",
            serde_json::json!({"service_name": "svc"}),
            "test:entity:service_definitions",
        )
        .await
        .unwrap();
    let mgr = CacheLayerManager::new(store.clone(), "test");

    assert!(mgr
        .get_entity("service_definitions", "svc")
        .await
        .unwrap()
        .is_none());
    assert!(store
        .get("svc", "test:entity:service_definitions")
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        store
            .get("current", "test:state:cache_schema")
            .await
            .unwrap(),
        Some(serde_json::json!({"version": CACHE_SCHEMA_VERSION}))
    );
}

#[tokio::test]
async fn test_cache_instance_added_preserves_observed_status_on_upsert() {
    let store = MCPStore::setup_with_options(StoreOptions {
        backend: Some(CacheStorage::Memory),
        namespace: Some("cache-instance-status-upsert".to_string()),
        ..StoreOptions::default()
    })
    .unwrap();
    let instance_id = store_instance_id();
    store
        .registry
        .register_definition(ServiceDefinition {
            service_name: "svc".to_string(),
            base_config: serde_json::Map::new(),
            scopes: ScopeDeclarations::store_only(),
            lifecycle: None,
            metadata: serde_json::Map::new(),
            base_revision: 1,
            added_time: 100,
        })
        .await;
    store
        .registry
        .register_instance(ServiceInstance {
            instance_id,
            service_name: "svc".to_string(),
            scope: ScopeRef::Store,
            transport: "stdio".to_string(),
            url: None,
            command: Some("first-command".to_string()),
            status: ConnectionStatus::Connected,
            tools: Vec::new(),
            effective_config: serde_json::Map::new(),
            config_revision: ConfigRevision {
                base_revision: 1,
                scope_revision: 1,
            },
            applied_config_revision: Some(ConfigRevision {
                base_revision: 1,
                scope_revision: 1,
            }),
            added_time: 100,
        })
        .await;
    store.cache_instance_added(instance_id).await.unwrap();

    let observed = InstanceStatus {
        instance_id,
        service_name: "svc".to_string(),
        scope: ScopeRef::Store,
        health_status: HealthStatus::Degraded,
        last_health_check: 200,
        connection_attempts: 3,
        max_connection_attempts: 9,
        current_error: Some("temporary failure".to_string()),
        tools: vec![ToolStatusItem {
            tool_name: "echo".to_string(),
            status: ToolAvailability::Unavailable,
        }],
        window_error_rate: Some(0.25),
        latency_p95: Some(120.0),
        latency_p99: Some(180.0),
        sample_size: Some(20),
        next_retry_time: Some(300.0),
        hard_deadline: Some(400.0),
        lease_deadline: Some(500.0),
        lifecycle_state: ServiceLifecycleState {
            restart_attempts: 2,
            manually_stopped: true,
            manually_stopped_at: Some(190),
            manual_stop_persistent: true,
        },
    };
    store.put_instance_status(&observed).await.unwrap();

    let mut updated = store.registry.find_instance(instance_id).await.unwrap();
    updated.command = Some("updated-command".to_string());
    updated.config_revision = ConfigRevision {
        base_revision: 2,
        scope_revision: 1,
    };
    store.registry.register_instance(updated).await;
    store.cache_instance_added(instance_id).await.unwrap();

    assert_eq!(
        store.cached_instance_status(instance_id).await.unwrap(),
        Some(observed)
    );
}

#[test]
fn test_cache_snapshot_rejects_missing_schema_version() {
    let result = serde_json::from_value::<CacheSnapshot>(serde_json::json!({
        "entities": {},
        "relations": {},
        "states": {},
        "events": {}
    }));

    assert!(result.is_err());
}

#[tokio::test]
async fn test_replace_store_with_snapshot_migrates_all_layers() {
    let first = memory_cache_store();
    let second = memory_cache_store();
    let mgr = CacheLayerManager::new(first, "test");

    let instance_id = store_instance_id().to_string();
    mgr.put_entity(
        "service_instances",
        &instance_id,
        serde_json::json!({"instance_id": instance_id}),
    )
    .await
    .unwrap();
    mgr.put_relation(
        "agent_instances",
        "agent",
        serde_json::json!({"instances": []}),
    )
    .await
    .unwrap();
    mgr.put_state(
        "instance_status",
        &instance_id,
        serde_json::json!({"instance_id": instance_id}),
    )
    .await
    .unwrap();
    mgr.put_event(
        "service",
        "svc:added",
        serde_json::json!({"event": "added"}),
    )
    .await
    .unwrap();

    let snapshot = mgr
        .replace_store_with_snapshot_and_namespace(second, "test")
        .await
        .unwrap();
    assert_eq!(snapshot.schema_version, CACHE_SCHEMA_VERSION);
    assert_eq!(snapshot.entities["service_instances"].len(), 1);
    assert_eq!(snapshot.relations["agent_instances"].len(), 1);
    assert_eq!(snapshot.states["instance_status"].len(), 1);
    assert_eq!(snapshot.events["service"].len(), 1);

    assert!(mgr
        .get_entity("service_instances", &instance_id)
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_relation("agent_instances", "agent")
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_state("instance_status", &instance_id)
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_event("service", "svc:added")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn test_replace_store_with_snapshot_migrates_session_layers() {
    let first = memory_cache_store();
    let second = memory_cache_store();
    let mgr = CacheLayerManager::new(first, "test");
    let session_key = "store:s1";
    let instance_id = store_instance_id();

    mgr.put_entity(
        "sessions",
        session_key,
        serde_json::to_value(SessionEntity {
            session_key: session_key.to_string(),
            session_id: "s1".to_string(),
            scope: SessionScope::Store,
            agent_id: None,
            created_at: 100,
            updated_at: 100,
            last_active: 100,
            lease_seconds: Some(300),
            expires_at: Some(400),
            version: 1,
            metadata: serde_json::json!({}),
        })
        .unwrap(),
    )
    .await
    .unwrap();
    mgr.put_relation(
        "session_services",
        session_key,
        serde_json::to_value(SessionServiceRelation {
            session_key: session_key.to_string(),
            services: vec![SessionServiceItem {
                instance_id,
                service_name: "svc".to_string(),
                scope: ScopeRef::Store,
                bound_at: 101,
            }],
            updated_at: 101,
            version: 1,
        })
        .unwrap(),
    )
    .await
    .unwrap();
    mgr.put_relation(
        "session_tools",
        session_key,
        serde_json::to_value(SessionToolVisibility {
            session_key: session_key.to_string(),
            mode: ToolVisibilityMode::Allowlist,
            tools: vec![SessionToolItem {
                instance_id,
                service_name: "svc".to_string(),
                scope: ScopeRef::Store,
                tool_name: "echo".to_string(),
            }],
            updated_at: 102,
            version: 1,
        })
        .unwrap(),
    )
    .await
    .unwrap();
    mgr.put_state(
        "session_status",
        session_key,
        serde_json::to_value(SessionStatusState {
            session_key: session_key.to_string(),
            status: SessionStatus::Active,
            updated_at: 100,
            version: 1,
            reason: None,
        })
        .unwrap(),
    )
    .await
    .unwrap();
    mgr.put_state(
        "session_context",
        "store",
        serde_json::to_value(SessionContextState {
            context_key: "store".to_string(),
            active_session_key: Some(session_key.to_string()),
            auto_session_key: None,
            updated_at: 100,
            version: 1,
        })
        .unwrap(),
    )
    .await
    .unwrap();
    mgr.put_state(
        "context_tool_visibility",
        &format!("store:{instance_id}"),
        serde_json::to_value(ContextToolVisibilityState {
            context_key: "store".to_string(),
            instance_id,
            service_name: "svc".to_string(),
            scope: ScopeRef::Store,
            mode: ToolVisibilityMode::Allowlist,
            tools: vec![SessionToolItem {
                instance_id,
                service_name: "svc".to_string(),
                scope: ScopeRef::Store,
                tool_name: "echo".to_string(),
            }],
            updated_at: 103,
            version: 1,
        })
        .unwrap(),
    )
    .await
    .unwrap();
    mgr.put_state(
        "tool_preferences",
        &format!("store:{instance_id}:echo"),
        serde_json::to_value(ToolPreferenceState {
            context_key: "store".to_string(),
            instance_id,
            service_name: "svc".to_string(),
            scope: ScopeRef::Store,
            tool_name: "echo".to_string(),
            preferences: serde_json::Map::from_iter([(
                "return_direct".to_string(),
                serde_json::json!(true),
            )]),
            updated_at: 104,
            version: 1,
        })
        .unwrap(),
    )
    .await
    .unwrap();
    mgr.put_state(
        "openapi_import_context",
        "global",
        serde_json::to_value(OpenApiImportContextState {
            last_service_name: "inventory".to_string(),
            updated_at: 105,
            version: 1,
        })
        .unwrap(),
    )
    .await
    .unwrap();
    mgr.put_event(
        "session_events",
        "store:s1:0001",
        serde_json::to_value(SessionEvent {
            session_key: session_key.to_string(),
            event_type: SessionEventType::Create,
            occurred_at: 100,
            payload: serde_json::json!({}),
        })
        .unwrap(),
    )
    .await
    .unwrap();

    let snapshot = mgr
        .replace_store_with_snapshot_and_namespace(second, "test")
        .await
        .unwrap();

    assert_eq!(snapshot.entities["sessions"].len(), 1);
    assert_eq!(snapshot.relations["session_services"].len(), 1);
    assert_eq!(snapshot.relations["session_tools"].len(), 1);
    assert_eq!(snapshot.states["session_status"].len(), 1);
    assert_eq!(snapshot.states["session_context"].len(), 1);
    assert_eq!(snapshot.states["context_tool_visibility"].len(), 1);
    assert_eq!(snapshot.states["tool_preferences"].len(), 1);
    assert_eq!(snapshot.states["openapi_import_context"].len(), 1);
    assert_eq!(snapshot.events["session_events"].len(), 1);
    assert!(mgr
        .get_entity("sessions", session_key)
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_relation("session_services", session_key)
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_relation("session_tools", session_key)
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_state("session_status", session_key)
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_state("context_tool_visibility", &format!("store:{instance_id}"))
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_state("tool_preferences", &format!("store:{instance_id}:echo"))
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_state("openapi_import_context", "global")
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_event("session_events", "store:s1:0001")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn test_create_agent() {
    let store = memory_cache_store();
    let mgr = CacheLayerManager::new(store, "test");

    mgr.create_agent("a1", 1234567890).await.unwrap();
    let agent = mgr.get_agent("a1").await.unwrap().unwrap();
    assert_eq!(agent["agent_id"], "a1");
    assert!(agent.get("is_global").is_none());
}
