use std::sync::Arc;

use crate::events::EventBus;
use crate::health::supervisor::InstanceSupervisor;
use crate::transport::client::ConnectionPool;

pub(crate) struct ExecutionEngine {
    pub(crate) pool: ConnectionPool,
    /// 控制面板挂载的自愈监督器；未挂载即无自愈。由 ControlPanel 按需创建。
    pub(crate) supervisor: std::sync::OnceLock<Arc<InstanceSupervisor>>,
    pub(crate) event_bus: EventBus,
}
