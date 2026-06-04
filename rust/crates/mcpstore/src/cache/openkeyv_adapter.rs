use std::collections::HashMap;
use std::sync::Arc;

use openkeyv::{
    store::{memory::MemoryStore as OpenKeyvMemoryStore, redis::RedisStore as OpenKeyvRedisInner},
    AsyncEnumerateCollections, AsyncEnumerateKeys, AsyncKeyValue,
};
use serde_json::{Map, Value};
use tokio::sync::OnceCell;

use crate::cache::{CacheError, KvStore, Result};

pub trait OpenKeyvBackend:
    AsyncKeyValue + AsyncEnumerateKeys + AsyncEnumerateCollections + Send + Sync
{
}

impl<T> OpenKeyvBackend for T where
    T: AsyncKeyValue + AsyncEnumerateKeys + AsyncEnumerateCollections + Send + Sync
{
}

pub struct OpenKeyvAdapter<T>
where
    T: OpenKeyvBackend,
{
    inner: T,
}

impl<T> OpenKeyvAdapter<T>
where
    T: OpenKeyvBackend,
{
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

pub struct OpenKeyvRedisStore {
    redis_url: String,
    inner: OnceCell<OpenKeyvRedisInner>,
}

impl OpenKeyvRedisStore {
    pub fn new(redis_url: impl Into<String>) -> Self {
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

pub fn openkeyv_memory_backend() -> Arc<dyn KvStore> {
    Arc::new(OpenKeyvAdapter::new(OpenKeyvMemoryStore::new()))
}

pub fn openkeyv_redis_backend(redis_url: &str) -> Arc<dyn KvStore> {
    Arc::new(OpenKeyvAdapter::new(OpenKeyvRedisStore::new(redis_url)))
}

fn map_openkeyv_err(err: openkeyv::Error) -> CacheError {
    CacheError::StoreError(format!("openkeyv operation failed: {err}"))
}

fn value_to_object(value: Value) -> Result<HashMap<String, Value>> {
    match value {
        Value::Object(object) => Ok(object.into_iter().collect()),
        other => Err(CacheError::NotAnObject(format!("value={other}"))),
    }
}

fn object_to_value(value: HashMap<String, Value>) -> Value {
    Value::Object(Map::from_iter(value))
}

#[async_trait::async_trait]
impl<T> KvStore for OpenKeyvAdapter<T>
where
    T: OpenKeyvBackend,
{
    async fn put(&self, key: &str, value: Value, collection: &str) -> Result<()> {
        self.inner
            .put(key, value_to_object(value)?, Some(collection), None)
            .await
            .map_err(map_openkeyv_err)
    }

    async fn get(&self, key: &str, collection: &str) -> Result<Option<Value>> {
        self.inner
            .get(key, Some(collection))
            .await
            .map(|value| value.map(object_to_value))
            .map_err(map_openkeyv_err)
    }

    async fn delete(&self, key: &str, collection: &str) -> Result<()> {
        self.inner
            .delete(key, Some(collection))
            .await
            .map(|_| ())
            .map_err(map_openkeyv_err)
    }

    async fn collections(&self) -> Result<Vec<String>> {
        self.inner.collections(None).await.map_err(map_openkeyv_err)
    }

    async fn keys(&self, collection: &str) -> Result<Vec<String>> {
        self.inner
            .keys(Some(collection), None)
            .await
            .map_err(map_openkeyv_err)
    }

    async fn get_many(&self, keys: &[String], collection: &str) -> Result<Vec<Option<Value>>> {
        self.inner
            .get_many(keys, Some(collection))
            .await
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value.map(object_to_value))
                    .collect()
            })
            .map_err(map_openkeyv_err)
    }
}

#[async_trait::async_trait]
impl AsyncKeyValue for OpenKeyvRedisStore {
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
impl AsyncEnumerateKeys for OpenKeyvRedisStore {
    async fn keys(
        &self,
        collection: Option<&str>,
        limit: Option<usize>,
    ) -> openkeyv::Result<Vec<String>> {
        self.inner().await?.keys(collection, limit).await
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateCollections for OpenKeyvRedisStore {
    async fn collections(&self, limit: Option<usize>) -> openkeyv::Result<Vec<String>> {
        self.inner().await?.collections(limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn openkeyv_memory_adapter_round_trips_objects() {
        let store = OpenKeyvAdapter::new(OpenKeyvMemoryStore::new());
        let value = serde_json::json!({"name": "svc", "enabled": true});

        store
            .put("svc", value.clone(), "mcpstore:entity:services")
            .await
            .unwrap();

        assert_eq!(
            store.get("svc", "mcpstore:entity:services").await.unwrap(),
            Some(value)
        );
        assert_eq!(
            store.keys("mcpstore:entity:services").await.unwrap(),
            vec!["svc".to_string()]
        );
        assert!(store
            .collections()
            .await
            .unwrap()
            .contains(&"mcpstore:entity:services".to_string()));
    }

    #[tokio::test]
    async fn openkeyv_memory_adapter_preserves_get_many_order() {
        let store = OpenKeyvAdapter::new(OpenKeyvMemoryStore::new());
        store
            .put("a", serde_json::json!({"name": "a"}), "services")
            .await
            .unwrap();
        store
            .put("c", serde_json::json!({"name": "c"}), "services")
            .await
            .unwrap();

        let keys = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let values = store.get_many(&keys, "services").await.unwrap();

        assert_eq!(values[0], Some(serde_json::json!({"name": "a"})));
        assert_eq!(values[1], None);
        assert_eq!(values[2], Some(serde_json::json!({"name": "c"})));
    }

    #[tokio::test]
    async fn openkeyv_memory_adapter_deletes_entries() {
        let store = OpenKeyvAdapter::new(OpenKeyvMemoryStore::new());
        store
            .put("svc", serde_json::json!({"name": "svc"}), "services")
            .await
            .unwrap();

        store.delete("svc", "services").await.unwrap();

        assert_eq!(store.get("svc", "services").await.unwrap(), None);
    }

    #[tokio::test]
    async fn openkeyv_memory_adapter_rejects_non_object_values() {
        let store = OpenKeyvAdapter::new(OpenKeyvMemoryStore::new());
        let err = store
            .put("bad", serde_json::json!(["not", "object"]), "services")
            .await;

        assert!(matches!(err, Err(CacheError::NotAnObject(_))));
    }
}
