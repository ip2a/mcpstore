use std::collections::HashMap;
use std::sync::{Arc, RwLock as SyncRwLock};

use tokio::sync::RwLock;

use crate::event_reactor::{EventBackend, EventReactor};
use crate::identity::InstanceId;
use crate::store::options::{NodeMode, SourceMode};
use crate::store::runtime::StoreRuntimeConfig;

pub(crate) struct RuntimeState {
    pub(crate) namespace: SyncRwLock<String>,
    pub(crate) applied_openapi_configs:
        RwLock<HashMap<InstanceId, serde_json::Map<String, serde_json::Value>>>,
    pub(crate) event_reactor: RwLock<Option<Arc<EventReactor<EventBackend>>>>,
    pub(crate) event_backend: RwLock<Option<EventBackend>>,
    pub(crate) source_mode: SourceMode,
    pub(crate) node_mode: NodeMode,
    pub(crate) runtime_config: StoreRuntimeConfig,
}
