mod backend;
mod swap;

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::cache::KvLayerManager;
use crate::store::store_config::JsonStoreConfig;

pub(crate) use backend::{EventBackend, EventBackendSnapshot};

pub(crate) struct PersistenceRouter {
    pub(crate) store_config: RwLock<JsonStoreConfig>,
    pub(crate) cache: Arc<KvLayerManager>,
    pub(crate) event_backend: RwLock<Option<EventBackend>>,
    /// Bumped on every cutover. Feed consumers hold a `watch::Receiver` and
    /// resubscribe through `event_backend_snapshot` when the generation moves,
    /// so a backend swap can never leave them subscribed to the abandoned
    /// store.
    pub(crate) event_epoch: tokio::sync::watch::Sender<u64>,
}
