use std::sync::Arc;

use openkeyv::store::memory::MemoryStore as OpenKeyvMemoryStore;

use crate::cache::openkeyv_store::{OpenKeyvCacheStore, OpenKeyvStoreApi};
use crate::cache::redis::LazyRedisStore;
use crate::cache::Result;

#[async_trait::async_trait]
pub(crate) trait CacheStore: Send + Sync {
    async fn put(&self, key: &str, value: serde_json::Value, collection: &str) -> Result<()>;
    async fn compare_and_put(
        &self,
        key: &str,
        expected_version: Option<u64>,
        value: serde_json::Value,
        collection: &str,
    ) -> Result<()>;
    async fn get(&self, key: &str, collection: &str) -> Result<Option<serde_json::Value>>;
    async fn delete(&self, key: &str, collection: &str) -> Result<()>;
    async fn collections(&self) -> Result<Vec<String>>;
    async fn keys(&self, collection: &str) -> Result<Vec<String>>;
    async fn get_many(
        &self,
        keys: &[String],
        collection: &str,
    ) -> Result<Vec<Option<serde_json::Value>>>;
}

pub(crate) fn memory_cache_store() -> Arc<dyn CacheStore> {
    let inner: Arc<dyn OpenKeyvStoreApi> = Arc::new(OpenKeyvMemoryStore::new());
    Arc::new(OpenKeyvCacheStore::new(inner))
}

/// Like `memory_cache_store`, but also returns the underlying MemoryStore
/// handle (sharing the same `Arc<MemoryClient>`) for use by EventReactor.
pub(crate) fn memory_cache_store_with_handle() -> (Arc<dyn CacheStore>, OpenKeyvMemoryStore) {
    let inner = OpenKeyvMemoryStore::new();
    let api: Arc<dyn OpenKeyvStoreApi> = Arc::new(inner.clone());
    (Arc::new(OpenKeyvCacheStore::new(api)), inner)
}

pub(crate) fn redis_cache_store(redis_url: &str) -> Arc<dyn CacheStore> {
    let api: Arc<dyn OpenKeyvStoreApi> = Arc::new(LazyRedisStore::new(redis_url));
    Arc::new(OpenKeyvCacheStore::new(api))
}
