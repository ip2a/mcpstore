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
}
