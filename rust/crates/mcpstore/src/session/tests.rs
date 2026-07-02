use super::*;
use crate::StoreOptions;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn temp_config_path() -> String {
    std::env::temp_dir()
        .join(format!("mcpstore-session-{}.json", uuid::Uuid::new_v4()))
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

async fn register_tool_service(store: &MCPStore, name: &str, tools: &[&str]) {
    store.add_service(name, stdio_config()).await.unwrap();
    let mut service = store.registry.find_service(name).await.unwrap();
    service.status = ConnectionStatus::Connected;
    service.tools = tools
        .iter()
        .map(|tool| crate::registry::ToolInfo {
            name: (*tool).to_string(),
            description: (*tool).to_string(),
            schema: serde_json::json!({"type": "object"}),
        })
        .collect();
    store.registry.register(service).await;
}

#[tokio::test]
async fn create_session_defaults_to_store_scope_key() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();

    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();

    assert_eq!(session.session_key, "store:global:s1");
    assert_eq!(session.scope, SessionScope::Store);
    assert_eq!(session.agent_id, None);
    assert_eq!(session.version, 1);
    assert!(store
        .cache()
        .get_state("session_status", "store:global:s1")
        .await
        .unwrap()
        .is_some());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn session_context_create_or_get_wraps_rust_agent_session_flow() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    register_tool_service(&store, "alpha", &["echo", "search"]).await;
    register_tool_service(&store, "beta", &["read"]).await;

    let session = store
        .session("task-1")
        .lease_seconds(120)
        .metadata(serde_json::json!({"owner": "rust-agent"}))
        .create_or_get()
        .await
        .unwrap();

    assert_eq!(session.session_key(), "store:global:task-1");
    assert_eq!(
        session.entity().await.unwrap().metadata["owner"],
        "rust-agent"
    );
    session.bind_service("alpha").await.unwrap();
    session
        .set_tool_visibility(vec![SessionToolSelection {
            service_name: "alpha".to_string(),
            tool_name: "search".to_string(),
        }])
        .await
        .unwrap();

    let tools = session.list_tools().await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "alpha_search");

    let same_session = store.session("task-1").create_or_get().await.unwrap();
    assert_eq!(same_session.session_key(), session.session_key());
    assert_eq!(same_session.list_services().await.unwrap().len(), 1);

    let status = session.close_with_reason("done").await.unwrap();
    assert_eq!(status.status, SessionStatus::Closed);
    let err = same_session.list_tools().await.unwrap_err().to_string();
    assert!(err.contains("Session is not active"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn session_context_can_attach_to_existing_session_key() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let created = store
        .session("attached")
        .for_agent("agent-a")
        .create()
        .await
        .unwrap();

    let attached = store.session_by_key(created.session_key());

    assert_eq!(
        attached.entity().await.unwrap().agent_id.as_deref(),
        Some("agent-a")
    );
    attached.extend(60).await.unwrap();
    assert_eq!(created.entity().await.unwrap().lease_seconds, Some(60));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn redis_backend_shares_session_state_between_store_instances_when_available() {
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        return;
    };
    let namespace = format!("session-e2e-{}", uuid::Uuid::new_v4());
    let first_path = temp_config_path();
    let second_path = temp_config_path();
    let first = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(first_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url.clone()),
        namespace: Some(namespace.clone()),
    })
    .unwrap();
    let second = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(second_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url),
        namespace: Some(namespace),
    })
    .unwrap();

    let created = first
        .session("shared")
        .lease_seconds(30)
        .metadata(serde_json::json!({"owner": "first"}))
        .create()
        .await
        .unwrap();

    let seen = second.session("shared").get().await.unwrap().unwrap();
    assert_eq!(seen.session_key(), created.session_key());
    assert_eq!(seen.entity().await.unwrap().metadata["owner"], "first");

    seen.extend(90).await.unwrap();
    assert_eq!(created.entity().await.unwrap().lease_seconds, Some(90));

    first
        .set_session_state(
            created.session_key(),
            "cursor",
            serde_json::json!({"page": 1}),
        )
        .await
        .unwrap();
    assert_eq!(
        second
            .get_session_state_value(created.session_key(), "cursor")
            .await
            .unwrap(),
        Some(serde_json::json!({"page": 1}))
    );
    second
        .set_session_state_with_retry(
            created.session_key(),
            "cursor",
            serde_json::json!({"page": 2}),
            SessionRetryPolicy::new(3),
        )
        .await
        .unwrap();
    assert_eq!(
        first
            .list_session_state(created.session_key())
            .await
            .unwrap()
            .values
            .get("cursor"),
        Some(&serde_json::json!({"page": 2}))
    );
    first
        .delete_session_state(created.session_key(), "cursor")
        .await
        .unwrap();
    assert_eq!(
        second
            .get_session_state_value(created.session_key(), "cursor")
            .await
            .unwrap(),
        None
    );
    second
        .set_session_state(created.session_key(), "answer", serde_json::json!(42))
        .await
        .unwrap();
    first
        .clear_session_state(created.session_key())
        .await
        .unwrap();
    assert!(second
        .list_session_state(created.session_key())
        .await
        .unwrap()
        .values
        .is_empty());

    seen.close_with_reason("done").await.unwrap();
    assert_eq!(
        created.status().await.unwrap().status,
        SessionStatus::Closed
    );

    std::fs::remove_file(first_path).ok();
    std::fs::remove_file(second_path).ok();
}

#[tokio::test]
async fn redis_backend_shares_session_bindings_and_tool_visibility_when_available() {
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        return;
    };
    let namespace = format!("session-business-e2e-{}", uuid::Uuid::new_v4());
    let first_path = temp_config_path();
    let second_path = temp_config_path();
    let first = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(first_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url.clone()),
        namespace: Some(namespace.clone()),
    })
    .unwrap();
    let second = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(second_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url),
        namespace: Some(namespace),
    })
    .unwrap();
    register_tool_service(&first, "alpha", &["echo", "search"]).await;
    register_tool_service(&second, "alpha", &["echo", "search"]).await;

    let created = first.session("shared-tools").create().await.unwrap();
    first
        .bind_service_to_session(created.session_key(), "alpha")
        .await
        .unwrap();
    first
        .set_session_tool_visibility(
            created.session_key(),
            vec![SessionToolSelection {
                service_name: "alpha".to_string(),
                tool_name: "search".to_string(),
            }],
        )
        .await
        .unwrap();

    let seen = second.session("shared-tools").get().await.unwrap().unwrap();
    assert_eq!(seen.session_key(), created.session_key());
    assert_eq!(seen.list_services().await.unwrap().len(), 1);
    assert_eq!(seen.list_session_tools().await.unwrap().len(), 1);
    assert_eq!(seen.list_tools().await.unwrap()[0].name, "alpha_search");

    second
        .unbind_service_from_session(created.session_key(), "alpha")
        .await
        .unwrap();
    assert!(first.list_session_services(created.session_key()).await.unwrap().is_empty());

    std::fs::remove_file(first_path).ok();
    std::fs::remove_file(second_path).ok();
}

#[tokio::test]
async fn session_context_state_tracks_active_and_auto_session_pointers() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let active = store.session("active").create().await.unwrap();
    let auto = store.session("auto").create().await.unwrap();

    let state = store
        .set_active_session_for_context(SessionScope::Store, None, Some(active.session_key()))
        .await
        .unwrap();
    assert_eq!(
        state.active_session_key.as_deref(),
        Some(active.session_key())
    );
    assert_eq!(
        store
            .get_active_session_for_context(SessionScope::Store, None)
            .await
            .unwrap()
            .unwrap()
            .session_key,
        active.session_key()
    );

    let state = store
        .enable_auto_session_for_context(SessionScope::Store, None, auto.session_key())
        .await
        .unwrap();
    assert_eq!(state.auto_session_key.as_deref(), Some(auto.session_key()));
    assert!(store
        .is_auto_session_enabled_for_context(SessionScope::Store, None)
        .await
        .unwrap());
    assert_eq!(
        store
            .get_active_session_for_context(SessionScope::Store, None)
            .await
            .unwrap()
            .unwrap()
            .session_key,
        active.session_key()
    );

    active.close().await.unwrap();
    assert_eq!(
        store
            .get_active_session_for_context(SessionScope::Store, None)
            .await
            .unwrap()
            .unwrap()
            .session_key,
        auto.session_key()
    );

    store
        .set_active_session_for_context(SessionScope::Store, None, None)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_active_session_for_context(SessionScope::Store, None)
            .await
            .unwrap()
            .unwrap()
            .session_key,
        auto.session_key()
    );

    auto.close().await.unwrap();
    assert!(store
        .get_active_session_for_context(SessionScope::Store, None)
        .await
        .unwrap()
        .is_none());

    let state = store
        .disable_auto_session_for_context(SessionScope::Store, None)
        .await
        .unwrap();
    assert!(state.auto_session_key.is_none());
    assert!(!store
        .is_auto_session_enabled_for_context(SessionScope::Store, None)
        .await
        .unwrap());
    assert!(store
        .get_active_session_for_context(SessionScope::Store, None)
        .await
        .unwrap()
        .is_none());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn redis_backend_shares_session_context_state_between_store_instances_when_available() {
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        return;
    };
    let namespace = format!("session-context-e2e-{}", uuid::Uuid::new_v4());
    let first_path = temp_config_path();
    let second_path = temp_config_path();
    let first = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(first_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url.clone()),
        namespace: Some(namespace.clone()),
    })
    .unwrap();
    let second = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(second_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url),
        namespace: Some(namespace),
    })
    .unwrap();

    let active = first.session("active").create().await.unwrap();
    let auto = first.session("auto").create().await.unwrap();

    first
        .set_active_session_for_context(SessionScope::Store, None, Some(active.session_key()))
        .await
        .unwrap();
    assert_eq!(
        second
            .get_active_session_for_context(SessionScope::Store, None)
            .await
            .unwrap()
            .unwrap()
            .session_key,
        active.session_key()
    );

    second
        .enable_auto_session_for_context(SessionScope::Store, None, auto.session_key())
        .await
        .unwrap();
    assert!(first
        .is_auto_session_enabled_for_context(SessionScope::Store, None)
        .await
        .unwrap());
    second
        .set_active_session_for_context(SessionScope::Store, None, None)
        .await
        .unwrap();
    assert_eq!(
        first
            .get_active_session_for_context(SessionScope::Store, None)
            .await
            .unwrap()
            .unwrap()
            .session_key,
        auto.session_key()
    );

    std::fs::remove_file(first_path).ok();
    std::fs::remove_file(second_path).ok();
}

#[tokio::test]
async fn redis_backend_rejects_stale_session_cas_write_when_available() {
    let Ok(redis_url) = std::env::var("MCPSTORE_TEST_REDIS_URL") else {
        return;
    };
    let namespace = format!("session-cas-{}", uuid::Uuid::new_v4());
    let first_path = temp_config_path();
    let second_path = temp_config_path();
    let first = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(first_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url.clone()),
        namespace: Some(namespace.clone()),
    })
    .unwrap();
    let second = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(second_path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Redis),
        redis_url: Some(redis_url),
        namespace: Some(namespace),
    })
    .unwrap();

    let session = first.session("shared").create().await.unwrap();
    let mut stale = session.entity().await.unwrap();

    second
        .session_by_key(session.session_key())
        .extend(30)
        .await
        .unwrap();
    stale.lease_seconds = Some(60);
    stale.version += 1;

    let err = first
        .cache()
        .compare_and_put_entity(
            "sessions",
            session.session_key(),
            Some(1),
            serde_json::to_value(stale).unwrap(),
        )
        .await;

    assert!(matches!(err, Err(CacheError::Conflict(_))));
    assert_eq!(session.entity().await.unwrap().lease_seconds, Some(30));

    std::fs::remove_file(first_path).ok();
    std::fs::remove_file(second_path).ok();
}

#[tokio::test]
async fn agent_sessions_allow_same_session_id_without_collision() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();

    let a = store
        .create_session(CreateSessionRequest::agent("s1", "agent-a"))
        .await
        .unwrap();
    let b = store
        .create_session(CreateSessionRequest::agent("s1", "agent-b"))
        .await
        .unwrap();

    assert_eq!(a.session_key, "agent:agent-a:s1");
    assert_eq!(b.session_key, "agent:agent-b:s1");
    let sessions = store
        .list_sessions(Some(SessionScope::Agent), None)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 2);
    let agent_a = store
        .list_sessions(Some(SessionScope::Agent), Some("agent-a"))
        .await
        .unwrap();
    assert_eq!(agent_a, vec![a]);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn user_session_lookup_uses_rust_core_metadata() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();

    let session = store
        .session("shared")
        .metadata(serde_json::json!({"owner": "test"}))
        .create()
        .await
        .unwrap();

    assert!(store
        .find_session_by_user_session_id("user-1")
        .await
        .unwrap()
        .is_none());

    let updated = store
        .update_session_metadata(
            session.session_key(),
            serde_json::json!({"owner": "test", "user_session_id": "user-1"}),
        )
        .await
        .unwrap();
    assert_eq!(updated.metadata["user_session_id"], "user-1");

    let found = store
        .find_session_by_user_session_id("user-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.session_key, session.session_key());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn close_session_soft_closes_and_extend_rejects_closed_session() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();

    let status = store
        .close_session(&session.session_key, Some("done".to_string()))
        .await
        .unwrap();

    assert_eq!(status.status, SessionStatus::Closed);
    assert_eq!(status.reason.as_deref(), Some("done"));
    let err = store
        .extend_session(&session.session_key, 60)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Session is not active"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn scoped_session_batch_lifecycle_runs_in_rust_core() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let first = store
        .create_session(CreateSessionRequest::store("first"))
        .await
        .unwrap();
    let second = store
        .create_session(CreateSessionRequest::store("second"))
        .await
        .unwrap();
    let agent = store
        .create_session(CreateSessionRequest::agent("first", "agent-a"))
        .await
        .unwrap();

    store
        .set_active_session_for_context(SessionScope::Store, None, Some(&first.session_key))
        .await
        .unwrap();
    store
        .enable_auto_session_for_context(SessionScope::Store, None, &second.session_key)
        .await
        .unwrap();

    let statuses = store
        .close_sessions(Some(SessionScope::Store), None, Some("done".to_string()))
        .await
        .unwrap();
    assert_eq!(statuses.len(), 2);
    assert!(statuses
        .iter()
        .all(|status| status.status == SessionStatus::Closed));
    assert_eq!(
        store
            .get_session_status(&agent.session_key)
            .await
            .unwrap()
            .unwrap()
            .status,
        SessionStatus::Active
    );
    assert!(store
        .get_active_session_for_context(SessionScope::Store, None)
        .await
        .unwrap()
        .is_none());
    assert!(!store
        .is_auto_session_enabled_for_context(SessionScope::Store, None)
        .await
        .unwrap());

    let mut request = CreateSessionRequest::store("expired");
    request.lease_seconds = Some(1);
    let expired = store.create_session(request).await.unwrap();
    store
        .set_active_session_for_context(SessionScope::Store, None, Some(&expired.session_key))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let report = store
        .cleanup_sessions(Some(SessionScope::Store), None)
        .await
        .unwrap();
    assert!(report.refreshed_sessions >= 3);
    assert_eq!(report.expired_sessions, 1);
    assert!(report.cleared_active_session);
    assert!(store
        .get_active_session_for_context(SessionScope::Store, None)
        .await
        .unwrap()
        .is_none());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn extend_session_updates_lease_and_expiry() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();

    let updated = store
        .extend_session(&session.session_key, 120)
        .await
        .unwrap();

    assert_eq!(updated.lease_seconds, Some(120));
    assert!(updated.expires_at.is_some());
    assert!(updated.version > session.version);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn stale_session_entity_write_is_rejected_by_version() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();
    let mut first_update = session.clone();
    first_update.version += 1;
    first_update.last_active += 1;
    store
        .cache()
        .compare_and_put_entity(
            "sessions",
            &session.session_key,
            Some(session.version),
            serde_json::to_value(first_update.clone()).unwrap(),
        )
        .await
        .unwrap();

    let mut stale_update = session.clone();
    stale_update.version += 1;
    stale_update.last_active += 2;
    let err = store
        .cache()
        .compare_and_put_entity(
            "sessions",
            &session.session_key,
            Some(session.version),
            serde_json::to_value(stale_update).unwrap(),
        )
        .await;

    assert!(matches!(err, Err(CacheError::Conflict(_))));
    assert_eq!(
        store.get_session(&session.session_key).await.unwrap(),
        Some(first_update)
    );

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn session_retry_policy_retries_cache_conflicts_only() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let result = MCPStore::retry_session_write(SessionRetryPolicy::new(3), || {
        let attempts = Arc::clone(&attempts);
        async move {
            if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(StoreError::Cache(CacheError::Conflict("stale".to_string())));
            }
            Ok("ok")
        }
    })
    .await
    .unwrap();

    assert_eq!(result, "ok");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn session_retry_policy_respects_attempt_limit() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let err = MCPStore::retry_session_write(SessionRetryPolicy::new(1), || {
        let attempts = Arc::clone(&attempts);
        async move {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(StoreError::Cache(CacheError::Conflict("stale".to_string())))
        }
    })
    .await
    .unwrap_err();

    assert!(matches!(err, StoreError::Cache(CacheError::Conflict(_))));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn session_state_is_serializable_versioned_and_active_only() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let session = store
        .create_session(CreateSessionRequest::store("stateful"))
        .await
        .unwrap();

    let state = store
        .set_session_state(
            &session.session_key,
            "cursor",
            serde_json::json!({"page": 2, "items": ["a", "b"]}),
        )
        .await
        .unwrap();

    assert_eq!(state.version, 1);
    assert_eq!(state.values["cursor"]["page"], 2);
    assert_eq!(
        store
            .get_session_state_value(&session.session_key, "cursor")
            .await
            .unwrap()
            .unwrap()["items"][1],
        "b"
    );

    let state = SessionContext::from_key(&store, &session.session_key)
        .set_state_with_retry("answer", serde_json::json!(42), SessionRetryPolicy::new(2))
        .await
        .unwrap();
    assert_eq!(state.version, 2);
    assert_eq!(state.values["answer"], 42);

    let state = store
        .delete_session_state(&session.session_key, "cursor")
        .await
        .unwrap();
    assert_eq!(state.version, 3);
    assert!(!state.values.contains_key("cursor"));

    store
        .close_session(&session.session_key, Some("done".to_string()))
        .await
        .unwrap();
    let err = store
        .set_session_state(&session.session_key, "after_close", serde_json::json!(true))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Session is not active"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn expired_session_rejects_extend_and_marks_status_expired() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();

    let mut raw = session.clone();
    raw.expires_at = Some(MCPStore::now_timestamp() - 1);
    let session_key = raw.session_key.clone();
    store
        .cache()
        .put_entity("sessions", &session_key, serde_json::to_value(raw).unwrap())
        .await
        .unwrap();

    let err = store
        .extend_session(&session.session_key, 60)
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("Session is not active"));
    let status: SessionStatusState = serde_json::from_value(
        store
            .cache()
            .get_state("session_status", &session.session_key)
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(status.status, SessionStatus::Expired);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn bind_and_unbind_service_updates_session_relation() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();
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
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();

    let relation = store
        .bind_service_to_session(&session.session_key, "svc")
        .await
        .unwrap();
    assert_eq!(relation.services.len(), 1);
    assert_eq!(relation.services[0].service_original_name, "svc");

    let services = store
        .list_session_services(&session.session_key)
        .await
        .unwrap();
    assert_eq!(services.len(), 1);

    let relation = store
        .unbind_service_from_session(&session.session_key, "svc")
        .await
        .unwrap();
    assert!(relation.services.is_empty());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn set_session_tool_visibility_uses_allowlist() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    register_tool_service(&store, "svc", &["echo"]).await;
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();

    let visibility = store
        .set_session_tool_visibility(
            &session.session_key,
            vec![SessionToolSelection {
                service_name: "svc".to_string(),
                tool_name: "echo".to_string(),
            }],
        )
        .await
        .unwrap();

    assert_eq!(visibility.mode, ToolVisibilityMode::Allowlist);
    assert_eq!(visibility.tools.len(), 1);
    assert_eq!(visibility.tools[0].tool_original_name, "echo");
    let tools = store
        .list_session_tools(&session.session_key)
        .await
        .unwrap();
    assert_eq!(tools, visibility.tools);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn list_tools_in_unbound_store_session_inherits_store_services() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    register_tool_service(&store, "alpha", &["echo"]).await;
    register_tool_service(&store, "beta", &["search"]).await;
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();

    let tools = store
        .list_tools_in_session(&session.session_key)
        .await
        .unwrap();

    let names = tools.into_iter().map(|tool| tool.name).collect::<Vec<_>>();
    assert_eq!(names, vec!["alpha_echo", "beta_search"]);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn list_tools_in_bound_session_only_returns_bound_services() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    register_tool_service(&store, "alpha", &["echo"]).await;
    register_tool_service(&store, "beta", &["search"]).await;
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();
    store
        .bind_service_to_session(&session.session_key, "beta")
        .await
        .unwrap();

    let tools = store
        .list_tools_in_session(&session.session_key)
        .await
        .unwrap();

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "beta_search");
    assert_eq!(tools[0].service_global_name, "beta");

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn list_tools_in_session_intersects_with_tool_allowlist() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    register_tool_service(&store, "svc", &["echo", "search"]).await;
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();
    store
        .set_session_tool_visibility(
            &session.session_key,
            vec![SessionToolSelection {
                service_name: "svc".to_string(),
                tool_name: "search".to_string(),
            }],
        )
        .await
        .unwrap();

    let tools = store
        .list_tools_in_session(&session.session_key)
        .await
        .unwrap();

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "svc_search");

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn list_tools_in_agent_session_intersects_agent_services() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let alpha = store
        .add_service_for_agent("agent-a", "alpha", stdio_config())
        .await
        .unwrap();
    let beta = store
        .add_service_for_agent("agent-b", "beta", stdio_config())
        .await
        .unwrap();
    for name in [&alpha, &beta] {
        let mut service = store.registry.find_service(name).await.unwrap();
        service.status = ConnectionStatus::Connected;
        service.tools = vec![crate::registry::ToolInfo {
            name: "echo".to_string(),
            description: "echo".to_string(),
            schema: serde_json::json!({"type": "object"}),
        }];
        store.registry.register(service).await;
    }
    let session = store
        .create_session(CreateSessionRequest::agent("s1", "agent-a"))
        .await
        .unwrap();

    let tools = store
        .list_tools_in_session(&session.session_key)
        .await
        .unwrap();

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "alpha_echo");
    assert_eq!(tools[0].service_global_name, alpha);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn closed_session_rejects_session_tool_listing() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    register_tool_service(&store, "svc", &["echo"]).await;
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();
    store
        .close_session(&session.session_key, None)
        .await
        .unwrap();

    let err = store
        .list_tools_in_session(&session.session_key)
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("Session is not active"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn call_tool_in_session_rejects_tools_outside_allowlist() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    register_tool_service(&store, "svc", &["echo", "search"]).await;
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();
    store
        .set_session_tool_visibility(
            &session.session_key,
            vec![SessionToolSelection {
                service_name: "svc".to_string(),
                tool_name: "search".to_string(),
            }],
        )
        .await
        .unwrap();

    let err = store
        .call_tool_in_session(&session.session_key, "svc_echo", serde_json::json!({}))
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("Tool 'svc_echo' not found"));
    let events = store
        .cache()
        .get_all_events_async("session_events")
        .await
        .unwrap();
    assert!(events
        .values()
        .any(|event| event["event_type"] == "call_denied"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn read_resource_in_session_rejects_unbound_service_before_transport() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("alpha", stdio_config()).await.unwrap();
    store.add_service("beta", stdio_config()).await.unwrap();
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();
    store
        .bind_service_to_session(&session.session_key, "alpha")
        .await
        .unwrap();

    let err = store
        .read_resource_in_session(&session.session_key, "file:///secret", Some("beta"))
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("Service not found: beta"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn get_prompt_in_session_rejects_unbound_service_before_transport() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("alpha", stdio_config()).await.unwrap();
    store.add_service("beta", stdio_config()).await.unwrap();
    let session = store
        .create_session(CreateSessionRequest::store("s1"))
        .await
        .unwrap();
    store
        .bind_service_to_session(&session.session_key, "alpha")
        .await
        .unwrap();

    let err = store
        .get_prompt_in_session(
            &session.session_key,
            "summarize",
            serde_json::json!({}),
            Some("beta"),
        )
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("Service not found: beta"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn export_sessions_snapshot_contains_serializable_business_state() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    store.add_service("alpha", stdio_config()).await.unwrap();
    let session = store
        .create_session(CreateSessionRequest::store("exportable"))
        .await
        .unwrap();
    store
        .set_active_session_for_context(SessionScope::Store, None, Some(&session.session_key))
        .await
        .unwrap();
    store
        .bind_service_to_session(&session.session_key, "alpha")
        .await
        .unwrap();

    let snapshot = store.export_sessions_snapshot().await.unwrap();

    assert_eq!(
        snapshot["entities"]["store:global:exportable"]["session_id"],
        "exportable"
    );
    assert_eq!(
        snapshot["relations"]["session_services"]["store:global:exportable"]["services"][0]
            ["service_global_name"],
        "alpha"
    );
    assert_eq!(
        snapshot["states"]["session_status"]["store:global:exportable"]["status"],
        "active"
    );
    assert_eq!(
        snapshot["states"]["session_context"]["store:global"]["active_session_key"],
        "store:global:exportable"
    );
    assert!(snapshot["events"]
        .as_object()
        .unwrap()
        .values()
        .any(|event| {
            event["session_key"] == "store:global:exportable" && event["event_type"] == "create"
        }));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn import_sessions_snapshot_restores_business_state_without_overwrite() {
    let source_path = temp_config_path();
    let target_path = temp_config_path();
    let conflict_path = temp_config_path();
    let source = MCPStore::setup(Some(&source_path)).unwrap();
    source.add_service("alpha", stdio_config()).await.unwrap();
    let session = source
        .session("portable")
        .metadata(serde_json::json!({"owner": "source"}))
        .create()
        .await
        .unwrap();
    source
        .set_active_session_for_context(SessionScope::Store, None, Some(session.session_key()))
        .await
        .unwrap();
    session.bind_service("alpha").await.unwrap();

    let snapshot = source.export_sessions_snapshot().await.unwrap();
    let target = MCPStore::setup(Some(&target_path)).unwrap();
    let report = target
        .import_sessions_snapshot(snapshot.clone())
        .await
        .unwrap();

    assert_eq!(report.sessions_imported, 1);
    assert_eq!(report.session_service_relations_imported, 1);
    assert_eq!(report.session_status_states_imported, 1);
    assert_eq!(report.session_context_states_imported, 1);
    assert!(report.session_events_imported >= 2);
    let restored = target.session("portable").get().await.unwrap().unwrap();
    assert_eq!(restored.entity().await.unwrap().metadata["owner"], "source");
    assert_eq!(
        restored.status().await.unwrap().status,
        SessionStatus::Active
    );
    assert_eq!(
        restored.list_services().await.unwrap()[0].service_global_name,
        "alpha"
    );
    assert_eq!(
        target
            .get_active_session_for_context(SessionScope::Store, None)
            .await
            .unwrap()
            .unwrap()
            .session_key,
        restored.session_key()
    );

    let second_report = target
        .import_sessions_snapshot(snapshot.clone())
        .await
        .unwrap();
    assert_eq!(second_report.sessions_imported, 0);
    assert_eq!(second_report.sessions_unchanged, 1);
    assert_eq!(second_report.session_service_relations_unchanged, 1);
    assert_eq!(second_report.session_status_states_unchanged, 1);
    assert_eq!(second_report.session_context_states_unchanged, 1);

    let conflict = MCPStore::setup(Some(&conflict_path)).unwrap();
    conflict
        .session("portable")
        .metadata(serde_json::json!({"owner": "conflict"}))
        .create()
        .await
        .unwrap();
    let err = conflict
        .import_sessions_snapshot(snapshot)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("session import conflict"));
    assert_eq!(
        conflict
            .session("portable")
            .get()
            .await
            .unwrap()
            .unwrap()
            .entity()
            .await
            .unwrap()
            .metadata["owner"],
        "conflict"
    );

    std::fs::remove_file(source_path).ok();
    std::fs::remove_file(target_path).ok();
    std::fs::remove_file(conflict_path).ok();
}
