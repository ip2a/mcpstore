use std::sync::Arc;

use openkeyv::{
    store::redis::{ForeignKeyPolicy, RedisConfig, RedisStore as OpenKeyvRedisInner},
    AsyncCompareAndSwap, AsyncEnumerateCollections, AsyncEnumerateKeys, AsyncKeyValue, Revision,
    RevisionedValue, Value,
};
use tokio::sync::OnceCell;

/// Lazily-connected Redis store that fulfils the cache's aggregate trait.
/// Built on first use so Store creation stays synchronous.
pub(in crate::cache) struct LazyRedisStore {
    inner: OnceCell<Arc<OpenKeyvRedisInner>>,
    url: String,
    foreign_key_policy: ForeignKeyPolicy,
}

impl LazyRedisStore {
    pub(in crate::cache) fn new(
        url: impl Into<String>,
        foreign_key_policy: ForeignKeyPolicy,
    ) -> Self {
        Self {
            inner: OnceCell::new(),
            url: url.into(),
            foreign_key_policy,
        }
    }

    async fn handle(&self) -> openkeyv::Result<&Arc<OpenKeyvRedisInner>> {
        self.inner
            .get_or_try_init(|| async {
                let config = RedisConfig {
                    foreign_key_policy: self.foreign_key_policy,
                    ..RedisConfig::default()
                };
                OpenKeyvRedisInner::new_with_config(&self.url, config)
                    .await
                    .map(Arc::new)
            })
            .await
    }
}

#[async_trait::async_trait]
impl AsyncKeyValue for LazyRedisStore {
    async fn get(&self, key: &str, collection: Option<&str>) -> openkeyv::Result<Option<Value>> {
        self.handle().await?.get(key, collection).await
    }
    async fn ttl(
        &self,
        key: &str,
        collection: Option<&str>,
    ) -> openkeyv::Result<Option<(Value, Option<f64>)>> {
        self.handle().await?.ttl(key, collection).await
    }
    async fn put(
        &self,
        key: &str,
        value: Value,
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> openkeyv::Result<()> {
        self.handle().await?.put(key, value, collection, ttl).await
    }
    async fn delete(&self, key: &str, collection: Option<&str>) -> openkeyv::Result<bool> {
        self.handle().await?.delete(key, collection).await
    }
    async fn get_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> openkeyv::Result<Vec<Option<Value>>> {
        self.handle().await?.get_many(keys, collection).await
    }
    async fn ttl_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> openkeyv::Result<Vec<Option<(Value, Option<f64>)>>> {
        self.handle().await?.ttl_many(keys, collection).await
    }
    async fn put_many(
        &self,
        keys: &[String],
        values: &[Value],
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> openkeyv::Result<()> {
        self.handle()
            .await?
            .put_many(keys, values, collection, ttl)
            .await
    }
    async fn delete_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> openkeyv::Result<usize> {
        self.handle().await?.delete_many(keys, collection).await
    }
}

#[async_trait::async_trait]
impl AsyncCompareAndSwap for LazyRedisStore {
    async fn get_with_revision(
        &self,
        key: &str,
        collection: Option<&str>,
    ) -> openkeyv::Result<Option<RevisionedValue>> {
        self.handle()
            .await?
            .get_with_revision(key, collection)
            .await
    }
    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&Revision>,
        value: Value,
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> openkeyv::Result<openkeyv::CompareAndSwapResult> {
        self.handle()
            .await?
            .compare_and_swap(key, expected, value, collection, ttl)
            .await
    }
    async fn compare_and_delete(
        &self,
        key: &str,
        expected: &Revision,
        collection: Option<&str>,
    ) -> openkeyv::Result<openkeyv::CompareAndDeleteResult> {
        self.handle()
            .await?
            .compare_and_delete(key, expected, collection)
            .await
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateKeys for LazyRedisStore {
    async fn keys(
        &self,
        collection: Option<&str>,
        limit: Option<usize>,
    ) -> openkeyv::Result<Vec<String>> {
        self.handle().await?.keys(collection, limit).await
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateCollections for LazyRedisStore {
    async fn collections(&self, limit: Option<usize>) -> openkeyv::Result<Vec<String>> {
        self.handle().await?.collections(limit).await
    }
}
