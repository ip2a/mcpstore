use std::sync::Arc;

use crate::events::EventBus;
use crate::health::supervisor::InstanceSupervisor;
use crate::transport::client::ConnectionPool;

pub(crate) struct ExecutionEngine {
    pub(crate) pool: ConnectionPool,
    pub(crate) supervisor: Option<Arc<InstanceSupervisor>>,
    pub(crate) event_bus: EventBus,
}
