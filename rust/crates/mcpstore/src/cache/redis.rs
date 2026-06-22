use std::collections::HashMap;

use openkeyv::{
    store::redis::RedisStore as OpenKeyvRedisInner, AsyncEnumerateCollections, AsyncEnumerateKeys,
    AsyncKeyValue,
};
use serde_json::Value;
use tokio::sync::OnceCell;

pub(super) struct LazyRedisStore {
    redis_url: String,
    inner: OnceCell<OpenKeyvRedisInner>,
}

impl LazyRedisStore {
    pub(super) fn new(redis_url: impl Into<String>) -> Self {
        Self {
            redis_url: redis_url.into(),
            inner: OnceCell::new(),
        }
    }

    async fn inner(&self) -> openkeyv::Result<&OpenKeyvRedisInner> {
        self.inner
            .get_or_try_init(|| async { OpenKeyvRedisInner::new(&self.redis_url).await })
            .await
    }
}

#[async_trait::async_trait]
impl AsyncKeyValue for LazyRedisStore {
    async fn get(
        &self,
        key: &str,
        collection: Option<&str>,
    ) -> openkeyv::Result<Option<HashMap<String, Value>>> {
        self.inner().await?.get(key, collection).await
    }

    async fn ttl(
        &self,
        key: &str,
        collection: Option<&str>,
    ) -> openkeyv::Result<Option<(HashMap<String, Value>, f64)>> {
        self.inner().await?.ttl(key, collection).await
    }

    async fn put(
        &self,
        key: &str,
        value: HashMap<String, Value>,
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> openkeyv::Result<()> {
        self.inner().await?.put(key, value, collection, ttl).await
    }

    async fn delete(&self, key: &str, collection: Option<&str>) -> openkeyv::Result<bool> {
        self.inner().await?.delete(key, collection).await
    }

    async fn get_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> openkeyv::Result<Vec<Option<HashMap<String, Value>>>> {
        self.inner().await?.get_many(keys, collection).await
    }

    async fn ttl_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> openkeyv::Result<Vec<Option<(HashMap<String, Value>, f64)>>> {
        self.inner().await?.ttl_many(keys, collection).await
    }

    async fn put_many(
        &self,
        keys: &[String],
        values: &[HashMap<String, Value>],
        collection: Option<&str>,
        ttl: Option<f64>,
    ) -> openkeyv::Result<()> {
        self.inner()
            .await?
            .put_many(keys, values, collection, ttl)
            .await
    }

    async fn delete_many(
        &self,
        keys: &[String],
        collection: Option<&str>,
    ) -> openkeyv::Result<usize> {
        self.inner().await?.delete_many(keys, collection).await
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateKeys for LazyRedisStore {
    async fn keys(
        &self,
        collection: Option<&str>,
        limit: Option<usize>,
    ) -> openkeyv::Result<Vec<String>> {
        self.inner().await?.keys(collection, limit).await
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateCollections for LazyRedisStore {
    async fn collections(&self, limit: Option<usize>) -> openkeyv::Result<Vec<String>> {
        self.inner().await?.collections(limit).await
    }
}
