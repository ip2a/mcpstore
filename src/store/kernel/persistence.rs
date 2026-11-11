mod event_log;
mod swap;

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::kv::KvLayerManager;
use crate::store::store_config::JsonStoreConfig;

pub(crate) use event_log::{EventLog, EventLogSnapshot};

pub(crate) struct PersistenceRouter {
    pub(crate) store_config: RwLock<JsonStoreConfig>,
    pub(crate) kv: Arc<KvLayerManager>,
    pub(crate) event_log: RwLock<Option<EventLog>>,
    /// Bumped on every cutover. Feed consumers hold a `watch::Receiver` and
    /// resubscribe through `event_log_snapshot` when the generation moves,
    /// so a store swap can never leave them subscribed to the abandoned
    /// store.
    pub(crate) event_epoch: tokio::sync::watch::Sender<u64>,
}
