use std::sync::Arc;

use crate::cache::{memory_cache_store, redis_cache_store, CacheSnapshot, CacheStore};
use crate::store::prelude::*;

impl MCPStore {
    pub(crate) fn build_cache_store(
        cache_storage: &CacheStorage,
        redis_url: &str,
        _namespace: &str,
    ) -> Result<Arc<dyn CacheStore>> {
        match cache_storage {
            CacheStorage::Memory | CacheStorage::OpenKeyvMemory => Ok(memory_cache_store()),
            CacheStorage::Redis | CacheStorage::OpenKeyvRedis => Ok(redis_cache_store(redis_url)),
        }
    }

    pub async fn current_cache_storage(&self) -> CacheStorage {
        self.cache_storage.read().await.clone()
    }

    pub async fn current_backend(&self) -> BackendKind {
        self.current_cache_storage().await
    }

    pub async fn switch_cache_storage(
        &self,
        cache_storage: CacheStorage,
        redis_url: Option<String>,
        namespace: Option<String>,
    ) -> Result<CacheSnapshot> {
        let resolved_redis_url = match redis_url {
            Some(url) => url,
            None => self
                .redis_url
                .read()
                .await
                .clone()
                .unwrap_or_else(|| "redis://127.0.0.1/".to_string()),
        };
        let resolved_namespace = namespace.unwrap_or_else(|| self.namespace());
        let cache_store =
            Self::build_cache_store(&cache_storage, &resolved_redis_url, &resolved_namespace)?;
        let snapshot = self
            .cache
            .replace_store_with_snapshot_and_namespace(cache_store, resolved_namespace.clone())
            .await?;

        *self.cache_storage.write().await = cache_storage;
        *self.redis_url.write().await = Some(resolved_redis_url);
        *self
            .namespace
            .write()
            .expect("store namespace lock poisoned") = resolved_namespace;
        Ok(snapshot)
    }

    pub async fn switch_backend(
        &self,
        cache_storage: BackendKind,
        redis_url: Option<String>,
        namespace: Option<String>,
    ) -> Result<CacheSnapshot> {
        self.switch_cache_storage(cache_storage, redis_url, namespace)
            .await
    }
}
