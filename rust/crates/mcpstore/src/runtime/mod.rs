mod control_panel;
mod data_panel;

pub use control_panel::ControlPanel;
pub use data_panel::DataPanel;

use std::sync::Arc;

use crate::{Error, FailureCode, MCPStore, Result};

/// 组合角色：控制面板 + 数据面板可选组合，独立后台运行。
pub struct RoleComposer {
    store: Arc<MCPStore>,
    control_panel: Option<ControlPanel>,
    data_panel: Option<DataPanel>,
}

impl RoleComposer {
    pub fn new(store: Arc<MCPStore>) -> Self {
        Self {
            store,
            control_panel: None,
            data_panel: None,
        }
    }

    /// 启用控制面板（决策与自愈监督）。
    pub fn with_control_panel(mut self) -> Self {
        self.control_panel = Some(ControlPanel::new(self.store.clone()));
        self
    }

    /// 启用数据面板（按需执行）。
    pub fn with_data_panel(mut self, panel_id: String) -> Self {
        self.data_panel = Some(DataPanel::new(self.store.clone(), panel_id));
        self
    }

    /// 运行所有已启用的角色，任一退出即返回。
    pub async fn run(self) -> Result<()> {
        let mut tasks = Vec::new();

        if let Some(control) = self.control_panel {
            tasks.push(tokio::spawn(async move { control.run().await }));
        }
        if tasks.is_empty() {
            return Err(Error::new(
                FailureCode::Internal,
                "no role components enabled; use with_control_panel()",
            ));
        }

        for task in tasks {
            task.await
                .map_err(|error| Error::new(FailureCode::Internal, format!("role task join: {error}")))?
                ?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn control_panel_starts_supervision() {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-panel-test-{}.toml",
            std::process::id()
        ));
        let store = MCPStore::setup_with_options(crate::store::StoreOptions {
            config_path: Some(path.to_string_lossy().into_owned()),
            store: Some(crate::store::JsonStoreConfig::memory()),
            namespace: Some("panel-test".to_string()),
            ..Default::default()
        })
        .unwrap();

        let panel = ControlPanel::new(store.clone());
        panel.start().unwrap();
        panel.stop().await;
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn control_panel_start_attaches_supervisor_idempotently() {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-control-panel-test-{}.toml",
            std::process::id()
        ));
        let store = MCPStore::setup_with_options(crate::store::StoreOptions {
            config_path: Some(path.to_string_lossy().into_owned()),
            store: Some(crate::store::JsonStoreConfig::memory()),
            namespace: Some("control-panel-test".to_string()),
            ..Default::default()
        })
        .unwrap();

        // 未挂载即无自愈；ControlPanel 挂载幂等
        assert!(store.control_supervisor().is_none());
        ControlPanel::new(store.clone()).start().unwrap();
        assert!(store.control_supervisor().is_some());
        ControlPanel::new(store.clone()).start().unwrap();
        assert!(store.control_supervisor().is_some());

        ControlPanel::new(store.clone()).stop().await;
        std::fs::remove_file(&path).ok();
    }

    fn stdio_config_with_placement(placement: serde_json::Map<String, serde_json::Value>) -> crate::ServerConfig {
        let mut config = crate::ServerConfig {
            command: Some("echo".to_string()),
            args: vec!["fixture".to_string()],
            transport: Some("stdio".to_string()),
            ..Default::default()
        };
        config.mcpstore = Some(crate::config::McpStoreExtension {
            scopes: crate::config::ScopeDeclarations::store_only(),
            placement,
            ..Default::default()
        });
        config
    }

    #[tokio::test]
    async fn panel_services_hits_only_matching_placement() {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-placement-test-{}.json",
            std::process::id()
        ));
        let store = MCPStore::setup_with_options(crate::store::StoreOptions {
            config_path: Some(path.to_string_lossy().into_owned()),
            store: Some(crate::store::JsonStoreConfig::memory()),
            namespace: Some("placement-test".to_string()),
            ..Default::default()
        })
        .unwrap();

        // svc-a 给 edge-01 覆盖 args；svc-b 给 edge-02；svc-c 无 placement
        store
            .add_service(
                "svc-a",
                stdio_config_with_placement(serde_json::Map::from_iter([(
                    "edge-01".to_string(),
                    serde_json::json!({"args": ["edge-override"]}),
                )])),
            )
            .await
            .unwrap();
        store
            .add_service(
                "svc-b",
                stdio_config_with_placement(serde_json::Map::from_iter([(
                    "edge-02".to_string(),
                    serde_json::json!({"args": ["other"]}),
                )])),
            )
            .await
            .unwrap();
        store
            .add_service("svc-c", stdio_config_with_placement(Default::default()))
            .await
            .unwrap();

        let hits = store.panel_services("edge-01").await.unwrap();
        assert_eq!(hits.len(), 1, "only svc-a hits edge-01");
        assert_eq!(hits[0].service_name, "svc-a");
        assert_eq!(hits[0].merged_config["args"][0], "edge-override");
        assert_eq!(hits[0].merged_config["command"], "echo", "base merges through");

        let none = store.panel_services("edge-99").await.unwrap();
        assert!(none.is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn data_panel_serves_placement_services_locally() {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-panel-serve-{}.json",
            std::process::id()
        ));
        let store = MCPStore::setup_with_options(crate::store::StoreOptions {
            config_path: Some(path.to_string_lossy().into_owned()),
            store: Some(crate::store::JsonStoreConfig::memory()),
            namespace: Some("panel-serve".to_string()),
            ..Default::default()
        })
        .unwrap();

        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/mcpstore/tests/fixtures/execution_mcp_server.py")
            .canonicalize()
            .unwrap();
        let mut config = stdio_config_with_placement(serde_json::Map::from_iter([(
            "edge-01".to_string(),
            serde_json::json!({"command": "python3", "args": [fixture.to_string_lossy()]}),
        )]));
        // base 里的 echo fixture 被 placement diff 覆盖为真 python server
        store.add_service("svc", config).await.unwrap();

        let instance_id = crate::identity::ServiceInstanceKey::new(
            "svc".to_string(),
            crate::identity::ScopeRef::Store,
        )
        .instance_id();

        let panel = DataPanel::new(store.clone(), "edge-01".to_string());
        let connected = panel.serve().await.unwrap();
        assert_eq!(connected, 1);

        // 本地连接建立：实例状态 Running，且可真实调用
        let state = store
            .kernel
            .control
            .state
            .get_display(instance_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.phase, crate::state::RuntimePhase::Running);
        let result = store
            .call_tool(instance_id, "noop", serde_json::json!({}))
            .await
            .unwrap();
        assert!(!result.is_error);

        // 数据面板进程未挂 supervisor（无自愈），但连接可用
        assert!(store.control_supervisor().is_none());
        panel.stop().await;
        std::fs::remove_file(&path).ok();
    }
}
