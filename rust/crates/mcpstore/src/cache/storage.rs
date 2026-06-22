use std::sync::Arc;

use openkeyv::{
    store::memory::MemoryStore as OpenKeyvMemoryStore, AsyncEnumerateCollections,
    AsyncEnumerateKeys, AsyncKeyValue,
};
use serde_json::Value;

use crate::cache::{codec, redis::LazyRedisStore, CacheError, Result};

#[async_trait::async_trait]
pub(crate) trait CacheStore: Send + Sync {
    async fn put(&self, key: &str, value: Value, collection: &str) -> Result<()>;
    async fn get(&self, key: &str, collection: &str) -> Result<Option<Value>>;
    async fn delete(&self, key: &str, collection: &str) -> Result<()>;
    async fn collections(&self) -> Result<Vec<String>>;
    async fn keys(&self, collection: &str) -> Result<Vec<String>>;
    async fn get_many(&self, keys: &[String], collection: &str) -> Result<Vec<Option<Value>>>;
}

trait OpenKeyvBackend:
    AsyncKeyValue + AsyncEnumerateKeys + AsyncEnumerateCollections + Send + Sync
{
}

impl<T> OpenKeyvBackend for T where
    T: AsyncKeyValue + AsyncEnumerateKeys + AsyncEnumerateCollections + Send + Sync
{
}

struct OpenKeyvCacheStore<T>
where
    T: OpenKeyvBackend,
{
    inner: T,
}

impl<T> OpenKeyvCacheStore<T>
where
    T: OpenKeyvBackend,
{
    fn new(inner: T) -> Self {
        Self { inner }
    }
}

pub(crate) fn memory_cache_store() -> Arc<dyn CacheStore> {
    Arc::new(OpenKeyvCacheStore::new(OpenKeyvMemoryStore::new()))
}

pub(crate) fn redis_cache_store(redis_url: &str) -> Arc<dyn CacheStore> {
    Arc::new(OpenKeyvCacheStore::new(LazyRedisStore::new(redis_url)))
}

fn map_openkeyv_err(err: openkeyv::Error) -> CacheError {
    CacheError::StoreError(format!("openkeyv operation failed: {err}"))
}

#[async_trait::async_trait]
impl<T> CacheStore for OpenKeyvCacheStore<T>
where
    T: OpenKeyvBackend,
{
    async fn put(&self, key: &str, value: Value, collection: &str) -> Result<()> {
        self.inner
            .put(key, codec::value_to_object(value)?, Some(collection), None)
            .await
            .map_err(map_openkeyv_err)
    }

    async fn get(&self, key: &str, collection: &str) -> Result<Option<Value>> {
        self.inner
            .get(key, Some(collection))
            .await
            .map(|value| value.map(codec::object_to_value))
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
                    .map(|value| value.map(codec::object_to_value))
                    .collect()
            })
            .map_err(map_openkeyv_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn openkeyv_memory_adapter_round_trips_objects() {
        let store = OpenKeyvCacheStore::new(OpenKeyvMemoryStore::new());
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
        let store = OpenKeyvCacheStore::new(OpenKeyvMemoryStore::new());
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
        let store = OpenKeyvCacheStore::new(OpenKeyvMemoryStore::new());
        store
            .put("svc", serde_json::json!({"name": "svc"}), "services")
            .await
            .unwrap();

        store.delete("svc", "services").await.unwrap();

        assert_eq!(store.get("svc", "services").await.unwrap(), None);
    }

    #[tokio::test]
    async fn openkeyv_memory_adapter_rejects_non_object_values() {
        let store = OpenKeyvCacheStore::new(OpenKeyvMemoryStore::new());
        let err = store
            .put("bad", serde_json::json!(["not", "object"]), "services")
            .await;

        assert!(matches!(err, Err(CacheError::NotAnObject(_))));
    }
}
