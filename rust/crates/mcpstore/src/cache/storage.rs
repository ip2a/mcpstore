use std::sync::{Arc, OnceLock};

#[cfg(feature = "redis")]
use crate::cache::redis::LazyRedisStore;
use crate::cache::Result;
use openkeyv::store::memory::MemoryStore as OpenKeyvMemoryStore;

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
    /// Atomic take (get + compare_and_delete): exactly one caller gets the value, the rest get None.
    async fn claim(&self, key: &str, collection: &str) -> Result<Option<serde_json::Value>>;
    async fn collections(&self) -> Result<Vec<String>>;
    async fn keys(&self, collection: &str) -> Result<Vec<String>>;
    async fn get_many(
        &self,
        keys: &[String],
        collection: &str,
    ) -> Result<Vec<Option<serde_json::Value>>>;
}

pub(crate) fn memory_cache_store() -> Arc<dyn CacheStore> {
    let store = Arc::new(OpenKeyvMemoryStore::new());
    let handle = openkeyv::StoreHandle::with_capabilities(
        store.clone(),
        Some(store.clone()),
        Some(store.clone()),
        Some(store.clone()),
        Some(store),
    );
    Arc::new(crate::cache::live_store::LiveStore::from_handle(handle))
}

/// Like `memory_cache_store`, but also returns the underlying MemoryStore
/// handle (sharing the same `Arc<MemoryClient>`) for use by online migration.
pub(crate) fn memory_cache_store_with_handle() -> (Arc<dyn CacheStore>, OpenKeyvMemoryStore) {
    cache_store_from_memory(OpenKeyvMemoryStore::new())
}

/// `memory-test-shared` used to get a fresh store per setup, so two in-process panels couldn't meet on a ChangeFeed.
/// ponytail: one store per process. Only for this test name; the real memory() still creates a new one each time.
pub(crate) fn shared_memory_cache_store_with_handle() -> (Arc<dyn CacheStore>, OpenKeyvMemoryStore)
{
    static SHARED: OnceLock<OpenKeyvMemoryStore> = OnceLock::new();
    cache_store_from_memory(SHARED.get_or_init(OpenKeyvMemoryStore::new).clone())
}

fn cache_store_from_memory(
    inner: OpenKeyvMemoryStore,
) -> (Arc<dyn CacheStore>, OpenKeyvMemoryStore) {
    let store = Arc::new(inner.clone());
    let handle = openkeyv::StoreHandle::with_capabilities(
        store.clone(),
        Some(store.clone()),
        Some(store.clone()),
        Some(store.clone()),
        Some(store),
    );
    (
        Arc::new(crate::cache::live_store::LiveStore::from_handle(handle)),
        inner,
    )
}

#[cfg(feature = "redis")]
pub(crate) fn redis_store(redis_address: &str, keyspace: &str) -> Arc<dyn CacheStore> {
    let store = Arc::new(LazyRedisStore::new(redis_address, keyspace));
    let handle = openkeyv::StoreHandle::with_capabilities(
        store.clone(),
        Some(store.clone()),
        Some(store.clone()),
        Some(store),
        None,
    );
    Arc::new(crate::cache::live_store::LiveStore::from_handle(handle))
}
