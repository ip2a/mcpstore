//! Panel-role internalization tests: panel is a setup parameter; the role decides behavior.
//!
//! - ControlPanel: supervisor mounted at setup; load reads the seed (mcp.json wins at boot)
//! - DataPanel: load is a no-op; queries read kv directly; calls route per the panel's local
//!   `[placement]` config — default local, explicit `"delegate"` goes through the shared store

use crate::store::prelude::*;
use crate::store::{JsonStoreConfig, MCPStore, PanelRole, StoreOptions};

#[tokio::test]
async fn control_panel_role_attaches_supervisor_at_setup() {
    let path =
        std::env::temp_dir().join(format!("mcpstore-role-control-{}.json", std::process::id()));
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.to_string_lossy().into_owned()),
        store: Some(JsonStoreConfig::memory()),
        namespace: Some("role-control".to_string()),
        panel: PanelRole::ControlPanel,
        ..Default::default()
    })
    .unwrap();

    // mounted at setup, no further calls needed
    assert!(store.control_supervisor().is_some());
    assert_eq!(*store.panel_role(), PanelRole::ControlPanel);
    assert_eq!(store.panel_id(), "control");
    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn default_role_is_control_panel() {
    let path =
        std::env::temp_dir().join(format!("mcpstore-role-default-{}.json", std::process::id()));
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.to_string_lossy().into_owned()),
        store: Some(JsonStoreConfig::memory()),
        namespace: Some("role-default".to_string()),
        ..Default::default()
    })
    .unwrap();

    assert!(
        store.control_supervisor().is_some(),
        "default = control panel"
    );
    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn db_data_panel_write_is_applied_only_by_the_control_panel() {
    let path =
        std::env::temp_dir().join(format!("mcpstore-role-feed-{}.json", uuid::Uuid::new_v4()));
    let namespace = format!("role-feed-{}", uuid::Uuid::new_v4());
    let control = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.to_string_lossy().into_owned()),
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace),
        panel: PanelRole::DataPanel {
            panel_id: "edge-01".to_string(),
        },
    })
    .unwrap();
    data.add_service(
        "svc",
        ServerConfig {
            command: Some("echo".to_string()),
            args: vec!["fixture".to_string()],
            transport: Some("stdio".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(data
        .kernel
        .control
        .registry
        .find_definition("svc")
        .await
        .is_none());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let applied = control
            .kernel
            .control
            .registry
            .find_definition("svc")
            .await
            .is_some();
        let synced = control
            .kernel
            .control
            .config_manager
            .load_or_empty()
            .unwrap()
            .mcp_servers
            .contains_key("svc");
        if applied && synced {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("control panel did not apply the data panel add");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(data
        .kernel
        .control
        .registry
        .find_definition("svc")
        .await
        .is_none());
    assert!(data
        .kernel
        .runtime
        .local_connections
        .read()
        .await
        .is_empty());

    // data-panel load is a no-op: queries read kv directly, no registry hydration
    assert!(data.find_definition("svc").await.is_some());
    assert!(data
        .kernel
        .runtime
        .local_connections
        .read()
        .await
        .is_empty());

    data.remove_service("svc").await.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let gone = control
            .kernel
            .control
            .registry
            .find_definition("svc")
            .await
            .is_none();
        let file = control
            .kernel
            .control
            .config_manager
            .load_or_empty()
            .unwrap();
        if gone && !file.mcp_servers.contains_key("svc") {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("control panel did not apply the data panel remove");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn db_data_panel_scope_declare_is_applied_by_control_panel() {
    let path =
        std::env::temp_dir().join(format!("mcpstore-role-scope-{}.json", uuid::Uuid::new_v4()));
    let namespace = format!("role-scope-{}", uuid::Uuid::new_v4());
    let control = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.to_string_lossy().into_owned()),
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace),
        panel: PanelRole::DataPanel {
            panel_id: "edge-01".to_string(),
        },
    })
    .unwrap();
    control
        .add_service(
            "svc",
            ServerConfig {
                command: Some("echo".to_string()),
                args: vec!["fixture".to_string()],
                transport: Some("stdio".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if control
            .kernel
            .control
            .registry
            .find_definition("svc")
            .await
            .is_some()
        {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("control panel did not apply the add");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let scope = crate::identity::ScopeRef::Agent {
        agent_id: "agent-1".to_string(),
    };
    let instance_id = data
        .declare_service_scope("svc", &scope, crate::config::ScopeDescriptor::default())
        .await
        .unwrap();
    assert!(data
        .kernel
        .control
        .registry
        .find_instance(instance_id)
        .await
        .is_none());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let applied = control.find_instance(instance_id).await.is_some();
        let synced = control
            .kernel
            .control
            .config_manager
            .load_or_empty()
            .unwrap()
            .mcp_servers
            .get("svc")
            .is_some_and(|server| server.scopes().agents.contains_key("agent-1"));
        if applied && synced {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("control panel did not apply the scope declare");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(data
        .kernel
        .control
        .registry
        .find_instance(instance_id)
        .await
        .is_none());
    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn db_query_reads_kv_without_hydrating_the_registry() {
    let namespace = format!("role-read-{}", uuid::Uuid::new_v4());
    let control = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace),
        panel: PanelRole::DataPanel {
            panel_id: "edge-01".to_string(),
        },
    })
    .unwrap();
    control
        .add_service(
            "svc",
            ServerConfig {
                command: Some("echo".to_string()),
                args: vec!["fixture".to_string()],
                transport: Some("stdio".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if control.find_definition("svc").await.is_some() {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("control panel did not apply the add");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    // the whole query surface is available
    let instance_id = crate::identity::ServiceInstanceKey::new(
        "svc".to_string(),
        crate::identity::ScopeRef::Store,
    )
    .instance_id();
    let instance = data.find_instance(instance_id).await.expect("kv instance");
    assert_eq!(instance.service_name, "svc");
    assert!(data.find_definition("svc").await.is_some());
    assert_eq!(data.list_tools(instance_id).await.unwrap().len(), 0);
    assert_eq!(data.list_instances().await.len(), 1);
    assert_eq!(
        data.show_config().await.unwrap()["mcpServers"]["svc"]["command"],
        "echo"
    );

    // registry stays empty: queries didn't trigger full-table hydration
    assert!(data
        .kernel
        .control
        .registry
        .list_definitions()
        .await
        .is_empty());
    assert!(data
        .kernel
        .control
        .registry
        .list_instances()
        .await
        .is_empty());
}

/// Writes an empty mcp.json plus a `[placement]` config.toml next to it; returns the mcp.json
/// path for `config_path` (the app config is resolved as its sibling).
fn write_panel_config(entries: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir().join(format!("mcpstore-placement-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mcp_path = dir.join("mcp.json");
    std::fs::write(&mcp_path, "{}").unwrap();
    let mut toml = String::from("[placement]\n");
    for (key, value) in entries {
        toml.push_str(&format!("\"{key}\" = \"{value}\"\n"));
    }
    std::fs::write(dir.join("config.toml"), toml).unwrap();
    mcp_path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn db_data_panel_remote_call_executes_on_control_panel() {
    let namespace = format!("role-remote-call-{}", uuid::Uuid::new_v4());
    let control = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    let config_path = write_panel_config(&[("store.svc", "delegate")]);
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(config_path),
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace),
        panel: PanelRole::DataPanel {
            panel_id: "edge-01".to_string(),
        },
    })
    .unwrap();

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/mcpstore/tests/fixtures/execution_mcp_server.py")
        .canonicalize()
        .unwrap();
    let mut config = ServerConfig {
        command: Some("python3".to_string()),
        args: vec![fixture.to_string_lossy().into_owned()],
        transport: Some("stdio".to_string()),
        ..Default::default()
    };
    config.mcpstore = Some(crate::config::McpStoreExtension {
        scopes: crate::config::ScopeDeclarations::store_only(),
        ..Default::default()
    });
    control.add_service("svc", config).await.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if control.find_definition("svc").await.is_some() {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("control panel did not apply the add");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    // Delegate: the request goes into the store, the control panel claims and executes, the response is written back
    let instance_id = crate::identity::ServiceInstanceKey::new(
        "svc".to_string(),
        crate::identity::ScopeRef::Store,
    )
    .instance_id();
    let result = data
        .call_tool(instance_id, "noop", serde_json::json!({}))
        .await
        .unwrap();
    assert!(!result.is_error);

    // Execution happened on the control panel: it owns the connection; the data panel never touched execution
    assert!(control
        .kernel
        .runtime
        .local_connections
        .read()
        .await
        .contains(&instance_id));
    assert!(data
        .kernel
        .runtime
        .local_connections
        .read()
        .await
        .is_empty());

    // Request claimed and deleted, response read away and deleted by the data panel: no intermediate state left in the store
    assert!(data
        .cache()
        .get_all_entities_async("tool_call_requests")
        .await
        .unwrap()
        .is_empty());
    assert!(data
        .cache()
        .get_all_entities_async("tool_call_responses")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn db_data_panel_default_placement_executes_locally() {
    let namespace = format!("role-local-call-{}", uuid::Uuid::new_v4());
    let control = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    // svc is absent from [placement] → the default is local execution; the explicit
    // delegate entry for another service proves the decision is per instance.
    let config_path = write_panel_config(&[("store.other", "delegate")]);
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(config_path),
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace),
        panel: PanelRole::DataPanel {
            panel_id: "edge-01".to_string(),
        },
    })
    .unwrap();

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/mcpstore/tests/fixtures/execution_mcp_server.py")
        .canonicalize()
        .unwrap();
    let config = {
        let mut config = ServerConfig {
            command: Some("python3".to_string()),
            args: vec![fixture.to_string_lossy().into_owned()],
            transport: Some("stdio".to_string()),
            ..Default::default()
        };
        config.mcpstore = Some(crate::config::McpStoreExtension {
            scopes: crate::config::ScopeDeclarations::store_only(),
            ..Default::default()
        });
        config
    };
    control.add_service("svc", config.clone()).await.unwrap();
    control.add_service("other", config).await.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if control.find_definition("svc").await.is_some()
            && control.find_definition("other").await.is_some()
        {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("control panel did not apply the adds");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    // Default local: the data panel pulls the kv config and connects lazily on first call
    let instance_id = crate::identity::ServiceInstanceKey::new(
        "svc".to_string(),
        crate::identity::ScopeRef::Store,
    )
    .instance_id();
    let result = data
        .call_tool(instance_id, "noop", serde_json::json!({}))
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(data
        .kernel
        .runtime
        .local_connections
        .read()
        .await
        .contains(&instance_id));
    assert!(!control
        .kernel
        .runtime
        .local_connections
        .read()
        .await
        .contains(&instance_id));

    // Explicit delegate: execution happens on the control panel, the data panel never connects
    let other_id = crate::identity::ServiceInstanceKey::new(
        "other".to_string(),
        crate::identity::ScopeRef::Store,
    )
    .instance_id();
    let result = data
        .call_tool(other_id, "noop", serde_json::json!({}))
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(control
        .kernel
        .runtime
        .local_connections
        .read()
        .await
        .contains(&other_id));
    assert!(!data
        .kernel
        .runtime
        .local_connections
        .read()
        .await
        .contains(&other_id));

    // Delegated instances have no local execution surface beyond tool calls: fail loudly
    let error = data.list_prompts(other_id).await.unwrap_err();
    assert_eq!(error.code(), crate::error::FailureCode::ServiceUnavailable);
}

#[tokio::test]
async fn data_panel_rejects_process_private_memory_store() {
    let error = MCPStore::setup_with_options(StoreOptions {
        store: Some(JsonStoreConfig::memory()),
        namespace: Some(format!("role-guard-{}", uuid::Uuid::new_v4())),
        panel: PanelRole::DataPanel {
            panel_id: "edge-01".to_string(),
        },
        ..Default::default()
    })
    .err()
    .expect("data panel on private memory store must be rejected");
    assert_eq!(error.code(), crate::error::FailureCode::ConfigInvalid);
}

#[tokio::test]
async fn remove_of_unknown_service_fails_fast() {
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        store: Some(JsonStoreConfig::memory()),
        namespace: Some(format!("role-remove-{}", uuid::Uuid::new_v4())),
        ..Default::default()
    })
    .unwrap();
    let error = store.remove_service("ghost").await.unwrap_err();
    assert_eq!(error.code(), crate::error::FailureCode::ServiceNotFound);
}

#[tokio::test]
async fn valkey_backend_is_accepted_and_wired() {
    // LazyRedisStore connects on first use, so setup needs no live server; the
    // assertion pins that the valkey arm passes setup (KV + CAS + feed wired by
    // the event backend when a real server is reachable).
    let store = MCPStore::setup_with_options(StoreOptions {
        store: Some(JsonStoreConfig::new(
            "valkey",
            serde_json::json!({"url": "redis://127.0.0.1:1/"}),
        )),
        namespace: Some(format!("role-valkey-{}", uuid::Uuid::new_v4())),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(store.current_store_name().await, "valkey");
}
