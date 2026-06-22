
use super::layer::*;
use crate::cache::memory_cache_store;

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
async fn test_create_agent() {
    let store = memory_cache_store();
    let mgr = CacheLayerManager::new(store, "test");

    mgr.create_agent("a1", 1234567890, false).await.unwrap();
    let agent = mgr.get_agent("a1").await.unwrap().unwrap();
    assert_eq!(agent["agent_id"], "a1");
    assert_eq!(agent["is_global"], false);
}
