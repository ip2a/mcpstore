use std::sync::Arc;

use crate::auth::AuthCoordinator;
use crate::config::ConfigManager;
use crate::registry::ServiceRegistry;
use crate::state::ServiceStateManager;

pub(crate) struct ControlPlane {
    pub(crate) config_manager: ConfigManager,
    pub(crate) registry: ServiceRegistry,
    pub(crate) auth: AuthCoordinator,
    pub(crate) state: Arc<ServiceStateManager>,
}
