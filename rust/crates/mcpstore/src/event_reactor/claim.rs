//! Distributed claim: ensures exactly one instance executes a given (change_id, rule_id).
//!
//! Lease semantics: each claim carries a TTL. When the claim holder finishes
//! successfully, it calls `mark_succeeded` (removes TTL). On failure, it calls
//! `mark_failed` with a cleanup TTL. If the holder crashes, the TTL expires
//! naturally and another instance can re-claim via the create-if-absent CAS path.

use std::time::Duration;

use openkeyv::{AsyncCompareAndSwap, AsyncKeyValue, Revision};
use serde_json::json;

use crate::cache::codec;

/// Default lease time in seconds. If the claim holder crashes, the TTL
/// expires and another instance can re-claim via create-if-absent CAS.
const DEFAULT_LEASE_TTL: f64 = 60.0;

/// Threshold below which a remaining TTL is considered "about to expire"
/// and we attempt to steal. In seconds.
const STEAL_THRESHOLD: f64 = 1.0;

const CLAIM_COLLECTION_SUFFIX: &str = "reactor:claims";

#[derive(Debug)]
pub(crate) enum ClaimResult {
    /// This instance won the claim; proceed with execution.
    Claimed,
    /// Another instance already claimed this work and the lease is still live.
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

    /// Attempt to claim `(change_id, rule_id)` for this instance.
    ///
    /// Strategy:
    /// 1. Create-if-absent CAS (`expected = None`). If the key doesn't exist
    ///    (or has expired), we win immediately.
    /// 2. If the key exists with remaining TTL < STEAL_THRESHOLD, we attempt
    ///    to steal it via CAS on the observed revision.
    /// 3. Otherwise, another instance holds a live claim.
    pub(crate) async fn try_claim(
        &self,
        change_id: &str,
        rule_id: &str,
    ) -> Result<ClaimResult, ClaimError> {
        let key = format!("{change_id}:{rule_id}");
        let payload = json!({ "owner": self.owner_id, "status": "claimed" });
        let value = codec::json_to_value(payload).map_err(ClaimError::Codec)?;

        // Fast path: create-if-absent
        match self
            .store
            .compare_and_swap(
                &key,
                None,
                value.clone(),
                Some(&self.collection),
                Some(DEFAULT_LEASE_TTL),
            )
            .await
            .map_err(ClaimError::Store)?
        {
            openkeyv::CompareAndSwapResult::Applied { .. } => {
            return Ok(ClaimResult::Claimed);
        }
            openkeyv::CompareAndSwapResult::Conflict { current } => {
                let entry = match current {
                    Some(e) => e,
                    None => {
                        // Entry expired between CAS and conflict response.
                        // Retry create-if-absent once.
                        return match self
                            .store
                            .compare_and_swap(
                                &key,
                                None,
                                value,
                                Some(&self.collection),
                                Some(DEFAULT_LEASE_TTL),
                            )
                            .await
                            .map_err(ClaimError::Store)?
                        {
                            openkeyv::CompareAndSwapResult::Applied { .. } => Ok(ClaimResult::Claimed),
                            openkeyv::CompareAndSwapResult::Conflict { .. } => Ok(
                                ClaimResult::AlreadyClaimed { owner: "race".into() },
                            ),
                        };
                    }
                };

                // Check if the lease is about to expire — attempt steal.
                // None TTL means permanent (never expires) → cannot steal.
                let remaining_ttl = match entry.ttl {
                    None => return Ok(ClaimResult::AlreadyClaimed {
                        owner: extract_owner(&entry.value),
                    }),
                    Some(t) => t,
                };
                if remaining_ttl > STEAL_THRESHOLD {
                    let owner = extract_owner(&entry.value);
                    return Ok(ClaimResult::AlreadyClaimed { owner });
                }

                // TTL is below threshold: try to steal via CAS on the observed revision
                let stale_revision: &Revision = &entry.revision;
                match self
                    .store
                    .compare_and_swap(
                        &key,
                        Some(stale_revision),
                        value,
                        Some(&self.collection),
                        Some(DEFAULT_LEASE_TTL),
                    )
                    .await
                    .map_err(ClaimError::Store)?
                {
                    openkeyv::CompareAndSwapResult::Applied { .. } => {
                        debug_assert!(true, "stole expired lease");
                        Ok(ClaimResult::Claimed)
                    }
                    openkeyv::CompareAndSwapResult::Conflict { current: _ } => {
                        Ok(ClaimResult::AlreadyClaimed { owner: "stolen-by-other".into() })
                    }
                }
            }
        }
    }

    /// Mark a claim as succeeded (write final state, no TTL).
    pub(crate) async fn mark_succeeded(
        &self,
        change_id: &str,
        rule_id: &str,
    ) -> Result<(), ClaimError> {
        self.write_status(change_id, rule_id, "succeeded", None).await
    }

    /// Mark a claim as failed (write final state with cleanup TTL).
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
        let payload = json!({ "owner": self.owner_id, "status": status });
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
                .compare_and_swap(
                    &key,
                    expected,
                    value.clone(),
                    Some(&self.collection),
                    ttl_secs,
                )
                .await
                .map_err(ClaimError::Store)?
            {
                openkeyv::CompareAndSwapResult::Applied { .. } => return Ok(()),
                openkeyv::CompareAndSwapResult::Conflict { .. } => continue,
            }
        }
    }
}

fn extract_owner(value: &openkeyv::Value) -> String {
    codec::value_to_json(value.clone())
        .ok()
        .and_then(|v| v.get("owner").and_then(|o| o.as_str()).map(String::from))
        .unwrap_or_else(|| "unknown".into())
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
