use crate::store::MCPStore;
use crate::Result;
use std::sync::Arc;
use std::time::Duration;

/// 数据面板：执行与心跳角色。
/// 定期把心跳+能力自报写进共享存储的 node_status 行（updated_at 即存活信号），
/// 读侧（控制面）按时间戳判失联，沉默即异常。
pub struct DataPanel {
    store: Arc<MCPStore>,
    node_id: String,
    capabilities: Vec<String>,
}

impl DataPanel {
    pub fn new(store: Arc<MCPStore>, node_id: String, capabilities: Vec<String>) -> Self {
        Self {
            store,
            node_id,
            capabilities,
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    /// 写一次心跳。
    pub async fn heartbeat(&self) -> Result<()> {
        self.store
            .write_node_status(serde_json::json!({
                "capabilities": self.capabilities,
            }))
            .await
    }

    /// 心跳循环：15s 一跳，直到进程退出。
    pub async fn run(&self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            if let Err(error) = self.heartbeat().await {
                tracing::warn!("[DATA_PANEL] node status heartbeat failed: {error}");
            }
        }
    }

    /// 后台运行（daemon/边缘宿主 spawn 用），句柄可 abort 以停止心跳。
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await.ok();
        })
    }

    /// 收尾：只清理本进程启动的传输（不动其他节点拥有的连接）。
    pub async fn stop(&self) {
        self.store.close_local_connections().await;
    }
}
