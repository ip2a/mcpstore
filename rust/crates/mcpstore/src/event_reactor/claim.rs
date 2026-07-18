//! Distributed lease for one `(change_id, rule_id)` execution.

use openkeyv::{AsyncCompareAndSwap, AsyncKeyValue};
use serde::{Deserialize, Serialize};

use crate::cache::codec;

const DEFAULT_LEASE_TTL: f64 = 60.0;
const STEAL_THRESHOLD: f64 = 1.0;
const CLAIM_COLLECTION_SUFFIX: &str = "reactor:claims";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Lease {
    owner: String,
    expires_at: i64,
}

#[derive(Debug)]
pub(crate) enum ClaimResult {
    Claimed,
    AlreadyClaimed { owner: String },
}

pub(crate) struct ClaimStore<S> {
    store: S,
    collection: String,
    owner_id: String,
}

impl<S> ClaimStore<S>
where
    S: AsyncKeyValue + AsyncCompareAndSwap + Clone + Send + Sync,
{
    pub(crate) fn new(store: S, namespace: &str, owner_id: impl Into<String>) -> Self {
        Self {
            store,
            collection: format!("{namespace}:{CLAIM_COLLECTION_SUFFIX}"),
            owner_id: owner_id.into(),
        }
    }

    pub(crate) async fn try_claim(
        &self,
        change_id: &str,
        rule_id: &str,
    ) -> Result<ClaimResult, ClaimError> {
        let key = format!("{change_id}:{rule_id}");
        let lease = Lease {
            owner: self.owner_id.clone(),
            expires_at: chrono::Utc::now().timestamp_millis() + (DEFAULT_LEASE_TTL * 1000.0) as i64,
        };
        let value = codec::json_to_value(serde_json::to_value(lease)?)?;

        loop {
            match self
                .store
                .compare_and_swap(
                    &key,
                    None,
                    value.clone(),
                    Some(&self.collection),
                    Some(DEFAULT_LEASE_TTL),
                )
                .await?
            {
                openkeyv::CompareAndSwapResult::Applied { .. } => {
                    return Ok(ClaimResult::Claimed);
                }
                openkeyv::CompareAndSwapResult::Conflict { current } => {
                    let Some(entry) = current else { continue };
                    let current_lease = decode_lease(entry.value)?;
                    let remaining_ttl = entry.ttl.ok_or(ClaimError::LeaseWithoutExpiry)?;
                    if remaining_ttl > STEAL_THRESHOLD {
                        return Ok(ClaimResult::AlreadyClaimed {
                            owner: current_lease.owner,
                        });
                    }
                    match self
                        .store
                        .compare_and_swap(
                            &key,
                            Some(&entry.revision),
                            value.clone(),
                            Some(&self.collection),
                            Some(DEFAULT_LEASE_TTL),
                        )
                        .await?
                    {
                        openkeyv::CompareAndSwapResult::Applied { .. } => {
                            return Ok(ClaimResult::Claimed);
                        }
                        openkeyv::CompareAndSwapResult::Conflict { .. } => continue,
                    }
                }
            }
        }
    }

    pub(crate) async fn release(&self, change_id: &str, rule_id: &str) -> Result<(), ClaimError> {
        let key = format!("{change_id}:{rule_id}");
        loop {
            let Some(entry) = self
                .store
                .get_with_revision(&key, Some(&self.collection))
                .await?
            else {
                return Ok(());
            };
            if decode_lease(entry.value)?.owner != self.owner_id {
                return Ok(());
            }
            match self
                .store
                .compare_and_delete(&key, &entry.revision, Some(&self.collection))
                .await?
            {
                openkeyv::CompareAndDeleteResult::Deleted => return Ok(()),
                openkeyv::CompareAndDeleteResult::Conflict { .. } => continue,
            }
        }
    }
}

fn decode_lease(value: openkeyv::Value) -> Result<Lease, ClaimError> {
    Ok(serde_json::from_value(codec::value_to_json(value)?)?)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ClaimError {
    #[error(transparent)]
    Store(#[from] openkeyv::Error),
    #[error(transparent)]
    Codec(#[from] crate::cache::CacheError),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error("reactor lease is missing its expiry")]
    LeaseWithoutExpiry,
}

#[cfg(test)]
mod tests {
    use openkeyv::{store::memory::MemoryStore, AsyncKeyValue};

    use crate::cache::codec;

    use super::{ClaimResult, ClaimStore};

    #[tokio::test]
    async fn claim_record_is_only_a_lease_and_release_deletes_it() {
        let backend = MemoryStore::new();
        let claims = ClaimStore::new(backend.clone(), "test", "worker-1");

        assert!(matches!(
            claims.try_claim("change", "rule").await.unwrap(),
            ClaimResult::Claimed
        ));

        let value = backend
            .get("change:rule", Some("test:reactor:claims"))
            .await
            .unwrap()
            .expect("lease exists");
        let lease = codec::value_to_json(value).unwrap();
        assert_eq!(
            lease.get("owner").and_then(serde_json::Value::as_str),
            Some("worker-1")
        );
        assert!(lease
            .get("expires_at")
            .and_then(serde_json::Value::as_i64)
            .is_some());
        assert!(lease.get("status").is_none());

        claims.release("change", "rule").await.unwrap();
        assert!(backend
            .get("change:rule", Some("test:reactor:claims"))
            .await
            .unwrap()
            .is_none());
    }
}
