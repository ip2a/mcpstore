use std::sync::Arc;

use serde_json::Value as JsonValue;

use crate::cache::codec;
use crate::cache::layer::{CacheError, Result};
use openkeyv::{
    AsyncCompareAndSwap, AsyncEnumerateCollections, AsyncEnumerateKeys, CompareAndSwapResult,
    StoreHandle,
};

/// mcpstore's unified cache Store. Internally holds an OpenKeyv StoreHandle and
/// exposes the JSON-level operations that the cache layer needs.
pub struct LiveStore {
    handle: StoreHandle,
}

impl LiveStore {
    pub(crate) fn from_handle(handle: StoreHandle) -> Self {
        Self { handle }
    }

    fn cas(&self) -> Result<&Arc<dyn AsyncCompareAndSwap>> {
        self.handle
            .compare_and_swap
            .as_ref()
            .ok_or_else(|| CacheError::StoreError("store does not provide compare-and-swap".into()))
    }

    fn enumerate_keys(&self) -> Result<&Arc<dyn AsyncEnumerateKeys>> {
        self.handle
            .enumerate_keys
            .as_ref()
            .ok_or_else(|| CacheError::StoreError("store does not provide key enumeration".into()))
    }

    fn enumerate_collections(&self) -> Result<&Arc<dyn AsyncEnumerateCollections>> {
        self.handle.enumerate_collections.as_ref().ok_or_else(|| {
            CacheError::StoreError("store does not provide collection enumeration".into())
        })
    }
}

fn map_openkeyv_err(err: openkeyv::Error) -> CacheError {
    CacheError::StoreError(format!("openkeyv: {err}"))
}

fn value_version(value: &JsonValue) -> Option<u64> {
    value.get("version").and_then(JsonValue::as_u64)
}

// LiveStore implements the cache layer's JSON operations directly.
#[async_trait::async_trait]
impl super::storage::CacheStore for LiveStore {
    async fn put(&self, key: &str, value: JsonValue, collection: &str) -> Result<()> {
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "collection={collection}, key={key}"
            )));
        }
        let okv_value = codec::json_to_value(value)?;
        self.handle
            .base
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
        if !value.is_object() {
            return Err(CacheError::NotAnObject(format!(
                "collection={collection}, key={key}"
            )));
        }
        let cas = self.cas()?;
        let expected_revision = match expected_version {
            None => None,
            Some(_) => {
                let current = cas
                    .get_with_revision(key, Some(collection))
                    .await
                    .map_err(map_openkeyv_err)?;
                match current {
                    Some(rv) => {
                        let stored_json = codec::value_to_json(rv.value)?;
                        let stored_ver = value_version(&stored_json);
                        if stored_ver != expected_version {
                            return Err(CacheError::Conflict(format!(
                                "version mismatch: expected {expected_version:?}, found {stored_ver:?}"
                            )));
                        }
                        Some(rv.revision)
                    }
                    None => {
                        if expected_version.is_some() {
                            return Err(CacheError::Conflict(
                                "key does not exist for CAS update".into(),
                            ));
                        }
                        None
                    }
                }
            }
        };

        let okv_value = codec::json_to_value(value)?;
        let result = cas
            .compare_and_swap(
                key,
                expected_revision.as_ref(),
                okv_value,
                Some(collection),
                None,
            )
            .await
            .map_err(map_openkeyv_err)?;

        match result {
            CompareAndSwapResult::Applied { .. } => Ok(()),
            CompareAndSwapResult::Conflict { .. } => Err(CacheError::Conflict(
                "concurrent modification detected".into(),
            )),
        }
    }

    async fn get(&self, key: &str, collection: &str) -> Result<Option<JsonValue>> {
        let value = self
            .handle
            .base
            .get(key, Some(collection))
            .await
            .map_err(map_openkeyv_err)?;
        match value {
            Some(v) => Ok(Some(codec::value_to_json(v)?)),
            None => Ok(None),
        }
    }

    async fn delete(&self, key: &str, collection: &str) -> Result<()> {
        self.handle
            .base
            .delete(key, Some(collection))
            .await
            .map_err(map_openkeyv_err)?;
        Ok(())
    }

    async fn collections(&self) -> Result<Vec<String>> {
        self.enumerate_collections()?
            .collections(None)
            .await
            .map_err(map_openkeyv_err)
    }

    async fn keys(&self, collection: &str) -> Result<Vec<String>> {
        self.enumerate_keys()?
            .keys(Some(collection), None)
            .await
            .map_err(map_openkeyv_err)
    }

    async fn get_many(&self, keys: &[String], collection: &str) -> Result<Vec<Option<JsonValue>>> {
        let values = self
            .handle
            .base
            .get_many(keys, Some(collection))
            .await
            .map_err(map_openkeyv_err)?;
        values
            .into_iter()
            .map(|opt| match opt {
                Some(v) => codec::value_to_json(v).map(Some),
                None => Ok(None),
            })
            .collect()
    }
}
