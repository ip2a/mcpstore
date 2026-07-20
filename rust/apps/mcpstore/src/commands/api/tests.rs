use super::*;
use mcpstore::{
    cache::models::{
        InstanceToolRelation, ServiceDefinitionEntity, ServiceInstanceEntity, ToolEntity,
    },
    config::{McpStoreExtension, ScopeDeclarations},
    registry::ConfigRevision,
    CacheStorage, ServiceInstanceKey, SourceMode, StoreOptions,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};

static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique_namespace() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    format!("api-session-test-{nanos}")
}

fn unique_temp_dir_path(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let count = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}-{count}", std::process::id()))
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

fn stdio_config_with_lifecycle() -> ServerConfig {
    let mut config = stdio_config();
    config.mcpstore = Some(mcpstore::config::McpStoreExtension {
        scopes: ScopeDeclarations::store_only(),
        lifecycle: Some(mcpstore::config::ServiceLifecycleConfig {
            startup_policy: Some(mcpstore::config::StartupPolicy::Lazy),
            restart_policy: Some(mcpstore::config::RestartPolicy {
                kind: mcpstore::config::RestartPolicyKind::OnFailure,
                max_retries: Some(3),
            }),
        }),
        ..mcpstore::config::McpStoreExtension::default()
    });
    config
}

async fn seed_db_service(store: &MCPStore) {
    let config = stdio_config();
    seed_db_service_config(store, config).await;
}

async fn seed_db_service_config(store: &MCPStore, config: ServerConfig) {
    let cache = store.cache();
    let scope = ScopeRef::Store;
    let instance_id = ServiceInstanceKey::new("demo", scope.clone()).instance_id();
    let base_config = config.base_config();
    let lifecycle = config
        .mcpstore
        .as_ref()
        .and_then(|extension| extension.lifecycle.clone());
    let metadata = config
        .mcpstore
        .as_ref()
        .map(|extension| extension.extra.clone())
        .unwrap_or_default();
    cache
        .put_entity(
            "service_definitions",
            "demo",
            serde_json::to_value(ServiceDefinitionEntity {
                service_name: "demo".to_string(),
                base_config: base_config.clone(),
                scopes: ScopeDeclarations::store_only(),
                lifecycle,
                metadata,
                base_revision: 1,
                added_time: 111,
            })
            .unwrap(),
        )
        .await
        .unwrap();
    cache
        .put_entity(
            "service_instances",
            &instance_id.to_string(),
            serde_json::to_value(ServiceInstanceEntity {
                instance_id,
                service_name: "demo".to_string(),
                scope: scope.clone(),
                transport: "stdio".to_string(),
                url: None,
                command: Some("echo".to_string()),
                effective_config: base_config,
                config_revision: ConfigRevision {
                    base_revision: 1,
                    scope_revision: 1,
                },
                applied_config_revision: None,
                added_time: 111,
            })
            .unwrap(),
        )
        .await
        .unwrap();
    cache
        .put_entity(
            "tools",
            &format!("{instance_id}:echo"),
            serde_json::to_value(ToolEntity {
                instance_id,
                service_name: "demo".to_string(),
                scope: scope.clone(),
                tool_name: "echo".to_string(),
                title: None,
                description: "echo tool".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "text": {"type": "string", "description": "Original text."},
                        "debug": {"type": "boolean"}
                    },
                    "required": ["text", "debug"]
                }),
                output_schema: None,
                annotations: None,
                meta: None,
                created_time: 111,
                tool_hash: "fixture".to_string(),
            })
            .unwrap(),
        )
        .await
        .unwrap();
    cache
        .put_relation(
            "instance_tools",
            &instance_id.to_string(),
            serde_json::to_value(InstanceToolRelation {
                instance_id,
                service_name: "demo".to_string(),
                scope,
                tools: vec!["echo".to_string()],
            })
            .unwrap(),
        )
        .await
        .unwrap();
}

async fn spawn_test_api(store: Arc<MCPStore>) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let (addr, handle, _) = spawn_test_api_with_state(store).await;
    (addr, handle)
}

async fn spawn_test_api_with_state(
    store: Arc<MCPStore>,
) -> (SocketAddr, tokio::task::JoinHandle<()>, Arc<ApiState>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(ApiState {
        store,
        client_changes: Arc::new(Mutex::new(HashMap::new())),
    });
    let app = router(Arc::clone(&state), "");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, handle, state)
}

#[tokio::test]
async fn client_config_routes_preserve_secrets_and_require_expected_hash_for_undo() {
    let path = unique_temp_dir_path("client-config-api").with_extension("json");
    let secret = "api-secret-do-not-return";
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&json!({
            "mcpServers": {
                "existing": {
                    "command": "node",
                    "args": ["server.js"],
                    "env": {"TOKEN": secret},
                    "headers": {"Authorization": "Bearer header-secret"}
                },
                "new": {"command": "python"},
                "conflict": {"command": "node"},
                "unrelated": {"enabled": true}
            },
            "otherSettings": {"keep": true}
        }))
        .unwrap(),
    )
    .unwrap();

    let store_path = unique_temp_dir_path("client-config-api-store").with_extension("json");
    std::fs::write(&store_path, b"{}").unwrap();
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(store_path.to_string_lossy().into_owned()),
        source_mode: SourceMode::Local,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    let (addr, handle, state) = spawn_test_api_with_state(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");
    state
        .store
        .add_service(
            "conflict",
            ServerConfig {
                command: Some("already-owned".to_string()),
                ..ServerConfig::default()
            },
        )
        .await
        .unwrap();
    let entry = json!({
        "name": "aggregate",
        "kind": "aggregate_stdio",
        "config": {"command": "mcpstore", "args": ["mcp-server"]}
    });

    let inspect = client
        .post(format!("{base_url}/client-config/inspect"))
        .json(&json!({"client": "claude_code", "path": path}))
        .send()
        .await
        .unwrap();
    assert!(inspect.status().is_success());
    let inspect_body = inspect.text().await.unwrap();
    assert!(inspect_body.contains("existing"));
    assert!(!inspect_body.contains(secret));
    assert!(!inspect_body.contains("Bearer header-secret"));

    let inspection: Value = serde_json::from_str(&inspect_body).unwrap();
    let hash = inspection["data"]["content_hash"].as_str().unwrap();

    let plan = client
        .post(format!("{base_url}/client-config/plan"))
        .json(&json!({
            "client": "claude_code",
            "path": path,
            "entries": [entry]
        }))
        .send()
        .await
        .unwrap();
    assert!(plan.status().is_success());
    let plan_body = plan.text().await.unwrap();
    assert!(plan_body.contains("new"));
    assert!(!plan_body.contains(secret));
    assert!(!plan_body.contains("Bearer header-secret"));

    let stale_apply = client
        .post(format!("{base_url}/client-config/apply"))
        .json(&json!({
            "client": "claude_code",
            "path": path,
            "expected_hash": "stale-hash",
            "entries": [entry]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stale_apply.status(), axum::http::StatusCode::BAD_REQUEST);

    let apply = client
        .post(format!("{base_url}/client-config/apply"))
        .json(&json!({
            "client": "claude_code",
            "path": path,
            "expected_hash": hash,
            "entries": [entry]
        }))
        .send()
        .await
        .unwrap();
    assert!(apply.status().is_success());
    let apply_payload: Value = apply.json().await.unwrap();
    let change_id = apply_payload["data"]["change_id"].as_str().unwrap();

    let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(written["mcpServers"]["aggregate"]["command"], "mcpstore");
    assert_eq!(written["mcpServers"]["unrelated"]["enabled"], true);
    assert_eq!(written["otherSettings"]["keep"], true);
    assert_eq!(written["mcpServers"]["existing"]["env"]["TOKEN"], secret);

    let undo = client
        .post(format!("{base_url}/client-config/undo"))
        .json(&json!({"change_id": change_id}))
        .send()
        .await
        .unwrap();
    assert!(undo.status().is_success());
    let restored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(restored["mcpServers"].get("aggregate").is_none());
    assert_eq!(restored["mcpServers"]["existing"]["env"]["TOKEN"], secret);

    let partial_import = client
        .post(format!("{base_url}/client-config/import"))
        .json(&json!({
            "client": "claude_code",
            "path": path,
            "names": ["new", "conflict"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(partial_import.status(), axum::http::StatusCode::BAD_REQUEST);
    assert!(state
        .store
        .get_definition_config("new")
        .await
        .unwrap()
        .is_none());

    let import = client
        .post(format!("{base_url}/client-config/import"))
        .json(&json!({
            "client": "claude_code",
            "path": path,
            "names": ["existing"]
        }))
        .send()
        .await
        .unwrap();
    assert!(import.status().is_success());
    let import_body = import.text().await.unwrap();
    assert!(import_body.contains("existing"));
    assert!(!import_body.contains(secret));
    assert!(!import_body.contains("Bearer header-secret"));
    let imported = state
        .store
        .get_definition_config("existing")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(imported["command"], "node");
    assert_eq!(imported["env"]["TOKEN"], secret);

    let unknown = client
        .post(format!("{base_url}/client-config/undo"))
        .json(&json!({"change_id": "missing-change"}))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), axum::http::StatusCode::NOT_FOUND);
    let unknown_body = unknown.text().await.unwrap();
    assert!(unknown_body.contains("CHANGE_NOT_FOUND"));

    handle.abort();
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&store_path);
    let _ = std::fs::remove_file(path.with_file_name(format!(
        ".{}.mcpstore.lock",
        path.file_name().unwrap().to_string_lossy()
    )));
}

#[tokio::test]
async fn oauth_routes_expose_lifecycle_without_echoing_callback_or_credentials() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    let config = ServerConfig {
        url: Some("http://127.0.0.1:9/mcp".to_string()),
        auth: serde_json::from_value(json!({
            "type": "oauth_authorization_code",
            "client_id": "api-client",
            "redirect_uri": "http://127.0.0.1:8787/oauth/callback",
            "scopes": ["tools.read"]
        }))
        .unwrap(),
        transport: Some("streamable-http".to_string()),
        ..ServerConfig::default()
    };
    seed_db_service_config(&store, config).await;
    store.load_from_source().await.unwrap();
    let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();
    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    let status = client
        .get(format!("{base_url}/instances/{instance_id}/auth"))
        .send()
        .await
        .unwrap();
    let status_code = status.status();
    let status_body = status.text().await.unwrap();
    assert!(
        status_code.is_success(),
        "status={status_code} body={status_body}"
    );
    let status_payload = serde_json::from_str::<Value>(&status_body).unwrap();
    assert_eq!(status_payload["data"]["auth"]["status"], "unauthenticated");
    assert_eq!(status_payload["data"]["auth"]["flow"], "authorization_code");
    assert_eq!(
        status_payload["data"]["auth"]["scopes"],
        json!(["tools.read"])
    );

    let callback = client
        .get(format!("{base_url}/instances/{instance_id}/auth/callback"))
        .query(&[
            ("code", "sensitive-code"),
            ("state", "sensitive-state"),
            ("iss", "https://issuer.example"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(callback.status(), axum::http::StatusCode::BAD_REQUEST);
    let callback_body = callback.text().await.unwrap();
    assert!(callback_body.contains("AUTH_CALLBACK_REJECTED"));
    assert!(!callback_body.contains("sensitive-code"));
    assert!(!callback_body.contains("sensitive-state"));

    let empty_secret = client
        .post(format!(
            "{base_url}/instances/{instance_id}/auth/client-secret"
        ))
        .json(&json!({"client_secret": ""}))
        .send()
        .await
        .unwrap();
    assert_eq!(empty_secret.status(), axum::http::StatusCode::BAD_REQUEST);
    let secret_body = empty_secret.text().await.unwrap();
    assert!(secret_body.contains("AUTH_CONFIG_INVALID"));
    assert!(!secret_body.contains("client_secret"));

    let empty_key = client
        .post(format!(
            "{base_url}/instances/{instance_id}/auth/private-key"
        ))
        .json(&json!({"private_key_pem": ""}))
        .send()
        .await
        .unwrap();
    assert_eq!(empty_key.status(), axum::http::StatusCode::BAD_REQUEST);
    let key_body = empty_key.text().await.unwrap();
    assert!(key_body.contains("AUTH_CONFIG_INVALID"));
    assert!(!key_body.contains("private_key_pem"));

    handle.abort();
}

#[tokio::test]
async fn session_routes_use_rust_core_session_state_from_shared_cache() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    seed_db_service(&store).await;
    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    let create = client
        .post(format!("{base_url}/sessions/create"))
        .json(&json!({
            "session_id": "api-core-session",
            "lease_seconds": 60,
            "metadata": {"owner": "api-test"},
        }))
        .send()
        .await
        .unwrap();
    assert!(create.status().is_success());
    let create_payload = create.json::<Value>().await.unwrap();
    let session_key = create_payload["data"]["session"]["session_key"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(session_key, "store:api-core-session");
    let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();

    let bind = client
        .post(format!("{base_url}/sessions/bind_service"))
        .json(&json!({"session_key": session_key, "instance_id": instance_id}))
        .send()
        .await
        .unwrap();
    assert!(bind.status().is_success());

    let tools = client
        .get(format!("{base_url}/sessions/list_tools"))
        .query(&[("session_key", session_key.as_str())])
        .send()
        .await
        .unwrap();
    assert!(tools.status().is_success());
    let tools_payload = tools.json::<Value>().await.unwrap();
    assert_eq!(tools_payload["data"]["total"], 1);
    assert_eq!(tools_payload["data"]["tools"][0]["name"], "echo");

    let set_state = client
        .post(format!("{base_url}/sessions/state/set"))
        .json(&json!({
            "session_key": session_key,
            "key": "cursor",
            "value": {"page": 1},
        }))
        .send()
        .await
        .unwrap();
    assert!(set_state.status().is_success());
    let set_state_payload = set_state.json::<Value>().await.unwrap();
    assert_eq!(
        set_state_payload["data"]["state"]["values"]["cursor"]["page"],
        1
    );

    let get_state_value = client
        .get(format!("{base_url}/sessions/state/value"))
        .query(&[("session_key", session_key.as_str()), ("key", "cursor")])
        .send()
        .await
        .unwrap();
    assert!(get_state_value.status().is_success());
    let get_state_value_payload = get_state_value.json::<Value>().await.unwrap();
    assert_eq!(get_state_value_payload["data"]["value"]["page"], 1);

    let list_state = client
        .get(format!("{base_url}/sessions/state/list"))
        .query(&[("session_key", session_key.as_str())])
        .send()
        .await
        .unwrap();
    assert!(list_state.status().is_success());
    let list_state_payload = list_state.json::<Value>().await.unwrap();
    assert_eq!(list_state_payload["data"]["values"]["cursor"]["page"], 1);

    let delete_state = client
        .post(format!("{base_url}/sessions/state/delete/{session_key}"))
        .json(&json!({"key": "cursor"}))
        .send()
        .await
        .unwrap();
    assert!(delete_state.status().is_success());
    let delete_state_payload = delete_state.json::<Value>().await.unwrap();
    assert!(delete_state_payload["data"]["state"]["values"]
        .as_object()
        .unwrap()
        .is_empty());

    let set_answer = client
        .post(format!("{base_url}/sessions/state/set/{session_key}"))
        .json(&json!({"key": "answer", "value": 42}))
        .send()
        .await
        .unwrap();
    assert!(set_answer.status().is_success());

    let clear_state = client
        .post(format!("{base_url}/sessions/state/clear"))
        .json(&json!({"session_key": session_key}))
        .send()
        .await
        .unwrap();
    assert!(clear_state.status().is_success());
    let clear_state_payload = clear_state.json::<Value>().await.unwrap();
    assert!(clear_state_payload["data"]["state"]["values"]
        .as_object()
        .unwrap()
        .is_empty());

    let set_path_clear = client
        .post(format!("{base_url}/sessions/state/set/{session_key}"))
        .json(&json!({"key": "path_clear", "value": true}))
        .send()
        .await
        .unwrap();
    assert!(set_path_clear.status().is_success());

    let clear_state_by_path = client
        .post(format!("{base_url}/sessions/state/clear/{session_key}"))
        .send()
        .await
        .unwrap();
    assert!(clear_state_by_path.status().is_success());
    let clear_state_by_path_payload = clear_state_by_path.json::<Value>().await.unwrap();
    assert!(clear_state_by_path_payload["data"]["state"]["values"]
        .as_object()
        .unwrap()
        .is_empty());

    let close = client
        .post(format!("{base_url}/sessions/close"))
        .json(&json!({"session_key": session_key, "reason": "done"}))
        .send()
        .await
        .unwrap();
    assert!(close.status().is_success());

    let closed_tools = client
        .get(format!("{base_url}/sessions/list_tools"))
        .query(&[("session_key", session_key.as_str())])
        .send()
        .await
        .unwrap();
    assert_eq!(closed_tools.status(), axum::http::StatusCode::CONFLICT);
    let closed_payload = closed_tools.json::<Value>().await.unwrap();
    assert_eq!(closed_payload["errors"][0]["code"], "SESSION_NOT_ACTIVE");

    let closed_set_state = client
        .post(format!("{base_url}/sessions/state/set"))
        .json(&json!({
            "session_key": session_key,
            "key": "after_close",
            "value": true,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(closed_set_state.status(), axum::http::StatusCode::CONFLICT);
    let closed_set_state_payload = closed_set_state.json::<Value>().await.unwrap();
    assert_eq!(
        closed_set_state_payload["errors"][0]["code"],
        "SESSION_NOT_ACTIVE"
    );

    handle.abort();
}

#[tokio::test]
async fn third_party_config_export_requires_instance_id() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    seed_db_service_config(&store, stdio_config_with_lifecycle()).await;
    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    let native = client
        .get(format!("{base_url}/config"))
        .send()
        .await
        .unwrap();
    assert!(native.status().is_success());
    let native_payload = native.json::<Value>().await.unwrap();
    assert!(native_payload["data"]["mcpServers"]["demo"]
        .get("_mcpstore")
        .is_some());

    let missing_instance = client
        .get(format!("{base_url}/config?format=claude"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        missing_instance.status(),
        axum::http::StatusCode::BAD_REQUEST
    );
    let missing_payload = missing_instance.json::<Value>().await.unwrap();
    assert_eq!(
        missing_payload["errors"][0]["code"],
        json!("MISSING_PARAMETER")
    );

    let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();
    let claude = client
        .get(format!(
            "{base_url}/config?format=claude&instance_id={instance_id}"
        ))
        .send()
        .await
        .unwrap();
    assert!(claude.status().is_success());
    let claude_payload = claude.json::<Value>().await.unwrap();
    assert!(claude_payload["data"]["mcpServers"]["demo"]
        .get("_mcpstore")
        .is_none());
    assert_eq!(
        claude_payload["data"]["mcpServers"]["demo"]["command"],
        json!("echo")
    );

    handle.abort();
}

#[tokio::test]
async fn store_update_only_changes_base_config() {
    let fixture_dir = unique_temp_dir_path("mcpstore-api-update");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let config_path = fixture_dir.join("mcp.json");
    let store = MCPStore::setup(Some(config_path.to_str().unwrap())).unwrap();
    store
        .add_service("demo", stdio_config_with_lifecycle())
        .await
        .unwrap();
    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    let update = client
        .put(format!("{base_url}/services/demo"))
        .json(&json!({
            "command": "printf",
            "args": ["updated"],
            "transport": "stdio",
            "description": "updated fixture"
        }))
        .send()
        .await
        .unwrap();
    assert!(update.status().is_success());

    let native = client
        .get(format!("{base_url}/config"))
        .send()
        .await
        .unwrap();
    assert!(native.status().is_success());
    let native_payload = native.json::<Value>().await.unwrap();
    let service = &native_payload["data"]["mcpServers"]["demo"];
    assert_eq!(service["command"], "printf");
    assert_eq!(service["args"], json!(["updated"]));
    assert!(service["_mcpstore"]["scopes"]["store"].is_object());
    assert_eq!(service["_mcpstore"]["lifecycle"]["startup_policy"], "lazy");

    let invalid = client
        .put(format!("{base_url}/services/demo"))
        .json(&json!({
            "command": "printf",
            "_mcpstore": {"scopes": {"agents": {"agent-a": {}}}}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), axum::http::StatusCode::BAD_REQUEST);

    let invalid_null = client
        .put(format!("{base_url}/services/demo"))
        .json(&json!({
            "command": "printf",
            "_mcpstore": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid_null.status(), axum::http::StatusCode::BAD_REQUEST);

    handle.abort();
    std::fs::remove_dir_all(fixture_dir).ok();
}

#[tokio::test]
async fn store_scope_endpoint_declares_scope_for_existing_agent_only_definition() {
    let fixture_dir = unique_temp_dir_path("mcpstore-api-store-scope");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let config_path = fixture_dir.join("mcp.json");
    let store = MCPStore::setup(Some(config_path.to_str().unwrap())).unwrap();
    let mut config = stdio_config();
    let mut scopes = ScopeDeclarations::default();
    scopes
        .agents
        .insert("agent-a".to_string(), ScopeDescriptor::default());
    config.mcpstore = Some(McpStoreExtension {
        scopes,
        ..McpStoreExtension::default()
    });
    store.add_service("demo", config).await.unwrap();

    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");
    let add = client
        .put(format!("{base_url}/services/demo/scopes/store"))
        .json(&json!({
            "config": {
                "command": "printf",
                "args": ["store"],
                "transport": "stdio"
            }
        }))
        .send()
        .await
        .unwrap();
    assert!(add.status().is_success());

    let native = client
        .get(format!("{base_url}/config"))
        .send()
        .await
        .unwrap();
    assert!(native.status().is_success());
    let native_payload = native.json::<Value>().await.unwrap();
    let service = &native_payload["data"]["mcpServers"]["demo"];
    assert!(service["_mcpstore"]["scopes"]["store"].is_object());
    assert!(service["_mcpstore"]["scopes"]["agents"]["agent-a"].is_object());
    assert_eq!(
        service["_mcpstore"]["scopes"]["store"]["config"]["command"],
        "printf"
    );

    handle.abort();
    std::fs::remove_dir_all(fixture_dir).ok();
}

#[tokio::test]
async fn definition_and_scope_routes_are_explicit_and_isolated() {
    let fixture_dir = unique_temp_dir_path("mcpstore-api-explicit-scope-routes");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let config_path = fixture_dir.join("mcp.json");
    let store = MCPStore::setup(Some(config_path.to_str().unwrap())).unwrap();
    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");

    let add_definition = client
        .post(format!("{base_url}/services/demo"))
        .json(&json!({
            "command": "printf",
            "args": ["store"],
            "transport": "stdio",
            "_mcpstore": {"scopes": {"store": {}}}
        }))
        .send()
        .await
        .unwrap();
    assert!(add_definition.status().is_success());

    let declare_agent = client
        .put(format!("{base_url}/services/demo/scopes/agents/agent-a"))
        .json(&json!({"config": {"args": ["agent"]}}))
        .send()
        .await
        .unwrap();
    assert!(declare_agent.status().is_success());

    let store_instances = client
        .get(format!("{base_url}/scopes/store/instances"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let agent_instances = client
        .get(format!("{base_url}/scopes/agents/agent-a/instances"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_ne!(
        store_instances["data"]["services"][0]["instance_id"],
        agent_instances["data"]["services"][0]["instance_id"]
    );

    let remove_agent = client
        .delete(format!("{base_url}/services/demo/scopes/agents/agent-a"))
        .send()
        .await
        .unwrap();
    assert!(remove_agent.status().is_success());

    let store_after_remove = client
        .get(format!("{base_url}/scopes/store/instances"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let agent_after_remove = client
        .get(format!("{base_url}/scopes/agents/agent-a/instances"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(store_after_remove["data"]["total"], 1);
    assert_eq!(agent_after_remove["data"]["total"], 0);

    let legacy_route = client
        .post(format!("{base_url}/for_store/add_service"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(legacy_route.status(), axum::http::StatusCode::NOT_FOUND);

    let remove_definition = client
        .delete(format!("{base_url}/services/demo"))
        .send()
        .await
        .unwrap();
    assert!(remove_definition.status().is_success());

    handle.abort();
    std::fs::remove_dir_all(fixture_dir).ok();
}

#[tokio::test]
async fn session_snapshot_routes_export_and_import_rust_core_state() {
    let source = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    seed_db_service(&source).await;
    let (source_addr, source_handle) = spawn_test_api(source).await;
    let client = reqwest::Client::new();
    let source_base_url = format!("http://{source_addr}");

    let create = client
        .post(format!("{source_base_url}/sessions/create"))
        .json(&json!({
            "session_id": "snapshot-session",
            "lease_seconds": 60,
            "metadata": {"owner": "snapshot-test"},
        }))
        .send()
        .await
        .unwrap();
    assert!(create.status().is_success());
    let create_payload = create.json::<Value>().await.unwrap();
    let session_key = create_payload["data"]["session"]["session_key"]
        .as_str()
        .unwrap()
        .to_string();
    let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();

    let bind = client
        .post(format!("{source_base_url}/sessions/bind_service"))
        .json(&json!({"session_key": session_key, "instance_id": instance_id}))
        .send()
        .await
        .unwrap();
    assert!(bind.status().is_success());

    let set_state = client
        .post(format!("{source_base_url}/sessions/state/set"))
        .json(&json!({
            "session_key": session_key,
            "key": "cursor",
            "value": {"page": 2},
        }))
        .send()
        .await
        .unwrap();
    assert!(set_state.status().is_success());

    let export = client
        .get(format!("{source_base_url}/sessions/snapshot"))
        .send()
        .await
        .unwrap();
    assert!(export.status().is_success());
    let export_payload = export.json::<Value>().await.unwrap();
    let snapshot = export_payload["data"]["snapshot"].clone();
    assert_eq!(
        snapshot["entities"][session_key.as_str()]["metadata"]["owner"],
        "snapshot-test"
    );
    assert_eq!(
        snapshot["states"]["session_state"][session_key.as_str()]["values"]["cursor"]["page"],
        2
    );

    let target = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    seed_db_service(&target).await;
    let (target_addr, target_handle) = spawn_test_api(target).await;
    let target_base_url = format!("http://{target_addr}");

    let import = client
        .post(format!("{target_base_url}/sessions/snapshot/import"))
        .json(&snapshot)
        .send()
        .await
        .unwrap();
    assert!(import.status().is_success());
    let import_payload = import.json::<Value>().await.unwrap();
    assert_eq!(import_payload["data"]["report"]["sessions_imported"], 1);
    assert_eq!(
        import_payload["data"]["report"]["session_state_records_imported"],
        1
    );

    let imported_state = client
        .get(format!("{target_base_url}/sessions/state/value"))
        .query(&[("session_key", session_key.as_str()), ("key", "cursor")])
        .send()
        .await
        .unwrap();
    assert!(imported_state.status().is_success());
    let imported_state_payload = imported_state.json::<Value>().await.unwrap();
    assert_eq!(imported_state_payload["data"]["value"]["page"], 2);

    let import_again = client
        .post(format!("{target_base_url}/sessions/snapshot/import"))
        .json(&snapshot)
        .send()
        .await
        .unwrap();
    assert!(import_again.status().is_success());
    let import_again_payload = import_again.json::<Value>().await.unwrap();
    assert_eq!(
        import_again_payload["data"]["report"]["sessions_unchanged"],
        1
    );

    source_handle.abort();
    target_handle.abort();
}

#[tokio::test]
async fn store_routes_filter_tools_and_manage_tool_policy() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    seed_db_service(&store).await;
    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");
    let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();

    let default_list = client
        .get(format!("{base_url}/instances/{instance_id}/tools"))
        .send()
        .await
        .unwrap();
    assert!(default_list.status().is_success());
    let default_payload = default_list.json::<Value>().await.unwrap();
    assert_eq!(default_payload["data"]["filter"], "available");
    assert_eq!(default_payload["data"]["total"], 1);

    let set_policy = client
        .put(format!("{base_url}/instances/{instance_id}/tool-policy"))
        .json(&json!({"available_tools": []}))
        .send()
        .await
        .unwrap();
    assert!(set_policy.status().is_success());
    assert_eq!(
        set_policy.json::<Value>().await.unwrap()["data"]["policy"]["tools"],
        json!([])
    );

    let available = client
        .get(format!(
            "{base_url}/instances/{instance_id}/tools?filter=available"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(available["data"]["total"], 0);

    let removed = client
        .get(format!(
            "{base_url}/instances/{instance_id}/tools?filter=removed"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(removed["data"]["filter"], "removed");
    assert_eq!(removed["data"]["total"], 1);

    let invalid = client
        .get(format!(
            "{base_url}/instances/{instance_id}/tools?filter=hidden"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), axum::http::StatusCode::BAD_REQUEST);

    let clear = client
        .delete(format!("{base_url}/instances/{instance_id}/tool-policy"))
        .send()
        .await
        .unwrap();
    assert!(clear.status().is_success());

    let restored = client
        .get(format!("{base_url}/instances/{instance_id}/tools"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(restored["data"]["total"], 1);

    handle.abort();
}

#[tokio::test]
async fn store_routes_manage_rust_core_tool_transforms() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    seed_db_service(&store).await;
    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");
    let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();

    let set_transform = client
        .put(format!(
            "{base_url}/instances/{instance_id}/tool_transforms/echo"
        ))
        .json(&json!({
            "display_name": "say",
            "description": "Say text with a stable hidden debug flag.",
            "arguments": [
                {
                    "original_name": "text",
                    "new_name": "message",
                    "hidden": false,
                    "description": "Message to echo."
                },
                {
                    "original_name": "debug",
                    "hidden": true,
                    "default_value": false
                }
            ],
            "tags": ["compat"],
            "enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert!(set_transform.status().is_success());
    let set_payload = set_transform.json::<Value>().await.unwrap();
    assert_eq!(set_payload["data"]["transform"]["display_name"], "say");
    assert_eq!(set_payload["data"]["transform"]["version"], 1);

    let list_tools = client
        .get(format!("{base_url}/instances/{instance_id}/tools"))
        .send()
        .await
        .unwrap();
    assert!(list_tools.status().is_success());
    let list_tools_payload = list_tools.json::<Value>().await.unwrap();
    let tool = &list_tools_payload["data"]["tools"][0];
    assert_eq!(tool["name"], "say");
    assert_eq!(
        tool["input_schema"]["properties"]["message"]["description"],
        "Message to echo."
    );
    assert!(tool["input_schema"]["properties"].get("debug").is_none());
    assert_eq!(tool["input_schema"]["required"], json!(["message"]));

    let get_transform = client
        .get(format!(
            "{base_url}/instances/{instance_id}/tool_transforms/echo"
        ))
        .send()
        .await
        .unwrap();
    assert!(get_transform.status().is_success());
    let get_payload = get_transform.json::<Value>().await.unwrap();
    assert_eq!(get_payload["data"]["transform"]["tool_name"], "echo");

    let list_transforms = client
        .get(format!("{base_url}/tool_transforms"))
        .send()
        .await
        .unwrap();
    assert!(list_transforms.status().is_success());
    let list_payload = list_transforms.json::<Value>().await.unwrap();
    assert_eq!(list_payload["data"]["total"], 1);

    let delete_transform = client
        .delete(format!(
            "{base_url}/instances/{instance_id}/tool_transforms/echo"
        ))
        .send()
        .await
        .unwrap();
    assert!(delete_transform.status().is_success());

    let list_tools_after_delete = client
        .get(format!("{base_url}/instances/{instance_id}/tools"))
        .send()
        .await
        .unwrap();
    assert!(list_tools_after_delete.status().is_success());
    let list_tools_after_delete_payload = list_tools_after_delete.json::<Value>().await.unwrap();
    assert_eq!(
        list_tools_after_delete_payload["data"]["tools"][0]["name"],
        "echo"
    );

    handle.abort();
}

#[tokio::test]
async fn store_routes_manage_rust_core_openapi_imports() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");
    let spec = json!({
        "openapi": "3.0.0",
        "info": {"title": "Inventory", "version": "1.0.0"},
        "components": {
            "securitySchemes": {
                "ApiKeyAuth": {"type": "apiKey", "in": "header", "name": "x-api-key"}
            }
        },
        "security": [{"ApiKeyAuth": []}],
        "paths": {
            "/items": {
                "get": {"operationId": "listItems", "responses": {"200": {"description": "ok"}}},
                "post": {
                    "operationId": "createItem",
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {"name": {"type": "string"}},
                                    "required": ["name"]
                                }
                            }
                        }
                    },
                    "responses": {"201": {"description": "created"}}
                }
            }
        }
    });

    let import_response = client
        .post(format!("{base_url}/openapi_imports/inventory/import"))
        .json(&json!({
            "spec_url": "memory://inventory",
            "spec": spec,
            "timeout_millis": 4100,
            "fetch_timeout_millis": 4200,
            "auth": {"ApiKeyAuth": "secret"}
        }))
        .send()
        .await
        .unwrap();
    assert!(import_response.status().is_success());
    let import_payload = import_response.json::<Value>().await.unwrap();
    assert_eq!(
        import_payload["data"]["import"]["service_name"],
        "inventory"
    );
    assert_eq!(import_payload["data"]["import"]["total_endpoints"], 2);
    assert_eq!(
        import_payload["data"]["import"]["component_types"]["tools"],
        1
    );
    assert_eq!(
        import_payload["data"]["import"]["component_types"]["resources"],
        1
    );
    assert_eq!(import_payload["data"]["import"]["runtime_executable"], true);
    assert_eq!(
        import_payload["data"]["import"]["security_schemes"]["ApiKeyAuth"]["name"],
        "x-api-key"
    );

    let service_response = client
        .get(format!(
            "{base_url}/instances/{}",
            ServiceInstanceKey::new("inventory", ScopeRef::Store).instance_id()
        ))
        .send()
        .await
        .unwrap();
    assert!(service_response.status().is_success());
    let service_payload = service_response.json::<Value>().await.unwrap();
    assert_eq!(
        service_payload["data"]["effective_config"]["openapi_timeout_millis"],
        4100
    );
    assert_eq!(
        service_payload["data"]["effective_config"]["openapi_fetch_timeout_millis"],
        4200
    );

    let get_response = client
        .get(format!("{base_url}/openapi_imports/inventory"))
        .send()
        .await
        .unwrap();
    assert!(get_response.status().is_success());
    let get_payload = get_response.json::<Value>().await.unwrap();
    assert_eq!(
        get_payload["data"]["import"]["spec_info"]["title"],
        "Inventory"
    );

    let list_response = client
        .get(format!("{base_url}/openapi_imports"))
        .send()
        .await
        .unwrap();
    assert!(list_response.status().is_success());
    let list_payload = list_response.json::<Value>().await.unwrap();
    assert_eq!(list_payload["data"]["total"], 1);

    let invalid_timeout = client
            .post(format!("{base_url}/openapi_imports/bundle"))
            .json(&json!({
                "spec_url": "memory://invalid-timeout",
                "spec": {"openapi": "3.0.0", "info": {"title": "Invalid", "version": "1.0.0"}, "paths": {}},
                "fetch_timeout_millis": 0
            }))
            .send()
            .await
            .unwrap();
    assert_eq!(
        invalid_timeout.status(),
        axum::http::StatusCode::BAD_REQUEST
    );
    let invalid_timeout_payload = invalid_timeout.json::<Value>().await.unwrap();
    assert_eq!(
        invalid_timeout_payload["errors"][0]["field"],
        "fetch_timeout_millis"
    );

    handle.abort();
}

#[tokio::test]
async fn store_route_bundles_openapi_without_importing() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    let (addr, handle) = spawn_test_api(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");
    let fixture_dir = unique_temp_dir_path("mcpstore-api");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let schemas_path = fixture_dir.join("schemas.json");
    std::fs::write(
        &schemas_path,
        serde_json::to_vec(&json!({
            "Item": {
                "type": "object",
                "properties": {"id": {"type": "string"}},
                "required": ["id"]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let bundle_response = client
        .post(format!("{base_url}/openapi_imports/bundle"))
        .json(&json!({
            "spec_url": fixture_dir.join("openapi.json").to_string_lossy(),
            "spec": {
                "openapi": "3.0.0",
                "info": {"title": "Inventory", "version": "1.0.0"},
                "paths": {
                    "/items": {
                        "get": {
                            "operationId": "listItems",
                            "responses": {
                                "200": {
                                    "description": "ok",
                                    "content": {
                                        "application/json": {
                                            "schema": {"$ref": "./schemas.json#/Item"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }))
        .send()
        .await
        .unwrap();
    assert!(bundle_response.status().is_success());
    let bundle_payload = bundle_response.json::<Value>().await.unwrap();
    assert_eq!(
        bundle_payload["data"]["bundle"]["paths"]["/items"]["get"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["properties"]["id"]["type"],
        "string"
    );

    let list_response = client
        .get(format!("{base_url}/openapi_imports"))
        .send()
        .await
        .unwrap();
    assert!(list_response.status().is_success());
    let list_payload = list_response.json::<Value>().await.unwrap();
    assert_eq!(list_payload["data"]["total"], 0);

    handle.abort();
    std::fs::remove_dir_all(&fixture_dir).ok();
}

#[tokio::test]
async fn store_route_bundles_openapi_artifact_without_importing() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        backend: Some(CacheStorage::Memory),
        redis_url: None,
        namespace: Some(unique_namespace()),
    })
    .unwrap();
    let (addr, handle, state) = spawn_test_api_with_state(store).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");
    let fixture_dir = unique_temp_dir_path("mcpstore-api");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let schemas_path = fixture_dir.join("schemas.json");
    std::fs::write(
        &schemas_path,
        serde_json::to_vec(&json!({
            "Item": {
                "type": "object",
                "properties": {"id": {"type": "string"}},
                "required": ["id"]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let artifact_response = client
        .post(format!("{base_url}/openapi_imports/bundle_artifact"))
        .json(&json!({
            "spec_url": fixture_dir.join("openapi.json").to_string_lossy(),
            "ref_cache": {"ttl_seconds": 17},
            "spec": {
                "openapi": "3.0.0",
                "info": {"title": "Inventory", "version": "1.0.0"},
                "paths": {
                    "/items": {
                        "get": {
                            "operationId": "listItems",
                            "responses": {
                                "200": {
                                    "description": "ok",
                                    "content": {
                                        "application/json": {
                                            "schema": {"$ref": "./schemas.json#/Item"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }))
        .send()
        .await
        .unwrap();
    assert!(artifact_response.status().is_success());
    let artifact_payload = artifact_response.json::<Value>().await.unwrap();
    let artifact = &artifact_payload["data"]["artifact"];
    assert_eq!(
        artifact["bundle"]["paths"]["/items"]["get"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["properties"]["id"]["type"],
        "string"
    );
    assert_eq!(artifact["documents"].as_array().unwrap().len(), 2);
    assert_eq!(artifact["dependencies"].as_array().unwrap().len(), 1);
    assert_eq!(
        artifact["dependencies"][0]["source_ref"],
        "./schemas.json#/Item"
    );
    assert_eq!(artifact["diagnostics"].as_array().unwrap().len(), 0);
    let states = state
        .store
        .cache()
        .get_all_states_async("openapi_ref_documents")
        .await
        .unwrap();
    assert_eq!(states.len(), 1);
    let cached = states.values().next().unwrap();
    assert_eq!(cached["ttl_seconds"], json!(17));

    let list_response = client
        .get(format!("{base_url}/openapi_imports"))
        .send()
        .await
        .unwrap();
    assert!(list_response.status().is_success());
    let list_payload = list_response.json::<Value>().await.unwrap();
    assert_eq!(list_payload["data"]["total"], 0);

    handle.abort();
    std::fs::remove_dir_all(&fixture_dir).ok();
}
