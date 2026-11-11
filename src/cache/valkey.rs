use std::sync::Arc;

use openkeyv::{
    store::valkey::{ValkeyConfig, ValkeyStore as OpenKeyvValkeyInner},
    AsyncChangeFeed, AsyncCompareAndSwap, AsyncEnumerateCollections, AsyncEnumerateKeys,
    AsyncKeyValue, Revision, RevisionedValue, Subspace, Value,
};
use tokio::sync::OnceCell;

/// Lazily-connected Valkey store. Independent of the Redis path by design:
/// separate type, separate cargo feature, and a change feed wired into the
/// store handle. Built on first use so Store creation stays synchronous.
pub(in crate::cache) struct LazyValkeyStore {
    inner: OnceCell<Arc<OpenKeyvValkeyInner>>,
    url: String,
    keyspace: String,
}

impl LazyValkeyStore {
    pub(in crate::cache) fn new(url: impl Into<String>, keyspace: impl Into<String>) -> Self {
        Self {
            inner: OnceCell::new(),
            url: url.into(),
            keyspace: keyspace.into(),
        }
    }

    async fn handle(&self) -> openkeyv::Result<&Arc<OpenKeyvValkeyInner>> {
        self.inner
            .get_or_try_init(|| async {
                let keyspace = self.keyspace.clone();
                let config = ValkeyConfig::default().with_keyspace(keyspace.clone());
                let store = OpenKeyvValkeyInner::new_with_config(&self.url, config).await?;

                if !keyspace.is_empty() {
                    const MARKER_COLLECTION: &str = "__mcpstore_keyspace_meta";
                    const MARKER_KEY: &str = "keyspace_migration_v1";
                    if store
                        .get(MARKER_KEY, Some(MARKER_COLLECTION))
                        .await?
                        .is_none()
                    {
                        let options = openkeyv::MigrationOptions {
                            collection_prefix: Some((
                                format!("{keyspace}:"),
                                format!("{keyspace}:"),
                            )),
                            ..openkeyv::MigrationOptions::default()
                        };
                        openkeyv::migrate_into_keyspace(&store, &Subspace::new(keyspace), &options)
                            .await?;
                        store
                            .put(
                                MARKER_KEY,
                                Value::from_structured(&openkeyv::StructuredValue::from_json(
                                    &serde_json::json!({ "version": 1 }),
                                )?)?,
                                Some(MARKER_COLLECTION),
                                None,
                            )
                            .await?;
                    }
                }

                Ok(Arc::new(store))
            })
            .await
    }
}

#[async_trait::async_trait]
impl AsyncKeyValue for LazyValkeyStore {
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
impl AsyncCompareAndSwap for LazyValkeyStore {
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
impl AsyncEnumerateKeys for LazyValkeyStore {
    async fn keys(
        &self,
        collection: Option<&str>,
        limit: Option<usize>,
    ) -> openkeyv::Result<Vec<String>> {
        self.handle().await?.keys(collection, limit).await
    }
}

#[async_trait::async_trait]
impl AsyncEnumerateCollections for LazyValkeyStore {
    async fn collections(&self, limit: Option<usize>) -> openkeyv::Result<Vec<String>> {
        self.handle().await?.collections(limit).await
    }
}

#[async_trait::async_trait]
impl AsyncChangeFeed for LazyValkeyStore {
    async fn subscribe(
        &self,
        request: openkeyv::ChangeFeedRequest,
    ) -> openkeyv::Result<openkeyv::ChangeSubscription> {
        self.handle().await?.subscribe(request).await
    }
}
