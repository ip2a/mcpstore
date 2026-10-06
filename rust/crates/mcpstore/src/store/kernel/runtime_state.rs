use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::{OnceLock, RwLock as SyncRwLock, Weak};

use tokio::sync::RwLock;

use crate::identity::InstanceId;
use crate::store::options::PanelRole;
use crate::store::runtime::StoreRuntimeConfig;
use crate::store::MCPStore;

pub(crate) struct RuntimeState {
    pub(crate) namespace: SyncRwLock<String>,
    pub(crate) applied_openapi_configs:
        RwLock<HashMap<InstanceId, serde_json::Map<String, serde_json::Value>>>,
    pub(crate) local_connections: RwLock<HashSet<InstanceId>>,
    pub(crate) panel_role: PanelRole,
    pub(crate) runtime_config: StoreRuntimeConfig,
    /// Consume results are flushed back to mcp.json only when config_path was passed explicitly.
    /// `ConfigManager::new()` resolves to the user's real file, so it can't be a default sync target.
    pub(crate) sync_config_file: bool,
    pub(crate) service_event_feed_started: AtomicBool,
    /// Set when the backend has no ChangeFeed: write paths fail fast on it instead of pretending success.
    pub(crate) service_event_feed_failed: AtomicBool,
    pub(crate) tool_call_feed_started: AtomicBool,
    pub(crate) self_weak: OnceLock<Weak<MCPStore>>,
}
