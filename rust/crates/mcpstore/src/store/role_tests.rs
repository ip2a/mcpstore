//! 面板角色内化测试：panel 是 setup 的参数，角色决定行为。
//!
//! - ControlPanel：setup 即挂载自愈监督器
//! - DataPanel：load 时自动拉取 placement 命中的服务本地建连（无监督器）

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

    // setup 即挂载，无需任何后续调用
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
async fn data_panel_role_serves_placement_on_load() {
    let path = std::env::temp_dir().join(format!("mcpstore-role-data-{}.json", std::process::id()));
    // 先用控制面板角色写配置（带 placement）
    let control = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.to_string_lossy().into_owned()),
        namespace: Some("role-data".to_string()),
        ..Default::default()
    })
    .unwrap();

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/mcpstore/tests/fixtures/execution_mcp_server.py")
        .canonicalize()
        .unwrap();
    let mut config = ServerConfig {
        command: Some("echo".to_string()),
        args: vec!["fixture".to_string()],
        transport: Some("stdio".to_string()),
        ..Default::default()
    };
    config.mcpstore = Some(crate::config::McpStoreExtension {
        scopes: crate::config::ScopeDeclarations::store_only(),
        placement: serde_json::Map::from_iter([(
            "edge-01".to_string(),
            serde_json::json!({"command": "python3", "args": [fixture.to_string_lossy()]}),
        )]),
        ..Default::default()
    });
    control.add_service("svc", config).await.unwrap();

    // 数据面板：panel 是 setup 参数，load 即自动 serve
    let panel = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.to_string_lossy().into_owned()),
        namespace: Some("role-data".to_string()),
        panel: PanelRole::DataPanel {
            panel_id: "edge-01".to_string(),
        },
        ..Default::default()
    })
    .unwrap();

    assert!(
        panel.control_supervisor().is_none(),
        "data panel has no supervisor"
    );
    assert_eq!(panel.panel_id(), "edge-01");
    panel.load_from_source().await.unwrap();

    let instance_id = crate::identity::ServiceInstanceKey::new(
        "svc".to_string(),
        crate::identity::ScopeRef::Store,
    )
    .instance_id();
    let state = panel
        .kernel
        .control
        .state
        .get_display(instance_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.phase, crate::state::RuntimePhase::Running);

    let result = panel
        .call_tool(instance_id, "noop", serde_json::json!({}))
        .await
        .unwrap();
    assert!(!result.is_error);
    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn db_data_panel_write_is_applied_only_by_the_control_panel() {
    let path =
        std::env::temp_dir().join(format!("mcpstore-role-feed-{}.json", uuid::Uuid::new_v4()));
    let namespace = format!("role-feed-{}", uuid::Uuid::new_v4());
    let control = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.to_string_lossy().into_owned()),
        source_mode: SourceMode::Db,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
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

    data.load_from_config().await.unwrap();
    assert!(data
        .kernel
        .control
        .registry
        .find_definition("svc")
        .await
        .is_some());
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
        source_mode: SourceMode::Db,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
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
        source_mode: SourceMode::Db,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
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

    // 查询面全部可用
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

    // 注册表保持空：查询没有触发整表注水
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

#[tokio::test]
async fn db_data_panel_calls_placement_service_locally() {
    let namespace = format!("role-local-call-{}", uuid::Uuid::new_v4());
    let control = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
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
    let placed = {
        let mut config = ServerConfig {
            command: Some("echo".to_string()),
            args: vec!["fixture".to_string()],
            transport: Some("stdio".to_string()),
            ..Default::default()
        };
        config.mcpstore = Some(crate::config::McpStoreExtension {
            scopes: crate::config::ScopeDeclarations::store_only(),
            placement: serde_json::Map::from_iter([(
                "edge-01".to_string(),
                serde_json::json!({"command": "python3", "args": [fixture.to_string_lossy()]}),
            )]),
            ..Default::default()
        });
        config
    };
    let elsewhere = {
        let mut config = placed.clone();
        config.mcpstore = Some(crate::config::McpStoreExtension {
            scopes: crate::config::ScopeDeclarations::store_only(),
            placement: serde_json::Map::from_iter([(
                "edge-02".to_string(),
                serde_json::json!({"command": "python3", "args": [fixture.to_string_lossy()]}),
            )]),
            ..Default::default()
        });
        config
    };
    control.add_service("svc", placed).await.unwrap();
    control.add_service("other", elsewhere).await.unwrap();
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

    // 场景 2：placement 命中本面板 → lazy 建连 + 本地执行
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

    // 场景 3：placement 指向其他面板 → 明确拒绝
    let other_id = crate::identity::ServiceInstanceKey::new(
        "other".to_string(),
        crate::identity::ScopeRef::Store,
    )
    .instance_id();
    let error = data
        .call_tool(other_id, "noop", serde_json::json!({}))
        .await
        .unwrap_err();
    assert_eq!(error.code(), crate::error::FailureCode::ServiceUnavailable);
}

#[tokio::test]
async fn db_data_panel_remote_call_executes_on_control_panel() {
    let namespace = format!("role-remote-call-{}", uuid::Uuid::new_v4());
    let control = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(namespace.clone()),
        panel: PanelRole::ControlPanel,
    })
    .unwrap();
    let data = MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        source_mode: SourceMode::Db,
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

    // 场景 1：placement 为空 → 请求进库，控制面板认领执行，响应写回
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

    // 执行发生在控制面板：连接归它，数据面板没碰执行
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

    // 请求被认领删除，响应被数据面板读走删除：库里不留中间态
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
