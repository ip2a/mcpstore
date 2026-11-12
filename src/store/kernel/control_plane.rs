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
    /// Mutex to serialize load_from_db passes (startup hydration, manual reload,
    /// periodic reconcile) against each other; readers stay lock-free — the
    /// registry swap is their atomicity barrier.
    pub(crate) refresh_mutex: tokio::sync::Mutex<()>,
}

mod auth;
mod config;
mod connection;
mod mutation;
mod scope;
mod session;
