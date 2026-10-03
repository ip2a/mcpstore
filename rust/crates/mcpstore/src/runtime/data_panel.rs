use std::sync::Arc;

use crate::store::MCPStore;
use crate::Result;

/// 数据面板（Data Panel）：资源受限的懒惰方，按需响应。
///
/// - 不做周期性后台任务（无心跳）
/// - 存活由实际交互证明（call_tool 等操作自然更新状态）
/// - 通过服务级 placement 配置决定执行哪些服务：命中本 panel_id 的
///   服务以「base_config + placement diff」合并后的配置本地建连执行
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

    /// 拉取 placement 命中本面板的服务并本地建连执行。
    /// 单个服务失败不阻断其余服务，返回成功建立的数量。
    pub async fn serve(&self) -> Result<usize> {
        let services = self.store.panel_services(&self.panel_id).await?;
        let mut connected = 0;
        for service in services {
            let instance_id = service.instance_id;
            match self
                .store
                .connect_with_local_config(instance_id, service.merged_config)
                .await
            {
                Ok(()) => connected += 1,
                Err(error) => tracing::warn!(
                    panel_id = %self.panel_id,
                    service = %service.service_name,
                    %error,
                    "placement 服务本地执行失败"
                ),
            }
        }
        Ok(connected)
    }

    /// 收尾：只清理本进程启动的传输（不动其他面板拥有的连接）。
    pub async fn stop(&self) {
        self.store.close_local_connections().await;
    }
}
