use super::*;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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

fn hanging_stdio_config() -> ServerConfig {
    ServerConfig {
        url: None,
        command: Some("sh".to_string()),
        args: vec!["-c".to_string(), "sleep 60".to_string()],
        env: HashMap::new(),
        headers: HashMap::new(),
        transport: Some("stdio".to_string()),
        working_dir: None,
        description: Some("hanging".to_string()),
    }
}

async fn spawn_openapi_http_fixture() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buffer = vec![0; 8192];
                let Ok(size) = socket.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..size]);
                let first_line = request.lines().next().unwrap_or_default();
                let request_lower = request.to_ascii_lowercase();
                let body = if first_line.starts_with("GET /items ") {
                    serde_json::json!({"items": ["apple", "pear"]}).to_string()
                } else if first_line.starts_with("GET /items/sku-1 ") {
                    serde_json::json!({"sku": "sku-1", "name": "apple"}).to_string()
                } else if first_line.starts_with("POST /items ") {
                    serde_json::json!({"created": true, "path": "/items"}).to_string()
                } else if first_line.starts_with("POST /forms/urlencoded ") {
                    if request_lower.contains("content-type: application/x-www-form-urlencoded")
                        && request.contains("name=apple")
                    {
                        serde_json::json!({"received": "urlencoded"}).to_string()
                    } else {
                        serde_json::json!({"error": "bad urlencoded body"}).to_string()
                    }
                } else if first_line.starts_with("POST /forms/multipart ") {
                    if request_lower.contains("content-type: multipart/form-data")
                        && request.contains("name=\"name\"")
                        && request.contains("apple")
                    {
                        serde_json::json!({"received": "multipart"}).to_string()
                    } else {
                        serde_json::json!({"error": "bad multipart body"}).to_string()
                    }
                } else if first_line.starts_with("POST /forms/text ") {
                    if request_lower.contains("content-type: text/plain")
                        && request.contains("plain-body")
                    {
                        serde_json::json!({"received": "text"}).to_string()
                    } else {
                        serde_json::json!({"error": "bad text body"}).to_string()
                    }
                } else {
                    serde_json::json!({"error": first_line}).to_string()
                };
                let status = if body.contains("error") {
                    "404 Not Found"
                } else {
                    "200 OK"
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}")
}

async fn spawn_openapi_auth_http_fixture() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buffer = vec![0; 8192];
                let Ok(size) = socket.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..size]);
                let first_line = request.lines().next().unwrap_or_default();
                let authorized = request.lines().any(|line| {
                    line.split_once(':')
                        .map(|(name, value)| {
                            name.eq_ignore_ascii_case("x-api-key") && value.trim() == "secret"
                        })
                        .unwrap_or(false)
                });
                let (status, body) = if !authorized {
                    (
                        "401 Unauthorized",
                        serde_json::json!({"error": "missing api key"}).to_string(),
                    )
                } else if first_line.starts_with("GET /secure/items ") {
                    (
                        "200 OK",
                        serde_json::json!({"items": ["secured"]}).to_string(),
                    )
                } else if first_line.starts_with("POST /secure/items ") {
                    ("200 OK", serde_json::json!({"created": true}).to_string())
                } else {
                    (
                        "404 Not Found",
                        serde_json::json!({"error": first_line}).to_string(),
                    )
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}")
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
async fn db_source_queues_tool_change_refresh_without_writing_runtime_tools() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some("test-db-source-tool-change-queue".to_string()),
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

    let summary = store
        .list_changed_tools_scoped(None, Some("svc"), true)
        .await
        .unwrap();

    assert!(!summary.changed);
    assert_eq!(summary.trigger, "queued_manual_force");
    assert_eq!(summary.details["queued"], serde_json::json!(true));
    assert_eq!(
        summary.details["queued_services"],
        serde_json::json!(["svc"])
    );
    assert!(store
        .cache()
        .get_relation("service_tools", "svc")
        .await
        .unwrap()
        .is_none());

    let events = store
        .cache()
        .get_all_events_async(CONTROL_REQUEST_EVENT_TYPE)
        .await
        .unwrap();
    let event = events
        .values()
        .find(|event| event["type"] == serde_json::json!("ServiceRefreshToolsRequested"))
        .unwrap();
    assert_eq!(event["payload"]["service_name"], serde_json::json!("svc"));
    assert_eq!(event["payload"]["force_refresh"], serde_json::json!(true));
}

#[tokio::test]
async fn openapi_import_persists_shared_analysis_result() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-import-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Inventory", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/items": {
                "get": { "operationId": "listItems", "summary": "List items" },
                "post": { "operationId": "createItem", "requestBody": { "required": true } }
            },
            "/items/{sku}": {
                "get": {
                    "parameters": [{ "name": "sku", "in": "path", "required": true, "schema": { "type": "string" } }]
                }
            }
        }
    });

    let result = store
        .import_openapi_service_from_spec("inventory", "memory://inventory", spec)
        .await
        .unwrap();

    assert_eq!(result.service_name, "inventory");
    assert_eq!(result.total_endpoints, 3);
    assert_eq!(result.component_types.tools, 1);
    assert_eq!(result.component_types.resources, 1);
    assert_eq!(result.component_types.resource_templates, 1);
    assert!(result.runtime_executable);

    let service = store.find_service("inventory").await.unwrap();
    assert_eq!(service.transport, "openapi");
    assert_eq!(service.status, ConnectionStatus::Connected);

    let tools = store.list_tools("inventory").await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "createItem");

    let call_result = store
        .call_tool(
            "inventory",
            "createItem",
            serde_json::json!({"body": {"sku": "sku-1", "name": "apple"}}),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(text).unwrap()["created"],
        serde_json::json!(true)
    );

    let resources = store.list_resources("inventory").await.unwrap();
    assert_eq!(
        resources[0]["uri"],
        serde_json::json!("openapi://inventory/listItems")
    );
    let resource = store
        .read_resource("inventory", "openapi://inventory/listItems")
        .await
        .unwrap();
    assert!(resource["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("apple"));

    let templates = store.list_resource_templates("inventory").await.unwrap();
    assert_eq!(
        templates[0]["uriTemplate"],
        serde_json::json!("openapi://inventory/get_items_sku/{sku}")
    );
    let templated = store
        .read_resource("inventory", "openapi://inventory/get_items_sku/sku-1")
        .await
        .unwrap();
    assert!(templated["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("sku-1"));
    assert!(store
        .read_resource("inventory", "openapi://inventory/get_items_sku")
        .await
        .unwrap_err()
        .to_string()
        .contains("expected 1 path parameter"));
    assert!(store
        .read_resource("inventory", "openapi://inventory/get_items_sku/sku-1/extra")
        .await
        .unwrap_err()
        .to_string()
        .contains("expected 1 path parameter"));

    let persisted = store
        .get_openapi_import("inventory")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.spec_info.title.as_deref(), Some("Inventory"));
    assert_eq!(store.list_openapi_imports().await.unwrap().len(), 1);

    let inspect = store.cache_inspect().await.unwrap();
    assert_eq!(
        inspect["counts"]["states"]["openapi_imports"],
        serde_json::json!(1)
    );
    assert_eq!(
        inspect["counts"]["events"]["openapi_imports"],
        serde_json::json!(1)
    );
    assert_eq!(
        inspect["counts"]["entities"]["services"],
        serde_json::json!(1)
    );
    assert_eq!(inspect["counts"]["entities"]["tools"], serde_json::json!(1));
}

#[tokio::test]
async fn openapi_tool_http_error_returns_tool_error_without_marking_service_failed() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-http-error-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Inventory", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/items/reject": {
                "post": { "operationId": "rejectItem", "requestBody": { "required": true } }
            }
        }
    });

    store
        .import_openapi_service_from_spec("inventory", "memory://inventory", spec)
        .await
        .unwrap();

    let call_result = store
        .call_tool(
            "inventory",
            "rejectItem",
            serde_json::json!({"body": {"sku": "sku-1"}}),
        )
        .await
        .unwrap();
    assert!(call_result.is_error);
    let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
        panic!("expected text content");
    };
    let payload = serde_json::from_str::<serde_json::Value>(text).unwrap();
    assert_eq!(payload["status"], serde_json::json!(404));
    assert!(payload["body"]["error"]
        .as_str()
        .unwrap()
        .contains("POST /items/reject"));

    let service = store.find_service("inventory").await.unwrap();
    assert_ne!(service.status, ConnectionStatus::Error);
}

#[tokio::test]
async fn openapi_tools_support_common_request_body_media_types() {
    let base_url = spawn_openapi_http_fixture().await;
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-body-media-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Forms", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "paths": {
            "/forms/urlencoded": {
                "post": {
                    "operationId": "submitUrlencoded",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/x-www-form-urlencoded": {
                                "schema": { "type": "object", "properties": { "name": { "type": "string" } } }
                            }
                        }
                    }
                }
            },
            "/forms/multipart": {
                "post": {
                    "operationId": "submitMultipart",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": { "type": "object", "properties": { "name": { "type": "string" } } }
                            }
                        }
                    }
                }
            },
            "/forms/text": {
                "post": {
                    "operationId": "submitText",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "text/plain": { "schema": { "type": "string" } }
                        }
                    }
                }
            },
            "/forms/file": {
                "post": {
                    "operationId": "submitFile",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": { "file": { "type": "string", "format": "binary" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    store
        .import_openapi_service_from_spec("forms", "memory://forms", spec)
        .await
        .unwrap();

    for (tool_name, body, expected) in [
        (
            "submitUrlencoded",
            serde_json::json!({"name": "apple"}),
            "urlencoded",
        ),
        (
            "submitMultipart",
            serde_json::json!({"name": "apple"}),
            "multipart",
        ),
        ("submitText", serde_json::json!("plain-body"), "text"),
    ] {
        let call_result = store
            .call_tool("forms", tool_name, serde_json::json!({"body": body}))
            .await
            .unwrap();
        assert!(!call_result.is_error);
        let crate::transport::ContentItem::Text { text, .. } = &call_result.content[0] else {
            panic!("expected text content");
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(text).unwrap()["received"],
            serde_json::json!(expected)
        );
    }

    assert!(store
        .call_tool(
            "forms",
            "submitFile",
            serde_json::json!({"body": {"file": "ignored"}}),
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("multipart binary field"));
}

#[tokio::test]
async fn openapi_import_options_apply_security_to_tools_and_resources() {
    let base_url = spawn_openapi_auth_http_fixture().await;
    let spec = serde_json::json!({
        "openapi": "3.0.0",
        "info": { "title": "Secure Inventory", "version": "2026.1" },
        "servers": [{ "url": base_url }],
        "components": {
            "securitySchemes": {
                "ApiKeyAuth": { "type": "apiKey", "in": "header", "name": "x-api-key" }
            }
        },
        "security": [{ "ApiKeyAuth": [] }],
        "paths": {
            "/secure/items": {
                "get": { "operationId": "listSecureItems", "summary": "List secure items" },
                "post": { "operationId": "createSecureItem", "requestBody": { "required": true } }
            }
        }
    });

    let missing_auth_store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-auth-missing-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    missing_auth_store
        .import_openapi_service_from_spec("secure", "memory://secure", spec.clone())
        .await
        .unwrap();
    assert!(missing_auth_store
        .call_tool(
            "secure",
            "createSecureItem",
            serde_json::json!({"body": {"sku": "sku-1"}}),
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("missing auth value"));

    let header_store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-auth-header-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    header_store
        .import_openapi_service_from_spec_with_options(
            "secure",
            "memory://secure",
            spec.clone(),
            crate::openapi::OpenApiImportOptions {
                headers: HashMap::from([("x-api-key".to_string(), "secret".to_string())]),
                auth: serde_json::Map::new(),
            },
        )
        .await
        .unwrap();
    assert!(header_store
        .call_tool(
            "secure",
            "createSecureItem",
            serde_json::json!({"body": {"sku": "sku-1"}}),
        )
        .await
        .is_ok());

    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(format!("openapi-auth-{}", uuid::Uuid::new_v4())),
    })
    .unwrap();
    let result = store
        .import_openapi_service_from_spec_with_options(
            "secure",
            "memory://secure",
            spec,
            crate::openapi::OpenApiImportOptions {
                headers: HashMap::new(),
                auth: serde_json::json!({ "ApiKeyAuth": "secret" })
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        )
        .await
        .unwrap();
    assert!(result.security_schemes.contains_key("ApiKeyAuth"));
    assert_eq!(result.security.len(), 1);

    let call_result = store
        .call_tool(
            "secure",
            "createSecureItem",
            serde_json::json!({"body": {"sku": "sku-1"}}),
        )
        .await
        .unwrap();
    assert!(!call_result.is_error);

    let resource = store
        .read_resource("secure", "openapi://secure/listSecureItems")
        .await
        .unwrap();
    assert!(resource["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("secured"));
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
async fn cache_inspect_includes_session_collections() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some("inspect-sessions".to_string()),
    })
    .unwrap();
    let session_key = "store:global:s1";

    store
        .cache()
        .put_entity(
            "sessions",
            session_key,
            serde_json::json!({
                "session_key": session_key,
                "session_id": "s1",
                "scope": "store",
                "agent_id": null,
                "created_at": 100,
                "updated_at": 100,
                "last_active": 100,
                "lease_seconds": null,
                "expires_at": null,
                "version": 1,
                "metadata": {}
            }),
        )
        .await
        .unwrap();
    store
        .cache()
        .put_relation(
            "session_services",
            session_key,
            serde_json::json!({
                "session_key": session_key,
                "services": [],
                "updated_at": 100,
                "version": 1
            }),
        )
        .await
        .unwrap();
    store
        .cache()
        .put_relation(
            "session_tools",
            session_key,
            serde_json::json!({
                "session_key": session_key,
                "mode": "allowlist",
                "tools": [],
                "updated_at": 100,
                "version": 1
            }),
        )
        .await
        .unwrap();
    store
        .cache()
        .put_state(
            "session_status",
            session_key,
            serde_json::json!({
                "session_key": session_key,
                "status": "active",
                "updated_at": 100,
                "version": 1,
                "reason": null
            }),
        )
        .await
        .unwrap();
    store
        .cache()
        .put_event(
            "session_events",
            "store:global:s1:0001",
            serde_json::json!({
                "session_key": session_key,
                "event_type": "create",
                "occurred_at": 100,
                "payload": {}
            }),
        )
        .await
        .unwrap();

    let inspect = store.cache_inspect().await.unwrap();
    assert_eq!(inspect["counts"]["entities"]["sessions"], 1);
    assert_eq!(inspect["counts"]["relations"]["session_services"], 1);
    assert_eq!(inspect["counts"]["relations"]["session_tools"], 1);
    assert_eq!(inspect["counts"]["states"]["session_status"], 1);
    assert_eq!(inspect["counts"]["events"]["session_events"], 1);
    let collections = inspect["collections"].as_array().unwrap();
    for suffix in [
        "entity:sessions",
        "relations:session_services",
        "relations:session_tools",
        "state:session_status",
        "event:session_events",
    ] {
        let expected = format!("inspect-sessions:{suffix}");
        assert!(collections
            .iter()
            .any(|value| value.as_str() == Some(expected.as_str())));
    }

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
async fn tool_transform_rules_are_rust_backed_and_affect_scoped_tools() {
    let path = temp_config_path();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-tool-transform".to_string()),
    })
    .unwrap();
    store.add_service("svc", stdio_config()).await.unwrap();
    let mut service = store.registry.find_service("svc").await.unwrap();
    service.status = ConnectionStatus::Connected;
    service.tools = vec![crate::registry::ToolInfo {
        name: "echo".to_string(),
        description: "Technical echo".to_string(),
        schema: serde_json::json!({
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Raw text"},
                "debug": {"type": "boolean"}
            },
            "required": ["text", "debug"]
        }),
    }];
    store.registry.register(service).await;

    let rule = store
        .set_tool_transform(
            "svc",
            "echo",
            crate::ToolTransformPatch {
                display_name: Some("say".to_string()),
                description: Some("Say text".to_string()),
                arguments: vec![
                    crate::cache::models::ToolArgumentTransform {
                        original_name: "text".to_string(),
                        new_name: Some("message".to_string()),
                        hidden: false,
                        default_value: None,
                        description: Some("Message to say".to_string()),
                    },
                    crate::cache::models::ToolArgumentTransform {
                        original_name: "debug".to_string(),
                        new_name: None,
                        hidden: true,
                        default_value: Some(serde_json::json!(false)),
                        description: None,
                    },
                ],
                tags: vec!["llm-friendly".to_string()],
                enabled: Some(true),
            },
        )
        .await
        .unwrap();

    assert_eq!(rule.tool_global_name, "svc_echo");
    assert!(store
        .cache()
        .get_state("tool_transforms", "svc_echo")
        .await
        .unwrap()
        .is_some());

    let tools = store.list_tool_entries_scoped(None, None).await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "say");
    assert_eq!(tools[0].original_name, "echo");
    assert_eq!(tools[0].description, "Say text");
    assert!(tools[0].schema["properties"].get("debug").is_none());
    assert!(tools[0].schema["properties"].get("message").is_some());
    assert_eq!(tools[0].schema["required"], serde_json::json!(["message"]));

    let resolution = store
        .resolve_tool_for_agent(GLOBAL_AGENT_STORE, "say")
        .await
        .unwrap();
    assert_eq!(resolution.global_service_name, "svc");
    assert_eq!(resolution.canonical_tool_name, "echo");

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
async fn connect_service_times_out_hanging_stdio_startup() {
    let path = temp_config_path();
    let app_path = std::path::Path::new(&path)
        .parent()
        .unwrap()
        .join("config.toml");
    std::fs::write(&app_path, "[health_check]\nstartup_timeout = 1\n").unwrap();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.clone()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::OpenKeyvMemory),
        redis_url: None,
        namespace: Some("test-connect-timeout-status".to_string()),
    })
    .unwrap();
    store
        .add_service("hanging", hanging_stdio_config())
        .await
        .unwrap();

    let err = store
        .connect_service("hanging")
        .await
        .unwrap_err()
        .to_string();
    let status = store
        .cached_service_status("hanging")
        .await
        .unwrap()
        .unwrap();
    let service = store.find_service("hanging").await.unwrap();

    assert!(err.contains("Service connection timed out"));
    assert_eq!(status.health_status, HealthStatus::CircuitOpen);
    assert_eq!(service.status, ConnectionStatus::Error);

    std::fs::remove_file(path).ok();
    std::fs::remove_file(app_path).ok();
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

#[tokio::test]
async fn tool_change_diff_reports_added_removed_and_updated_tools() {
    let old_tools = vec![
        crate::registry::ToolInfo {
            name: "keep".to_string(),
            description: "old description".to_string(),
            schema: serde_json::json!({"type": "object"}),
        },
        crate::registry::ToolInfo {
            name: "remove".to_string(),
            description: String::new(),
            schema: serde_json::json!({"type": "object"}),
        },
    ];
    let new_tools = vec![
        crate::registry::ToolInfo {
            name: "add".to_string(),
            description: String::new(),
            schema: serde_json::json!({"type": "object"}),
        },
        crate::registry::ToolInfo {
            name: "keep".to_string(),
            description: "new description".to_string(),
            schema: serde_json::json!({"type": "object"}),
        },
    ];

    let (added, removed, updated, count) =
        MCPStore::diff_tool_infos_for_test(&old_tools, &new_tools);

    assert_eq!(added, vec!["add"]);
    assert_eq!(removed, vec!["remove"]);
    assert_eq!(updated, vec!["keep"]);
    assert_eq!(count, 3);
}
