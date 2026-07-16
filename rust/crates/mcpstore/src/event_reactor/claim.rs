//! Distributed claim: ensures exactly one instance executes a given (change_id, rule_id).

use std::time::Duration;

use openkeyv::{AsyncCompareAndSwap, AsyncKeyValue};
use serde_json::json;

use crate::cache::codec;

/// Default lease time: 60 seconds. If the claim holder crashes, another
/// instance can steal the claim after this deadline expires.
const DEFAULT_LEASE_TTL: f64 = 60.0;

const CLAIM_COLLECTION_SUFFIX: &str = "reactor:claims";

/// Records the outcome of a claim attempt.
#[derive(Debug)]
pub(crate) enum ClaimResult {
    /// This instance won the claim; proceed with execution.
    Claimed,
    /// Another instance already claimed this work.
    AlreadyClaimed { owner: String },
}

/// Persistent, distributed claim manager backed by openkeyv CAS.
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

    /// Attempt to claim `(change_id, rule_id)` for this instance.
    ///
    /// Uses create-if-absent CAS (`expected = None`). If the key already
    /// exists and the lease has expired, we attempt a steal via CAS on the
    /// stale revision.
    pub(crate) async fn try_claim(
        &self,
        change_id: &str,
        rule_id: &str,
    ) -> Result<ClaimResult, ClaimError> {
        let key = format!("{change_id}:{rule_id}");
        let payload = json!({
            "owner": self.owner_id,
            "status": "claimed",
        });
        let value = codec::json_to_value(payload).map_err(ClaimError::Codec)?;

        // Fast path: create-if-absent
        match self
            .store
            .compare_and_swap(&key, None, value.clone(), Some(&self.collection), Some(DEFAULT_LEASE_TTL))
            .await
            .map_err(ClaimError::Store)?
        {
            openkeyv::CompareAndSwapResult::Applied { .. } => return Ok(ClaimResult::Claimed),
            openkeyv::CompareAndSwapResult::Conflict { current } => {
                // Key exists. Check if lease expired and try to steal.
                if let Some(entry) = current {
                    // The entry's TTL remaining tells us if it's still alive.
                    // CAS conflict returns the current entry; if TTL is 0 or
                    // very small, the lease is effectively expired. However,
                    // openkeyv treats expired entries as absent on next get,
                    // so if we got a `current` back, the lease is still live.
                    let owner = codec::value_to_json(entry.value)
                        .ok()
                        .and_then(|v| v.get("owner").and_then(|o| o.as_str()).map(String::from))
                        .unwrap_or_else(|| "unknown".into());
                    return Ok(ClaimResult::AlreadyClaimed { owner });
                }
                // current is None → entry expired between our get and CAS.
                // Retry the create-if-absent once.
                match self
                    .store
                    .compare_and_swap(&key, None, value, Some(&self.collection), Some(DEFAULT_LEASE_TTL))
                    .await
                    .map_err(ClaimError::Store)?
                {
                    openkeyv::CompareAndSwapResult::Applied { .. } => Ok(ClaimResult::Claimed),
                    openkeyv::CompareAndSwapResult::Conflict { current: _ } => {
                        Ok(ClaimResult::AlreadyClaimed {
                            owner: "race".into(),
                        })
                    }
                }
            }
        }
    }

    /// Mark a claim as succeeded (write final state, remove TTL).
    pub(crate) async fn mark_succeeded(
        &self,
        change_id: &str,
        rule_id: &str,
    ) -> Result<(), ClaimError> {
        self.write_status(change_id, rule_id, "succeeded", None).await
    }

    /// Mark a claim as failed (write final state with optional TTL for cleanup).
    pub(crate) async fn mark_failed(
        &self,
        change_id: &str,
        rule_id: &str,
        ttl: Option<Duration>,
    ) -> Result<(), ClaimError> {
        self.write_status(change_id, rule_id, "failed", ttl).await
    }

    async fn write_status(
        &self,
        change_id: &str,
        rule_id: &str,
        status: &str,
        ttl: Option<Duration>,
    ) -> Result<(), ClaimError> {
        let key = format!("{change_id}:{rule_id}");
        let payload = json!({
            "owner": self.owner_id,
            "status": status,
        });
        let value = codec::json_to_value(payload).map_err(ClaimError::Codec)?;
        let ttl_secs = ttl.map(|d| d.as_secs_f64());

        loop {
            let revisioned = self
                .store
                .get_with_revision(&key, Some(&self.collection))
                .await
                .map_err(ClaimError::Store)?;
            let expected = revisioned.as_ref().map(|r| &r.revision);
            match self
                .store
                .compare_and_swap(&key, expected, value.clone(), Some(&self.collection), ttl_secs)
                .await
                .map_err(ClaimError::Store)?
            {
                openkeyv::CompareAndSwapResult::Applied { .. } => return Ok(()),
                openkeyv::CompareAndSwapResult::Conflict { .. } => continue,
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum ClaimError {
    Store(openkeyv::Error),
    Codec(crate::cache::CacheError),
}

impl std::fmt::Display for ClaimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "store: {e}"),
            Self::Codec(e) => write!(f, "codec: {e}"),
        }
    }
}
