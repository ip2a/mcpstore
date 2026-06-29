use std::collections::HashMap;

use openkeyv::{
    store::redis::RedisStore as OpenKeyvRedisInner, AsyncEnumerateCollections, AsyncEnumerateKeys,
    AsyncKeyValue,
};
use redis::AsyncCommands;
use serde_json::Value;
use tokio::sync::OnceCell;

use crate::cache::storage::CacheStore;
use crate::cache::{codec, CacheError, Result};

const COLLECTION_SEPARATOR: &str = ":";

const COMPARE_AND_PUT_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[1])
local expected = ARGV[1]
if current == false then
    if expected ~= '__none__' then
        return 0
    end
else
    local decoded = cjson.decode(current)
    local current_version = decoded['value']['version']
    if current_version == cjson.null then
        current_version = nil
    end
    if expected == '__none__' then
        return 0
    elseif tostring(current_version) ~= expected then
        return 0
    end
end
redis.call('SET', KEYS[1], ARGV[2])
return 1
"#;

pub(super) struct LazyRedisStore {
    redis_url: String,
    inner: OnceCell<OpenKeyvRedisInner>,
    client: OnceCell<redis::Client>,
}

pub(in crate::cache) struct RedisCacheStore {
    inner: LazyRedisStore,
}

impl RedisCacheStore {
    pub(in crate::cache) fn new(redis_url: impl Into<String>) -> Self {
        Self {
            inner: LazyRedisStore::new(redis_url),
        }
    }
}

impl LazyRedisStore {
    pub(super) fn new(redis_url: impl Into<String>) -> Self {
        Self {
            redis_url: redis_url.into(),
            inner: OnceCell::new(),
            client: OnceCell::new(),
        }
    }

    async fn inner(&self) -> openkeyv::Result<&OpenKeyvRedisInner> {
        self.inner
            .get_or_try_init(|| async { OpenKeyvRedisInner::new(&self.redis_url).await })
            .await
    }

    async fn client(&self) -> Result<&redis::Client> {
        self.client
            .get_or_try_init(|| async {
                redis::Client::open(self.redis_url.as_str())
                    .map_err(|err| CacheError::StoreError(format!("redis operation failed: {err}")))
            })
            .await
    }

    fn compound_key(collection: &str, key: &str) -> String {
        format!("{collection}{COLLECTION_SEPARATOR}{key}")
    }

    fn managed_entry_json(value: Value) -> Result<String> {
        serde_json::to_string(&serde_json::json!({
            "value": codec::value_to_object(value)?,
            "created_at": chrono::Utc::now(),
        }))
        .map_err(Into::into)
    }

    pub(super) async fn compare_and_put(
        &self,
        key: &str,
        expected_version: Option<u64>,
        value: Value,
        collection: &str,
    ) -> Result<()> {
        let client = self.client().await?;
        let mut conn = client
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|err| CacheError::StoreError(format!("redis operation failed: {err}")))?;
        let redis_key = Self::compound_key(collection, key);
        let expected = expected_version
            .map(|version| version.to_string())
            .unwrap_or_else(|| "__none__".to_string());
        let payload = Self::managed_entry_json(value)?;
        let updated: i64 = redis::Script::new(COMPARE_AND_PUT_SCRIPT)
            .key(&redis_key)
            .arg(&expected)
            .arg(&payload)
            .invoke_async(&mut conn)
            .await
            .map_err(|err| CacheError::StoreError(format!("redis operation failed: {err}")))?;
        if updated == 1 {
            Ok(())
        } else {
            let current: Option<String> = conn
                .get(&redis_key)
                .await
                .map_err(|err| CacheError::StoreError(format!("redis operation failed: {err}")))?;
            let current_version = current
                .and_then(|json| serde_json::from_str::<Value>(&json).ok())
                .and_then(|entry| entry.get("value")?.get("version")?.as_u64());
            Err(CacheError::Conflict(format!(
                "collection={collection}, key={key}, expected_version={expected_version:?}, current_version={current_version:?}"
            )))
        }
    }
}

fn map_openkeyv_err(err: openkeyv::Error) -> CacheError {
    CacheError::StoreError(format!("openkeyv operation failed: {err}"))
}

#[async_trait::async_trait]
impl CacheStore for RedisCacheStore {
    async fn put(&self, key: &str, value: Value, collection: &str) -> Result<()> {
        self.inner
            .put(key, codec::value_to_object(value)?, Some(collection), None)
            .await
            .map_err(map_openkeyv_err)
    }

    async fn compare_and_put(
        &self,
        key: &str,
        expected_version: Option<u64>,
        value: Value,
        collection: &str,
    ) -> Result<()> {
        self.inner
            .compare_and_put(key, expected_version, value, collection)
            .await
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
