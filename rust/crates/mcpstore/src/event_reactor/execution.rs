use openkeyv::{AsyncCompareAndSwap, AsyncEnumerateKeys, AsyncKeyValue};
use serde::{Deserialize, Serialize};

use crate::cache::codec;

const EXECUTION_COLLECTION_SUFFIX: &str = "reactor:executions";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ReactionExecutionStatus {
    Pending,
    Running { owner: String, started_at: i64 },
    Succeeded { finished_at: i64 },
    RetryWaiting { retry_at: i64, reason: String },
    Failed { finished_at: i64, reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ReactionExecutionRecord {
    pub change_id: String,
    pub rule_id: String,
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(flatten)]
    pub status: ReactionExecutionStatus,
}

pub(crate) struct ReactionExecutionStore<S> {
    store: S,
    collection: String,
}

impl<S> ReactionExecutionStore<S>
where
    S: AsyncKeyValue + AsyncCompareAndSwap + AsyncEnumerateKeys + Clone + Send + Sync,
{
    pub(crate) fn new(store: S, namespace: &str) -> Self {
        Self {
            store,
            collection: format!("{namespace}:{EXECUTION_COLLECTION_SUFFIX}"),
        }
    }

    pub(crate) async fn ensure_pending(
        &self,
        change_id: &str,
        rule_id: &str,
        collection: &str,
        event_key: &str,
    ) -> Result<ReactionExecutionStatus, ReactionExecutionError> {
        let key = execution_key(change_id, rule_id);
        if let Some(status) = self.get(change_id, rule_id).await? {
            return Ok(status);
        }
        let record = ReactionExecutionRecord {
            change_id: change_id.to_string(),
            rule_id: rule_id.to_string(),
            collection: Some(collection.to_string()),
            key: Some(event_key.to_string()),
            status: ReactionExecutionStatus::Pending,
        };
        let value = codec::json_to_value(serde_json::to_value(record)?)?;
        match self
            .store
            .compare_and_swap(&key, None, value, Some(&self.collection), None)
            .await?
        {
            openkeyv::CompareAndSwapResult::Applied { .. } => Ok(ReactionExecutionStatus::Pending),
            openkeyv::CompareAndSwapResult::Conflict { current } => {
                let current = current.ok_or(ReactionExecutionError::MissingConflictValue)?;
                Ok(decode_record(current.value)?.status)
            }
        }
    }

    pub(crate) async fn get(
        &self,
        change_id: &str,
        rule_id: &str,
    ) -> Result<Option<ReactionExecutionStatus>, ReactionExecutionError> {
        let value = self
            .store
            .get(&execution_key(change_id, rule_id), Some(&self.collection))
            .await?;
        value
            .map(|value| Ok(decode_record(value)?.status))
            .transpose()
    }

    pub(crate) async fn set(
        &self,
        change_id: &str,
        rule_id: &str,
        collection: &str,
        event_key: &str,
        status: ReactionExecutionStatus,
    ) -> Result<(), ReactionExecutionError> {
        let record = ReactionExecutionRecord {
            change_id: change_id.to_string(),
            rule_id: rule_id.to_string(),
            collection: Some(collection.to_string()),
            key: Some(event_key.to_string()),
            status,
        };
        self.store
            .put(
                &execution_key(change_id, rule_id),
                codec::json_to_value(serde_json::to_value(record)?)?,
                Some(&self.collection),
                None,
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn list(
        &self,
    ) -> Result<Vec<ReactionExecutionRecord>, ReactionExecutionError> {
        let keys = self.store.keys(Some(&self.collection), None).await?;
        let mut records = Vec::with_capacity(keys.len());
        for key in keys {
            let Some(value) = self.store.get(&key, Some(&self.collection)).await? else {
                continue;
            };
            records.push(decode_record(value)?);
        }
        Ok(records)
    }
}

fn execution_key(change_id: &str, rule_id: &str) -> String {
    format!("{change_id}:{rule_id}")
}

fn decode_record(
    value: openkeyv::Value,
) -> Result<ReactionExecutionRecord, ReactionExecutionError> {
    Ok(serde_json::from_value(codec::value_to_json(value)?)?)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ReactionExecutionError {
    #[error(transparent)]
    Store(#[from] openkeyv::Error),
    #[error(transparent)]
    Codec(#[from] crate::cache::CacheError),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error("compare-and-swap conflict did not include the current execution record")]
    MissingConflictValue,
}

#[cfg(test)]
mod tests {
    use openkeyv::store::memory::MemoryStore;

    use super::{ReactionExecutionStatus, ReactionExecutionStore};

    #[tokio::test]
    async fn execution_statuses_round_trip() {
        let store = ReactionExecutionStore::new(MemoryStore::new(), "test");
        assert_eq!(
            store
                .ensure_pending("change", "rule", "collection", "event")
                .await
                .unwrap(),
            ReactionExecutionStatus::Pending
        );

        let statuses = [
            ReactionExecutionStatus::Running {
                owner: "worker-1".to_string(),
                started_at: 10,
            },
            ReactionExecutionStatus::Succeeded { finished_at: 20 },
            ReactionExecutionStatus::RetryWaiting {
                retry_at: 30,
                reason: "temporary failure".to_string(),
            },
            ReactionExecutionStatus::Failed {
                finished_at: 40,
                reason: "permanent failure".to_string(),
            },
        ];

        for status in statuses {
            store
                .set("change", "rule", "collection", "event", status.clone())
                .await
                .unwrap();
            assert_eq!(store.get("change", "rule").await.unwrap(), Some(status));
        }
    }
}
