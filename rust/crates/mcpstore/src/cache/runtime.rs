use std::{sync::Arc, time::Duration};

use crate::cache::{memory_cache_store, redis_cache_store, CacheSnapshot, CacheStore};
use crate::store::prelude::*;
use openkeyv::AsyncKeyValue;

fn is_redis(storage: &CacheStorage) -> bool {
    matches!(storage, CacheStorage::Redis | CacheStorage::OpenKeyvRedis)
}

fn migration_error(error: openkeyv::Error) -> StoreError {
    StoreError::Other(format!("cache migration failed: {error}"))
}

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
        let current_storage = self.cache_storage.read().await.clone();
        let current_redis_url = self.redis_url.read().await.clone().unwrap_or_default();
        if is_redis(&current_storage)
            && is_redis(&cache_storage)
            && current_redis_url == resolved_redis_url
            && self.namespace() == resolved_namespace
        {
            return self.cache.snapshot().await.map_err(StoreError::from);
        }

        let source_backend = match self.event_backend.read().await.clone() {
            Some(backend) => backend,
            None if is_redis(&current_storage) => {
                crate::event_reactor::EventBackend::from_redis_url(&current_redis_url)
                    .await
                    .map_err(migration_error)?
            }
            None => {
                return Err(StoreError::Other(
                    "cache backend does not expose an OpenKeyV migration handle".to_string(),
                ));
            }
        };
        let source_namespace = self.namespace();
        let snapshot = self.cache.snapshot().await?;

        // The cache adapter and event backend share the same underlying target.
        let (cache_store, event_backend) = match cache_storage {
            CacheStorage::Memory | CacheStorage::OpenKeyvMemory => {
                let (store, mem) = crate::cache::storage::memory_cache_store_with_handle();
                (store, crate::event_reactor::EventBackend::from_memory(mem))
            }
            CacheStorage::Redis | CacheStorage::OpenKeyvRedis => {
                let store = Self::build_cache_store(
                    &cache_storage,
                    &resolved_redis_url,
                    &resolved_namespace,
                )?;
                let backend =
                    crate::event_reactor::EventBackend::from_redis_url(&resolved_redis_url)
                        .await
                        .map_err(|e| StoreError::Other(format!("event backend init: {e}")))?;
                (store, backend)
            }
        };

        crate::cache::CacheLayerManager::clear_namespace(cache_store.as_ref(), &resolved_namespace)
            .await?;
        let options = openkeyv::MigrationOptions {
            collection_prefix: Some((
                format!("{source_namespace}:"),
                format!("{resolved_namespace}:"),
            )),
            ..openkeyv::MigrationOptions::default()
        };
        let (mut report, mut changes) =
            openkeyv::copy_snapshot_with_feed(&source_backend, &event_backend, &options)
                .await
                .map_err(migration_error)?;

        let had_reactor = self.has_reactor().await;
        if had_reactor {
            self.stop_reactor().await;
        }

        // ponytail: one global cutover lock; split by collection only if this pause is measurable.
        let _route = self.cache.route.write().await;
        let mut current = self.cache.store.write().await;
        let barrier_collection = "__mcpstore_migration";
        let barrier_key = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        if let Err(error) = source_backend
            .put(
                &barrier_key,
                openkeyv::Value::utf8("cutover"),
                Some(barrier_collection),
                None,
            )
            .await
        {
            drop(current);
            if had_reactor {
                let _ = self.start_reactor().await;
            }
            return Err(migration_error(error));
        }
        loop {
            let change = match tokio::time::timeout(Duration::from_secs(5), changes.recv()).await {
                Ok(Ok(Some(change))) => change,
                Ok(Ok(None)) => {
                    drop(current);
                    if had_reactor {
                        let _ = self.start_reactor().await;
                    }
                    return Err(StoreError::Other(
                        "cache migration change feed ended before cutover barrier".to_string(),
                    ));
                }
                Err(_) => {
                    drop(current);
                    if had_reactor {
                        let _ = self.start_reactor().await;
                    }
                    return Err(StoreError::Other(
                        "cache migration timed out waiting for cutover barrier".to_string(),
                    ));
                }
                Ok(Err(error)) => {
                    drop(current);
                    if had_reactor {
                        let _ = self.start_reactor().await;
                    }
                    return Err(migration_error(error));
                }
            };
            if change.collection == barrier_collection && change.key == barrier_key {
                break;
            }
            let replay =
                match openkeyv::apply_change(&source_backend, &event_backend, &change, &options)
                    .await
                {
                    Ok(replay) => replay,
                    Err(error) => {
                        drop(current);
                        if had_reactor {
                            let _ = self.start_reactor().await;
                        }
                        return Err(migration_error(error));
                    }
                };
            openkeyv::merge_report(&mut report, replay);
        }
        if let Err(error) = source_backend
            .delete(&barrier_key, Some(barrier_collection))
            .await
        {
            drop(current);
            if had_reactor {
                let _ = self.start_reactor().await;
            }
            return Err(migration_error(error));
        }
        *current = cache_store;
        *self
            .cache
            .namespace
            .write()
            .expect("cache namespace lock poisoned") = resolved_namespace.clone();
        self.cache.last_state_snapshot.write().await.clear();
        drop(current);

        *self.cache_storage.write().await = cache_storage;
        *self.redis_url.write().await = Some(resolved_redis_url);
        *self
            .namespace
            .write()
            .expect("store namespace lock poisoned") = resolved_namespace;
        *self.event_backend.write().await = Some(event_backend);
        *self.event_reactor.write().await = None;
        tracing::info!(
            copied = report.copied,
            replayed = report.replayed,
            skipped_expired = report.skipped_expired,
            "cache migration completed"
        );
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
