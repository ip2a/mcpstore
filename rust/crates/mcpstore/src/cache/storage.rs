use std::sync::Arc;

use openkeyv::store::memory::MemoryStore as OpenKeyvMemoryStore;
use serde_json::Value;

use crate::cache::{openkeyv_store::OpenKeyvCacheStore, redis::RedisCacheStore, Result};

#[async_trait::async_trait]
pub(crate) trait CacheStore: Send + Sync {
    async fn put(&self, key: &str, value: Value, collection: &str) -> Result<()>;
    async fn compare_and_put(
        &self,
        key: &str,
        expected_version: Option<u64>,
        value: Value,
        collection: &str,
    ) -> Result<()>;
    async fn get(&self, key: &str, collection: &str) -> Result<Option<Value>>;
    async fn delete(&self, key: &str, collection: &str) -> Result<()>;
    async fn collections(&self) -> Result<Vec<String>>;
    async fn keys(&self, collection: &str) -> Result<Vec<String>>;
    async fn get_many(&self, keys: &[String], collection: &str) -> Result<Vec<Option<Value>>>;
}

pub(crate) fn memory_cache_store() -> Arc<dyn CacheStore> {
    Arc::new(OpenKeyvCacheStore::new(OpenKeyvMemoryStore::new()))
}

pub(crate) fn redis_cache_store(redis_url: &str) -> Arc<dyn CacheStore> {
    Arc::new(RedisCacheStore::new(redis_url))
}
