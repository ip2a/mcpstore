use std::sync::Arc;

use tokio::sync::RwLock;

use crate::cache::CacheLayerManager;
use crate::store::store_config::JsonStoreConfig;

pub(crate) struct PersistenceRouter {
    pub(crate) store_config: RwLock<JsonStoreConfig>,
    pub(crate) cache: Arc<CacheLayerManager>,
}
