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
async fn context_mutations_return_context_and_queries_return_service_objects() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let context = store
        .for_agent("agent-a")
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();

    assert_eq!(
        context.scope(),
        &ScopeRef::Agent {
            agent_id: "agent-a".to_string()
        }
    );
    assert!(store.for_store().list_services().await.unwrap().is_empty());

    let service = context
        .find_service(ServiceTarget::ServiceName("svc"))
        .await
        .unwrap();
    let info = service.info().await.unwrap();
    assert_eq!(info["service_name"], "svc");
    assert_eq!(
        info["scope"],
        serde_json::json!({"type": "agent", "agent_id": "agent-a"})
    );
    assert_eq!(
        info["instance_id"],
        ServiceInstanceKey::new(
            "svc",
            ScopeRef::Agent {
                agent_id: "agent-a".to_string()
            }
        )
        .instance_id()
        .to_string()
    );
    assert!(context.list_tools().await.unwrap().is_empty());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn add_service_returns_context_without_implicitly_selecting_resources() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let mut config = McpConfig::default();
    config.mcp_servers.insert("svc".to_string(), stdio_config());

    let context = store.for_store().add_service(config).await.unwrap();
    assert_eq!(context.scope(), &ScopeRef::Store);
    let services = context.list_services().await.unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].info().await.unwrap()["service_name"], "svc");

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn service_and_context_mutations_keep_their_respective_resource_types() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let context = store
        .for_agent("agent-a")
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();

    let context = context
        .patch_service(
            ServiceTarget::ServiceName("svc"),
            serde_json::json!({"headers": {"X-Demo": "agent-a"}}),
        )
        .await
        .unwrap();
    assert_eq!(
        context.scope(),
        &ScopeRef::Agent {
            agent_id: "agent-a".to_string()
        }
    );

    let service = context
        .find_service(ServiceTarget::ServiceName("svc"))
        .await
        .unwrap();
    let service = service
        .patch_service(serde_json::json!({"headers": {"X-Service": "yes"}}))
        .await
        .unwrap();
    assert_eq!(
        service.config().await.unwrap()["headers"]["X-Service"],
        "yes"
    );
    assert_eq!(
        service.state().await.unwrap().instance_id,
        ServiceInstanceKey::new(
            "svc",
            ScopeRef::Agent {
                agent_id: "agent-a".to_string()
            }
        )
        .instance_id()
    );

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn reset_and_remove_return_true_only_after_success() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let context = store
        .for_store()
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();
    let service = context
        .find_service(ServiceTarget::ServiceName("svc"))
        .await
        .unwrap();

    assert!(service.remove_service().await.unwrap());
    assert!(context.list_services().await.unwrap().is_empty());
    assert!(context.reset_config().await.unwrap());
    assert!(store.show_config().await.unwrap()["mcpServers"]
        .as_object()
        .unwrap()
        .is_empty());

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn scope_validation_applies_to_context_and_resource_objects() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let store_context = store
        .for_store()
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();
    let store_id = ServiceInstanceKey::new("svc", ScopeRef::Store).instance_id();
    let agent_context = store
        .for_agent("agent-a")
        .add_service_config("svc", stdio_config())
        .await
        .unwrap();

    let error = agent_context
        .find_service(ServiceTarget::InstanceId(store_id))
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("does not belong to scope"));

    let service = store_context
        .find_service(ServiceTarget::InstanceId(store_id))
        .await
        .unwrap();
    assert!(service.remove_service().await.unwrap());
    assert!(service
        .info()
        .await
        .unwrap_err()
        .to_string()
        .contains("not found"));

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn show_config_keeps_session_as_a_separate_facade() {
    let path = temp_config_path();
    let store = MCPStore::setup(Some(&path)).unwrap();
    let store_context = store
        .for_store()
        .add_service_config("store-only", stdio_config())
        .await
        .unwrap();
    let agent_context = store
        .for_agent("agent-a")
        .add_service_config("agent-only", stdio_config())
        .await
        .unwrap();

    let store_config = store_context.show_config().await.unwrap();
    let agent_config = agent_context.show_config().await.unwrap();
    assert!(store_config["mcpServers"].get("store-only").is_some());
    assert!(agent_config["mcpServers"].get("agent-only").is_some());

    let session = store
        .create_session(CreateSessionRequest::agent("session-a", "agent-a"))
        .await
        .unwrap();
    assert_eq!(
        store
            .show_session_config(&session.session_key)
            .await
            .unwrap(),
        agent_config
    );

    std::fs::remove_file(path).ok();
}
