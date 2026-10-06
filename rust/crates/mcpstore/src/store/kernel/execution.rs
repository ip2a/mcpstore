use std::sync::Arc;

use crate::events::EventBus;
use crate::health::supervisor::InstanceSupervisor;
use crate::transport::client::ConnectionPool;

pub(crate) struct ExecutionEngine {
    pub(crate) pool: ConnectionPool,
    /// Self-healing supervisor mounted by the control panel; not mounted means no self-healing. Created on demand by ControlPanel.
    pub(crate) supervisor: std::sync::OnceLock<Arc<InstanceSupervisor>>,
    pub(crate) event_bus: EventBus,
}
