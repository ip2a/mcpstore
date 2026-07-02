use super::layer::*;
use crate::cache::memory_cache_store;
use crate::cache::models::{
    ContextToolVisibilityState, OpenApiImportContextState, SessionContextState, SessionEntity,
    SessionEvent, SessionEventType, SessionScope, SessionServiceItem, SessionServiceRelation,
    SessionStatus, SessionStatusState, SessionToolItem, SessionToolVisibility, ToolPreferenceState,
    ToolVisibilityMode,
};

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

    mgr.put_entity("services", "svc1", serde_json::json!({"url": "http://x"}))
        .await
        .unwrap();
    let v = mgr.get_entity("services", "svc1").await.unwrap();
    assert_eq!(v, Some(serde_json::json!({"url": "http://x"})));
}

#[tokio::test]
async fn test_cache_layer_manager_session_entity() {
    let store = memory_cache_store();
    let mgr = CacheLayerManager::new(store, "test");

    let session = SessionEntity {
        session_key: "store:global:s1".to_string(),
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
        .get_entity("sessions", "store:global:s1")
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
async fn test_replace_store_with_snapshot_migrates_all_layers() {
    let first = memory_cache_store();
    let second = memory_cache_store();
    let mgr = CacheLayerManager::new(first, "test");

    mgr.put_entity("services", "svc", serde_json::json!({"name": "svc"}))
        .await
        .unwrap();
    mgr.put_relation(
        "agent_services",
        "agent",
        serde_json::json!({"services": []}),
    )
    .await
    .unwrap();
    mgr.put_state("service_status", "svc", serde_json::json!({"status": "ok"}))
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
    assert_eq!(snapshot.entities["services"].len(), 1);
    assert_eq!(snapshot.relations["agent_services"].len(), 1);
    assert_eq!(snapshot.states["service_status"].len(), 1);
    assert_eq!(snapshot.events["service"].len(), 1);

    assert!(mgr.get_entity("services", "svc").await.unwrap().is_some());
    assert!(mgr
        .get_relation("agent_services", "agent")
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_state("service_status", "svc")
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
    let session_key = "store:global:s1";

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
                service_global_name: "store.svc".to_string(),
                service_original_name: "svc".to_string(),
                source_agent: "global".to_string(),
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
                service_global_name: "store.svc".to_string(),
                tool_global_name: "store.svc.echo".to_string(),
                tool_original_name: "echo".to_string(),
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
        "store:global",
        serde_json::to_value(SessionContextState {
            context_key: "store:global".to_string(),
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
        "store:global:store.svc",
        serde_json::to_value(ContextToolVisibilityState {
            context_key: "store:global".to_string(),
            service_global_name: "store.svc".to_string(),
            mode: ToolVisibilityMode::Allowlist,
            tools: vec![SessionToolItem {
                service_global_name: "store.svc".to_string(),
                tool_global_name: "store.svc.echo".to_string(),
                tool_original_name: "echo".to_string(),
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
        "store:global:store.svc.echo",
        serde_json::to_value(ToolPreferenceState {
            context_key: "store:global".to_string(),
            service_global_name: "store.svc".to_string(),
            tool_global_name: "store.svc.echo".to_string(),
            tool_original_name: "echo".to_string(),
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
        "store:global:s1:0001",
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
        .get_state("context_tool_visibility", "store:global:store.svc")
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_state("tool_preferences", "store:global:store.svc.echo")
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_state("openapi_import_context", "global")
        .await
        .unwrap()
        .is_some());
    assert!(mgr
        .get_event("session_events", "store:global:s1:0001")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn test_create_agent() {
    let store = memory_cache_store();
    let mgr = CacheLayerManager::new(store, "test");

    mgr.create_agent("a1", 1234567890, false).await.unwrap();
    let agent = mgr.get_agent("a1").await.unwrap().unwrap();
    assert_eq!(agent["agent_id"], "a1");
    assert_eq!(agent["is_global"], false);
}
