use std::sync::Arc;

use crate::core::store::MCPStore;

/// 数据面板（Data Panel）：资源受限的懒惰方，按需响应。
///
/// - 不做周期性后台任务（无心跳）
/// - 存活由实际交互证明（call_tool 等操作自然更新状态）
/// - 通过 placement 配置执行哪些服务
pub struct DataPanel {
    store: Arc<MCPStore>,
    panel_id: String,
}

impl DataPanel {
    pub fn new(store: Arc<MCPStore>, panel_id: String) -> Self {
        Self { store, panel_id }
    }

    pub fn panel_id(&self) -> &str {
        &self.panel_id
    }

    /// Tear down transports started by this process only.
    pub async fn stop(&self) {
        // 未来可以在这里加清理逻辑：断开本地建立的连接、清理临时状态等
        tracing::info!(panel_id = %self.panel_id, "数据面板停止");
    }
}
