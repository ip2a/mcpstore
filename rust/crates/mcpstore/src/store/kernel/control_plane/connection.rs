use crate::store::prelude::*;
use crate::store::{ControlPlane, MCPStore};

impl ControlPlane {
    pub async fn connect_service(&self, store: &MCPStore, instance_id: InstanceId) -> Result<()> {
        // 与执行面同一入口：注水缺则补 / 数据面板 placement 判定都在里面。
        store.ensure_instance_connected(instance_id).await
    }
}

impl MCPStore {
    /// 连接 desired=Running 的实例（keep_alive / OnStoreStart 语义）。
    /// 启动（load 尾部）与事件消费共用这一份，语义不分叉。
    pub(crate) async fn connect_desired_running_instances(&self, service_name: Option<&str>) {
        for instance in self.kernel.control.registry.list_instances().await {
            if service_name.is_some_and(|name| instance.service_name != name) {
                continue;
            }
            let Ok(Some(state)) = self.kernel.control.state.get(instance.instance_id).await else {
                continue;
            };
            if state.desired != crate::state::DesiredState::Running {
                continue;
            }
            if let Err(error) = self
                .connect_service_internal(instance.instance_id, false)
                .await
            {
                tracing::warn!(
                    "[STORE] desired-instance connect failed: {} ({})",
                    instance.instance_id,
                    error
                );
            }
        }
    }
}
