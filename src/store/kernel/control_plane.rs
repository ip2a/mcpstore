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
    /// Mutex to protect load_from_db against race conditions. Ensures registry.clear()
    /// and re-population are atomic; prevents concurrent queries from seeing empty state.
    pub(crate) refresh_mutex: tokio::sync::Mutex<()>,
}

mod auth;
mod config;
mod connection;
mod mutation;
mod scope;
mod session;
