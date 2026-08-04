//! Concrete openkeyv backend for EventReactor.
//!
//! Holds a shared openkeyv store (any implementation of the capabilities the
//! reactor needs) as a trait object. `Arc<dyn EventBackendCap>` is `Clone`, so
//! the newtype satisfies the `EventReactor<S>` bound without per-method match
//! dispatch.

use std::sync::Arc;

use openkeyv::{
    AsyncChangeFeed, AsyncCompareAndSwap, AsyncEnumerateCollections, AsyncEnumerateKeys,
    AsyncKeyValue, Revision, RevisionedValue, Value,
};

/// Aggregate of the openkeyv capabilities the reactor needs. Object-safe
/// (every supertrait is `#[async_trait]` with `Send + Sync` and no generics).
pub trait EventBackendCap:
    AsyncKeyValue
    + AsyncCompareAndSwap
    + AsyncEnumerateKeys
    + AsyncEnumerateCollections
    + AsyncChangeFeed
    + Send
    + Sync
{
}

impl<T> EventBackendCap for T where
    T: AsyncKeyValue
        + AsyncCompareAndSwap
        + AsyncEnumerateKeys
        + AsyncEnumerateCollections
        + AsyncChangeFeed
        + Send
        + Sync
{
}

/// Shared event-capable backend. Cheap to clone (one `Arc` bump).
#[derive(Clone)]
pub struct EventBackend(Arc<dyn EventBackendCap>);

impl EventBackend {
    /// Wrap an existing store handle (e.g. share the cache layer's `MemoryStore`).
    pub fn from_memory<S>(store: S) -> Self
    where
        S: EventBackendCap + 'static,
    {
        Self(Arc::new(store))
    }

    /// Construct a Redis backend, connecting to the given URL.
    #[cfg(feature = "redis")]
    pub async fn from_redis_url(url: &str) -> openkeyv::Result<Self> {
        let store = openkeyv::store::redis::RedisStore::new(url).await?;
        Ok(Self(Arc::new(store)))
    }

    /// Access the underlying capability object (for ad-hoc trait queries).
    pub fn cap(&self) -> &Arc<dyn EventBackendCap> {
        &self.0
    }
}

// The remaining trait impls delegate to the inner trait object.
#[async_trait::async_trait]
impl AsyncKeyValue for EventBackend {
    async fn get(&self, key: &str, collection: Option<&str>) -> openkeyv::Result<Option<Value>> {
        self.0.get(key, collection).await
    }
    async fn ttl(
        &self,
        key: &str,
        collection: Option<&str>,
    ) -> openkeyv::Result<Option<(Value, Option<f64>)>> {
        self.0.ttl(key, collection).await
    }
    async fn put(
        &self,
        key: &str,
        value: Value,
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> openkeyv::Result<()> {
        self.0.put(key, value, collection, ttl).await
    }
    async fn delete(&self, key: &str, collection: Option<&str>) -> openkeyv::Result<bool> {
        self.0.delete(key, collection).await
    }
    async fn get_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> openkeyv::Result<Vec<Option<Value>>> {
        self.0.get_many(keys, collection).await
    }
    async fn ttl_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> openkeyv::Result<Vec<Option<(Value, Option<f64>)>>> {
        self.0.ttl_many(keys, collection).await
    }
    async fn put_many(
        &self,
        keys: &[String],
        values: &[Value],
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> openkeyv::Result<()> {
        self.0.put_many(keys, values, collection, ttl).await
    }
    async fn delete_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> openkeyv::Result<usize> {
        self.0.delete_many(keys, collection).await
    }
}

#[async_trait::async_trait]
impl AsyncCompareAndSwap for EventBackend {
    async fn get_with_revision(
        &self,
        key: &str,
        collection: Option<&str>,
    ) -> openkeyv::Result<Option<RevisionedValue>> {
        self.0.get_with_revision(key, collection).await
    }
    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&Revision>,
        value: Value,
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> openkeyv::Result<openkeyv::CompareAndSwapResult> {
        self.0
            .compare_and_swap(key, expected, value, collection, ttl)
            .await
    }
    async fn compare_and_delete(
        &self,
        key: &str,
        expected: &Revision,
        collection: Option<&str>,
    ) -> openkeyv::Result<openkeyv::CompareAndDeleteResult> {
        self.0.compare_and_delete(key, expected, collection).await
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateKeys for EventBackend {
    async fn keys(
        &self,
        collection: Option<&str>,
        limit: Option<usize>,
    ) -> openkeyv::Result<Vec<String>> {
        self.0.keys(collection, limit).await
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateCollections for EventBackend {
    async fn collections(&self, limit: Option<usize>) -> openkeyv::Result<Vec<String>> {
        self.0.collections(limit).await
    }
}

#[async_trait::async_trait]
impl AsyncChangeFeed for EventBackend {
    async fn subscribe(
        &self,
        request: openkeyv::ChangeFeedRequest,
    ) -> openkeyv::Result<openkeyv::ChangeSubscription> {
        self.0.subscribe(request).await
    }
}
