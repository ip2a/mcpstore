//! 面板角色内化测试：panel 是 setup 的参数，角色决定行为。
//!
//! - ControlPanel：setup 即挂载自愈监督器
//! - DataPanel：load 时自动拉取 placement 命中的服务本地建连（无监督器）

use crate::store::prelude::*;
use crate::store::{JsonStoreConfig, MCPStore, PanelRole, StoreOptions};

#[tokio::test]
async fn control_panel_role_attaches_supervisor_at_setup() {
    let path = std::env::temp_dir().join(format!(
        "mcpstore-role-control-{}.json",
        std::process::id()
    ));
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
    let path = std::env::temp_dir().join(format!(
        "mcpstore-role-default-{}.json",
        std::process::id()
    ));
    let store = MCPStore::setup_with_options(StoreOptions {
        config_path: Some(path.to_string_lossy().into_owned()),
        store: Some(JsonStoreConfig::memory()),
        namespace: Some("role-default".to_string()),
        ..Default::default()
    })
    .unwrap();

    assert!(store.control_supervisor().is_some(), "default = control panel");
    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn data_panel_role_serves_placement_on_load() {
    let path = std::env::temp_dir().join(format!(
        "mcpstore-role-data-{}.json",
        std::process::id()
    ));
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

    assert!(panel.control_supervisor().is_none(), "data panel has no supervisor");
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
