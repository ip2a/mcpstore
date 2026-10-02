use crate::store::MCPStore;
use crate::{Error, FailureCode, Result};
use std::sync::Arc;

/// 控制面板：调度与协调角色。
/// 封装控制面的长驻循环（reactor feed loop + 控制请求规则）。
pub struct ControlPanel {
    store: Arc<MCPStore>,
}

impl ControlPanel {
    pub fn new(store: Arc<MCPStore>) -> Self {
        Self { store }
    }

    /// 启动控制循环（幂等：reactor 以 subscriber 身份从保存的游标续读）。
    pub async fn start(&self) -> Result<()> {
        self.store.restart_control_reactor().await
    }

    /// 优雅停止控制循环。
    pub async fn stop(&self) {
        self.store.stop_reactor().await;
    }

    /// 阻塞运行直到 ctrl-c（独立进程形态用）。
    pub async fn run(self) -> Result<()> {
        self.start().await?;
        tokio::signal::ctrl_c()
            .await
            .map_err(|error| Error::new(FailureCode::Internal, format!("ctrl-c: {error}")))?;
        self.stop().await;
        Ok(())
    }
}
