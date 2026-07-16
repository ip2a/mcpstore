//! Concrete openkeyv backend for EventReactor.
//!
//! Holds a native openkeyv store instance (MemoryStore or RedisStore) and
//! implements the full capability set required by EventReactor:
//! AsyncChangeFeed + AsyncCompareAndSwap + AsyncKeyValue + Clone.
//!
//! This is separate from the cache layer's `Arc<dyn CacheStore>` (which is
//! type-erased and cannot satisfy Clone or AsyncChangeFeed). The EventBackend
//! is constructed asynchronously and owned by MCPStore as an optional
//! capability — when the configured backend supports ChangeFeed + CAS.

use openkeyv::{
    store::memory::MemoryStore, store::redis::RedisStore, AsyncChangeFeed, AsyncCompareAndSwap,
    AsyncEnumerateCollections, AsyncEnumerateKeys, AsyncKeyValue, ChangeFeedRequest,
    ChangeSubscription, CompareAndDeleteResult, CompareAndSwapResult, Result, Revision,
    RevisionedValue, Value,
};

/// Concrete event-capable backend wrapping a native openkeyv store.
#[derive(Clone)]
pub enum EventBackend {
    Memory(MemoryStore),
    Redis(RedisStore),
}

impl EventBackend {
    /// Wrap an existing MemoryStore handle (shares the same `Arc<MemoryClient>`
    /// as the cache layer, so both see the same data and ChangeFeed events).
    pub fn from_memory(store: MemoryStore) -> Self {
        Self::Memory(store)
    }

    /// Construct a Redis backend, connecting to the given URL. Redis data is
    /// shared across connections naturally via the Redis server.
    pub async fn from_redis_url(url: &str) -> Result<Self> {
        let store = RedisStore::new(url).await?;
        Ok(Self::Redis(store))
    }
}

// Delegate AsyncKeyValue to the inner store.
#[async_trait::async_trait]
impl AsyncKeyValue for EventBackend {
    async fn get(&self, key: &str, collection: Option<&str>) -> Result<Option<Value>> {
        match self {
            Self::Memory(s) => s.get(key, collection).await,
            Self::Redis(s) => s.get(key, collection).await,
        }
    }

    async fn ttl(
        &self,
        key: &str,
        collection: Option<&str>,
    ) -> Result<Option<(Value, Option<f64>)>> {
        match self {
            Self::Memory(s) => s.ttl(key, collection).await,
            Self::Redis(s) => s.ttl(key, collection).await,
        }
    }

    async fn put(
        &self,
        key: &str,
        value: Value,
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> Result<()> {
        match self {
            Self::Memory(s) => s.put(key, value, collection, ttl).await,
            Self::Redis(s) => s.put(key, value, collection, ttl).await,
        }
    }

    async fn delete(&self, key: &str, collection: Option<&str>) -> Result<bool> {
        match self {
            Self::Memory(s) => s.delete(key, collection).await,
            Self::Redis(s) => s.delete(key, collection).await,
        }
    }

    async fn get_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> Result<Vec<Option<Value>>> {
        match self {
            Self::Memory(s) => s.get_many(keys, collection).await,
            Self::Redis(s) => s.get_many(keys, collection).await,
        }
    }

    async fn ttl_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> Result<Vec<Option<(Value, Option<f64>)>>> {
        match self {
            Self::Memory(s) => s.ttl_many(keys, collection).await,
            Self::Redis(s) => s.ttl_many(keys, collection).await,
        }
    }

    async fn put_many(
        &self,
        keys: &[String],
        values: &[Value],
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> Result<()> {
        match self {
            Self::Memory(s) => s.put_many(keys, values, collection, ttl).await,
            Self::Redis(s) => s.put_many(keys, values, collection, ttl).await,
        }
    }

    async fn delete_many(&self, keys: &[String], collection: Option<&str>) -> Result<usize> {
        match self {
            Self::Memory(s) => s.delete_many(keys, collection).await,
            Self::Redis(s) => s.delete_many(keys, collection).await,
        }
    }
}

#[async_trait::async_trait]
impl AsyncCompareAndSwap for EventBackend {
    async fn get_with_revision(
        &self,
        key: &str,
        collection: Option<&str>,
    ) -> Result<Option<RevisionedValue>> {
        match self {
            Self::Memory(s) => s.get_with_revision(key, collection).await,
            Self::Redis(s) => s.get_with_revision(key, collection).await,
        }
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&Revision>,
        value: Value,
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> Result<CompareAndSwapResult> {
        match self {
            Self::Memory(s) => s.compare_and_swap(key, expected, value, collection, ttl).await,
            Self::Redis(s) => s.compare_and_swap(key, expected, value, collection, ttl).await,
        }
    }

    async fn compare_and_delete(
        &self,
        key: &str,
        expected: &Revision,
        collection: Option<&str>,
    ) -> Result<CompareAndDeleteResult> {
        match self {
            Self::Memory(s) => s.compare_and_delete(key, expected, collection).await,
            Self::Redis(s) => s.compare_and_delete(key, expected, collection).await,
        }
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateKeys for EventBackend {
    async fn keys(&self, collection: Option<&str>, limit: Option<usize>) -> Result<Vec<String>> {
        match self {
            Self::Memory(s) => s.keys(collection, limit).await,
            Self::Redis(s) => s.keys(collection, limit).await,
        }
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateCollections for EventBackend {
    async fn collections(&self, limit: Option<usize>) -> Result<Vec<String>> {
        match self {
            Self::Memory(s) => s.collections(limit).await,
            Self::Redis(s) => s.collections(limit).await,
        }
    }
}

#[async_trait::async_trait]
impl AsyncChangeFeed for EventBackend {
    async fn subscribe(&self, request: ChangeFeedRequest) -> Result<ChangeSubscription> {
        match self {
            Self::Memory(s) => s.subscribe(request).await,
            Self::Redis(s) => s.subscribe(request).await,
        }
    }
}
