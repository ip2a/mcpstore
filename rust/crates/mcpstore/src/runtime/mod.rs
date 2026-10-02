mod control_panel;
mod data_panel;

pub use control_panel::ControlPanel;
pub use data_panel::DataPanel;

use crate::store::MCPStore;
use crate::{Error, FailureCode, Result};
use std::sync::Arc;

/// 角色组合器：把可选的控制面板/数据面板拼在一个进程里跑。
pub struct Runtime {
    store: Arc<MCPStore>,
    control_panel: Option<ControlPanel>,
    data_panel: Option<DataPanel>,
}

impl Runtime {
    pub fn new(store: Arc<MCPStore>) -> Self {
        Self {
            store,
            control_panel: None,
            data_panel: None,
        }
    }

    /// 启用控制面板（调度与协调）。
    pub fn with_control_panel(mut self) -> Self {
        self.control_panel = Some(ControlPanel::new(self.store.clone()));
        self
    }

    /// 启用数据面板（执行与心跳）。
    pub fn with_data_panel(mut self, node_id: String, capabilities: Vec<String>) -> Self {
        self.data_panel = Some(DataPanel::new(self.store.clone(), node_id, capabilities));
        self
    }

    /// 运行所有已启用的角色，任一退出即返回。
    pub async fn run(self) -> Result<()> {
        let mut tasks = Vec::new();

        if let Some(control) = self.control_panel {
            tasks.push(tokio::spawn(async move { control.run().await }));
        }
        if let Some(data) = self.data_panel {
            tasks.push(tokio::spawn(async move { data.run().await }));
        }
        if tasks.is_empty() {
            return Err(Error::new(
                FailureCode::Internal,
                "no role components enabled; use with_control_panel()/with_data_panel()",
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
    async fn data_panel_heartbeat_reports_capabilities() {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-panel-test-{}.toml",
            std::process::id()
        ));
        let store = MCPStore::setup_with_options(crate::store::StoreOptions {
            node_id: Some("edge-01".to_string()),
            config_path: Some(path.to_string_lossy().into_owned()),
            store: Some(crate::store::JsonStoreConfig::memory()),
            namespace: Some("panel-test".to_string()),
            ..Default::default()
        })
        .unwrap();

        let panel = DataPanel::new(
            store.clone(),
            "edge-01".to_string(),
            vec!["browser".to_string()],
        );
        panel.heartbeat().await.unwrap();

        let status = store
            .read_node_status("edge-01")
            .await
            .unwrap()
            .expect("node_status must be written");
        assert_eq!(status["node"], "edge-01");
        assert_eq!(status["payload"]["capabilities"][0], "browser");
        assert!(status["updated_at"].as_i64().is_some());

        // stop 收尾：清空本进程连接跟踪（空集时也是合法 no-op）
        panel.stop().await;
        assert!(store
            .kernel
            .runtime
            .local_connections
            .read()
            .await
            .is_empty());
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

