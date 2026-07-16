use openkeyv::{
    store::redis::RedisStore as OpenKeyvRedisInner, AsyncCompareAndSwap, AsyncEnumerateCollections,
    AsyncEnumerateKeys, AsyncKeyValue, Revision,
};
use serde_json::Value as JsonValue;
use tokio::sync::OnceCell;

use crate::cache::storage::CacheStore;
use crate::cache::{codec, CacheError, Result};

pub(in crate::cache) struct RedisCacheStore {
    inner: OnceCell<OpenKeyvRedisInner>,
    redis_url: String,
}

impl RedisCacheStore {
    pub(in crate::cache) fn new(redis_url: impl Into<String>) -> Self {
        Self {
            inner: OnceCell::new(),
            redis_url: redis_url.into(),
        }
    }

    async fn store(&self) -> Result<&OpenKeyvRedisInner> {
        self.inner
            .get_or_try_init(|| async {
                OpenKeyvRedisInner::new(&self.redis_url).await
            })
            .await
            .map_err(map_openkeyv_err)
    }
}

fn value_version(value: &JsonValue) -> Option<u64> {
    value.get("version").and_then(JsonValue::as_u64)
}

fn map_openkeyv_err(err: openkeyv::Error) -> CacheError {
    CacheError::StoreError(format!("openkeyv operation failed: {err}"))
}

#[async_trait::async_trait]
impl CacheStore for RedisCacheStore {
    async fn put(&self, key: &str, value: JsonValue, collection: &str) -> Result<()> {
        let okv_value = codec::json_to_value(value)?;
        self.store()
            .await?
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
        let store = self.store().await?;

        let revisioned = store
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
        let outcome = store
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
        self.store()
            .await?
            .get(key, Some(collection))
            .await
            .map_err(map_openkeyv_err)?
            .map(|v| codec::value_to_json(v))
            .transpose()
    }

    async fn delete(&self, key: &str, collection: &str) -> Result<()> {
        self.store()
            .await?
            .delete(key, Some(collection))
            .await
            .map_err(map_openkeyv_err)?;
        Ok(())
    }

    async fn collections(&self) -> Result<Vec<String>> {
        self.store()
            .await?
            .collections(None)
            .await
            .map_err(map_openkeyv_err)
    }

    async fn keys(&self, collection: &str) -> Result<Vec<String>> {
        self.store()
            .await?
            .keys(Some(collection), None)
            .await
            .map_err(map_openkeyv_err)
    }

    async fn get_many(
        &self,
        keys: &[String],
        collection: &str,
    ) -> Result<Vec<Option<JsonValue>>> {
        self.store()
            .await?
            .get_many(keys, Some(collection))
            .await
            .map_err(map_openkeyv_err)?
            .into_iter()
            .map(|v| v.map(codec::value_to_json).transpose())
            .collect()
    }
}
