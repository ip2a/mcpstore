use std::collections::HashMap;

use mcpstore::{
    CreateSessionRequest, MCPStore, McpConfig, ScopeRef, ServerConfig, ServiceInstanceKey,
    ServiceTarget,
};

fn temp_config_path() -> String {
    std::env::temp_dir()
        .join(format!("mcpstore-facade-{}.json", uuid::Uuid::new_v4()))
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
        auth: Default::default(),
        transport: Some("stdio".to_string()),
        working_dir: None,
        description: Some("fixture".to_string()),
        mcpstore: None,
        extra: Default::default(),
    }
}

#[tokio::test]
async fn facade_adds_service_to_current_agent_scope() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let agent_scope = ScopeRef::Agent {
        agent_id: "agent-a".to_string(),
    };

    let instance_id = store
        .for_agent("agent-a")
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();

    assert_eq!(
        instance_id,
        ServiceInstanceKey::new("svc", agent_scope.clone()).instance_id()
    );
    assert_eq!(store.for_store().list_services().await.unwrap().len(), 0);

    let services = store.for_agent("agent-a").list_services().await.unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].instance.service_name, "svc");
    assert_eq!(services[0].instance.scope, agent_scope);
    assert_eq!(services[0].state.instance_id, instance_id);
    assert!(store
        .for_agent("agent-a")
        .list_tools()
        .await
        .unwrap()
        .is_empty());

    let wrong_scope = store
        .for_store()
        .wait_service(ServiceTarget::ServiceName("svc"), std::time::Duration::ZERO)
        .await
        .unwrap_err()
        .to_string();
    assert!(wrong_scope.contains("Scope Store is not declared for service 'svc'"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn facade_add_service_uses_context_scope() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let mut config = McpConfig::default();
    config.mcp_servers.insert("svc".to_string(), stdio_config());

    let instance_ids = store.for_store().add_service(config).await.unwrap();

    assert_eq!(
        instance_ids,
        vec![ServiceInstanceKey::new("svc", ScopeRef::Store).instance_id()]
    );
    let services = store.for_store().list_services().await.unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].instance.service_name, "svc");
    assert_eq!(services[0].instance.scope, ScopeRef::Store);
    assert_eq!(services[0].state.instance_id, instance_ids[0]);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn facade_patch_service_requires_current_scope_and_updates_definition() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();

    store
        .for_agent("agent-a")
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();

    store
        .for_agent("agent-a")
        .patch_service(
            ServiceTarget::ServiceName("svc"),
            serde_json::json!({"headers": {"X-Demo": "agent-a"}}),
        )
        .await
        .unwrap();

    let config = store.show_config().await.unwrap();
    assert_eq!(
        config["mcpServers"]["svc"]["headers"]["X-Demo"],
        serde_json::json!("agent-a")
    );

    let error = store
        .for_store()
        .patch_service(
            ServiceTarget::ServiceName("svc"),
            serde_json::json!({"headers": {"X-Demo": "store"}}),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("Scope Store is not declared for service 'svc'"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn store_facade_reset_config_clears_store() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let context = store.for_store();

    context
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();
    assert_eq!(context.list_services().await.unwrap().len(), 1);

    context.reset_config().await.unwrap();

    assert!(context.list_services().await.unwrap().is_empty());
    assert!(store.show_config().await.unwrap()["mcpServers"]
        .as_object()
        .unwrap()
        .is_empty());
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn agent_facade_reset_config_only_clears_current_agent() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();

    store
        .for_agent("agent-a")
        .add_service_config("svc-a", stdio_config())
        .await
        .unwrap();
    store
        .for_agent("agent-b")
        .add_service_config("svc-b", stdio_config())
        .await
        .unwrap();

    store.for_agent("agent-a").reset_config().await.unwrap();

    assert!(store
        .for_agent("agent-a")
        .list_services()
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .for_agent("agent-b")
            .list_services()
            .await
            .unwrap()
            .len(),
        1
    );
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn facade_show_config_uses_current_scope_and_session_scope() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();

    store
        .for_store()
        .add_service_config("store-only", stdio_config())
        .await
        .unwrap();
    store
        .for_agent("agent-a")
        .add_service_config("agent-only", stdio_config())
        .await
        .unwrap();

    let store_config = store.for_store().show_config().await.unwrap();
    assert!(store_config["mcpServers"].get("store-only").is_some());
    assert!(store_config["mcpServers"].get("agent-only").is_none());

    let agent_config = store.for_agent("agent-a").show_config().await.unwrap();
    assert!(agent_config["mcpServers"].get("store-only").is_none());
    assert!(agent_config["mcpServers"].get("agent-only").is_some());

    let session = store
        .create_session(CreateSessionRequest::agent("session-a", "agent-a"))
        .await
        .unwrap();
    let session_config = store
        .show_session_config(&session.session_key)
        .await
        .unwrap();
    assert_eq!(session_config, agent_config);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn facade_service_lifecycle_stays_in_current_scope() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let store_instance = store
        .for_store()
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();
    let agent_instance = store
        .for_agent("agent-a")
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();

    store
        .for_agent("agent-a")
        .patch_service(
            ServiceTarget::InstanceId(agent_instance),
            serde_json::json!({"headers": {"X-Demo": "agent-a"}}),
        )
        .await
        .unwrap();
    store
        .for_agent("agent-a")
        .update_service(ServiceTarget::ServiceName("svc"), stdio_config())
        .await
        .unwrap();

    store
        .for_store()
        .disconnect_service(ServiceTarget::ServiceName("svc"))
        .await
        .unwrap();
    let wrong_scope = store
        .for_agent("agent-a")
        .restart_service(ServiceTarget::InstanceId(store_instance))
        .await
        .unwrap_err()
        .to_string();
    assert!(wrong_scope.contains("does not belong to scope"));

    store
        .for_agent("agent-a")
        .remove_service(ServiceTarget::InstanceId(agent_instance))
        .await
        .unwrap();
    assert!(store
        .for_agent("agent-a")
        .list_services()
        .await
        .unwrap()
        .is_empty());
    assert_eq!(store.for_store().list_services().await.unwrap().len(), 1);

    std::fs::remove_file(path).ok();
}
