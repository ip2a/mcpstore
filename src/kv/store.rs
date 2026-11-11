use std::sync::Arc;

use serde_json::Value as JsonValue;

use crate::kv::codec;
use crate::kv::layer::{KvError, Result};
use openkeyv::{
    AsyncCompareAndSwap, AsyncEnumerateCollections, AsyncEnumerateKeys, CompareAndDeleteResult,
    CompareAndSwapResult, StoreHandle,
};

/// mcpstore's unified KV store. Internally holds an OpenKeyv StoreHandle and
/// exposes the JSON-level operations that the KV layer needs.
pub struct KvStore {
    handle: StoreHandle,
}

impl KvStore {
    pub(crate) fn from_handle(handle: StoreHandle) -> Self {
        Self { handle }
    }

    pub(crate) fn handle(&self) -> StoreHandle {
        self.handle.clone()
    }

    fn cas(&self) -> Result<&Arc<dyn AsyncCompareAndSwap>> {
        self.handle
            .compare_and_swap
            .as_ref()
            .ok_or_else(|| KvError::Store("store does not provide compare-and-swap".into()))
    }

    fn enumerate_keys(&self) -> Result<&Arc<dyn AsyncEnumerateKeys>> {
        self.handle
            .enumerate_keys
            .as_ref()
            .ok_or_else(|| KvError::Store("store does not provide key enumeration".into()))
    }

    fn enumerate_collections(&self) -> Result<&Arc<dyn AsyncEnumerateCollections>> {
        self.handle.enumerate_collections.as_ref().ok_or_else(|| {
            KvError::Store("store does not provide collection enumeration".into())
        })
    }
}

fn map_openkeyv_err(err: openkeyv::Error) -> KvError {
    KvError::Store(format!("openkeyv: {err}"))
}

fn value_version(value: &JsonValue) -> Option<u64> {
    value.get("version").and_then(JsonValue::as_u64)
}

// KvStore implements the KV layer's JSON operations directly.
impl KvStore {
    pub(crate) async fn put(&self, key: &str, value: JsonValue, collection: &str) -> Result<()> {
        if !value.is_object() {
            return Err(KvError::NotAnObject(format!(
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

    pub(crate) async fn compare_and_put(
        &self,
        key: &str,
        expected_version: Option<u64>,
        value: JsonValue,
        collection: &str,
    ) -> Result<()> {
        if !value.is_object() {
            return Err(KvError::NotAnObject(format!(
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
                            return Err(KvError::Conflict(format!(
                                "version mismatch: expected {expected_version:?}, found {stored_ver:?}"
                            )));
                        }
                        Some(rv.revision)
                    }
                    None => {
                        if expected_version.is_some() {
                            return Err(KvError::Conflict(
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
            CompareAndSwapResult::Conflict { .. } => Err(KvError::Conflict(
                "concurrent modification detected".into(),
            )),
        }
    }

    pub(crate) async fn get(&self, key: &str, collection: &str) -> Result<Option<JsonValue>> {
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

    pub(crate) async fn delete(&self, key: &str, collection: &str) -> Result<()> {
        self.handle
            .base
            .delete(key, Some(collection))
            .await
            .map_err(map_openkeyv_err)?;
        Ok(())
    }

    pub(crate) async fn claim(&self, key: &str, collection: &str) -> Result<Option<JsonValue>> {
        let cas = self.cas()?;
        let Some(revisioned) = cas
            .get_with_revision(key, Some(collection))
            .await
            .map_err(map_openkeyv_err)?
        else {
            return Ok(None);
        };
        match cas
            .compare_and_delete(key, &revisioned.revision, Some(collection))
            .await
            .map_err(map_openkeyv_err)?
        {
            CompareAndDeleteResult::Deleted => Ok(Some(codec::value_to_json(revisioned.value)?)),
            CompareAndDeleteResult::Conflict { .. } => Ok(None),
        }
    }

    pub(crate) async fn collections(&self) -> Result<Vec<String>> {
        self.enumerate_collections()?
            .collections(None)
            .await
            .map_err(map_openkeyv_err)
    }

    pub(crate) async fn keys(&self, collection: &str) -> Result<Vec<String>> {
        self.enumerate_keys()?
            .keys(Some(collection), None)
            .await
            .map_err(map_openkeyv_err)
    }

    pub(crate) async fn get_many(&self, keys: &[String], collection: &str) -> Result<Vec<Option<JsonValue>>> {
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
