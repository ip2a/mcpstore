use crate::store::MCPStore;
use crate::{Error, FailureCode, Result};
use std::sync::Arc;

/// 控制面板：调度与协调角色。
/// 挂载自愈监督器（keep_alive 断线重连、健康状态机）；未挂载即无自愈。
pub struct ControlPanel {
    store: Arc<MCPStore>,
}

impl ControlPanel {
    pub fn new(store: Arc<MCPStore>) -> Self {
        Self { store }
    }

    /// 挂载控制面自愈循环（幂等）。
    pub fn start(&self) -> Result<()> {
        self.store.attach_control_supervisor()
    }

    /// 停止自愈循环（已建立的连接保持现状，仅停止监督）。
    pub async fn stop(&self) {
        if let Some(supervisor) = self.store.control_supervisor() {
            supervisor.shutdown().await;
        }
    }

    /// 阻塞运行直到 ctrl-c（独立进程形态用）。
    pub async fn run(self) -> Result<()> {
        self.start()
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        tokio::signal::ctrl_c()
            .await
            .map_err(|error| Error::new(FailureCode::Internal, format!("ctrl-c: {error}")))?;
        self.stop().await;
        Ok(())
    }
}
