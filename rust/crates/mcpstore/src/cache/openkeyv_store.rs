use openkeyv::{
    AsyncCompareAndSwap, AsyncEnumerateCollections, AsyncEnumerateKeys, AsyncKeyValue, Revision,
};
use serde_json::Value as JsonValue;

use crate::cache::storage::CacheStore;
use crate::cache::{codec, CacheError, Result};

pub(in crate::cache) trait OpenKeyvStoreApi:
    AsyncKeyValue
    + AsyncEnumerateKeys
    + AsyncEnumerateCollections
    + AsyncCompareAndSwap
    + Send
    + Sync
{
}

impl<T> OpenKeyvStoreApi for T where
    T: AsyncKeyValue
        + AsyncEnumerateKeys
        + AsyncEnumerateCollections
        + AsyncCompareAndSwap
        + Send
        + Sync,
{
}

pub(in crate::cache) struct OpenKeyvCacheStore<T>
where
    T: OpenKeyvStoreApi,
{
    inner: T,
}

impl<T> OpenKeyvCacheStore<T>
where
    T: OpenKeyvStoreApi,
{
    pub(in crate::cache) fn new(inner: T) -> Self {
        Self { inner }
    }
}

fn value_version(value: &JsonValue) -> Option<u64> {
    value.get("version").and_then(JsonValue::as_u64)
}

fn map_openkeyv_err(err: openkeyv::Error) -> CacheError {
    CacheError::StoreError(format!("openkeyv operation failed: {err}"))
}

#[async_trait::async_trait]
impl<T> CacheStore for OpenKeyvCacheStore<T>
where
    T: OpenKeyvStoreApi,
{
    async fn put(&self, key: &str, value: JsonValue, collection: &str) -> Result<()> {
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "collection={collection}, key={key}"
            )));
        }
        let okv_value = codec::json_to_value(value)?;
        self.inner
            .put(key, okv_value, Some(collection), None)
            .await
            .map_err(map_openkeyv_err)
    }

    async fn compare_and_put(
        &self,
        key: &str,
        expected_version: Option<u64>,
        value: JsonValue,
        collection: &str,
    ) -> Result<()> {
        let revisioned = self
            .inner
            .get_with_revision(key, Some(collection))
            .await
            .map_err(map_openkeyv_err)?;

        let current_version = revisioned
            .as_ref()
            .and_then(|r| value_version(&codec::value_to_json(r.value.clone()).ok()?));

        let matches = match expected_version {
            Some(expected) => current_version == Some(expected),
            None => revisioned.is_none(),
        };

        if !matches {
            return Err(CacheError::Conflict(format!(
                "collection={collection}, key={key}, expected_version={expected_version:?}, current_version={current_version:?}"
            )));
        }

        let expected_revision: Option<&Revision> = revisioned.as_ref().map(|r| &r.revision);
        let okv_value = codec::json_to_value(value)?;
        let outcome = self
            .inner
            .compare_and_swap(key, expected_revision, okv_value, Some(collection), None)
            .await
            .map_err(map_openkeyv_err)?;

        match outcome {
            openkeyv::CompareAndSwapResult::Applied { .. } => Ok(()),
            openkeyv::CompareAndSwapResult::Conflict { .. } => Err(CacheError::Conflict(
                format!("concurrent modification: collection={collection}, key={key}"),
            )),
        }
    }

    async fn get(&self, key: &str, collection: &str) -> Result<Option<JsonValue>> {
        self.inner
            .get(key, Some(collection))
            .await
            .map_err(map_openkeyv_err)?
            .map(|v| codec::value_to_json(v))
            .transpose()
    }

    async fn delete(&self, key: &str, collection: &str) -> Result<()> {
        self.inner
            .delete(key, Some(collection))
            .await
            .map_err(map_openkeyv_err)?;
        Ok(())
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

    async fn get_many(
        &self,
        keys: &[String],
        collection: &str,
    ) -> Result<Vec<Option<JsonValue>>> {
        self.inner
            .get_many(keys, Some(collection))
            .await
            .map_err(map_openkeyv_err)?
            .into_iter()
            .map(|v| v.map(codec::value_to_json).transpose())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use openkeyv::store::memory::MemoryStore as OpenKeyvMemoryStore;

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
    async fn openkeyv_memory_adapter_compare_and_put_rejects_stale_version() {
        let store = OpenKeyvCacheStore::new(OpenKeyvMemoryStore::new());
        store
            .compare_and_put(
                "s1",
                None,
                serde_json::json!({"session_key": "s1", "version": 1}),
                "sessions",
            )
            .await
            .unwrap();

        let err = store
            .compare_and_put(
                "s1",
                Some(0),
                serde_json::json!({"session_key": "s1", "version": 2}),
                "sessions",
            )
            .await;

        assert!(matches!(err, Err(CacheError::Conflict(_))));
        assert_eq!(
            store.get("s1", "sessions").await.unwrap(),
            Some(serde_json::json!({"session_key": "s1", "version": 1}))
        );
    }

    #[tokio::test]
    async fn openkeyv_memory_adapter_compare_and_put_rejects_duplicate_create() {
        let store = OpenKeyvCacheStore::new(OpenKeyvMemoryStore::new());
        store
            .put("s1", serde_json::json!({"session_key": "s1"}), "sessions")
            .await
            .unwrap();

        let err = store
            .compare_and_put(
                "s1",
                None,
                serde_json::json!({"session_key": "s1", "version": 1}),
                "sessions",
            )
            .await;

        assert!(matches!(err, Err(CacheError::Conflict(_))));
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
