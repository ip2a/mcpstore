//! Cross-process tool projection feed: every projection-relevant KV mutation
//! becomes a shared-store change, and the gateway bridge turns it into a local
//! MCP_TOOLS_CHANGED — reusing the existing catalog invalidation chain instead
//! of a second invalidation mechanism.

use openkeyv::{AsyncChangeFeed, ChangeFeedRequest, ChangeFilter, ChangeOperation, ChangeStart};

use crate::kv::KvLayerManager;
use crate::store::prelude::*;

/// Projection-relevant collections, by layer. This one list drives the shared
/// ChangeFeed filter; the gateway bridge owns the instance-id parsing per type.
const PROJECTION_TYPES: &[(&str, &str)] = &[
    ("entity", "tools"),
    ("entity", "service_instances"),
    ("entity", "service_definitions"),
    ("state", "service_overrides"),
    ("state", "tool_overrides"),
    ("state", "context_tool_visibility"),
    ("relations", "session_tools"),
    ("relations", "session_services"),
    // The relation is the only write when a connect resolves an empty tool
    // set — no per-tool entity accompanies it, so without this entry the
    // gateway would keep serving the previous tool list.
    ("relations", "instance_tools"),
];

impl MCPStore {
    /// Subscribe to every projection-relevant collection on the current
    /// store's ChangeFeed, returning the subscription together with the
    /// store generation it was built on — a consumer watching the event
    /// epoch can tell whether a cutover already happened between this call
    /// and its own startup. Gateway startup calls this before building mounts
    /// so a change landing between mount build and bridge start is not lost.
    pub async fn subscribe_tool_projection_changes(
        &self,
    ) -> Result<(openkeyv::ChangeSubscription, u64)> {
        let snapshot = self.event_log_snapshot().await?;
        let namespace = self.namespace();
        let collections = PROJECTION_TYPES
            .iter()
            .map(|(layer, name)| match *layer {
                "entity" => {
                    KvLayerManager::entity_collection_with_namespace(&namespace, name)
                }
                "state" => KvLayerManager::state_collection_with_namespace(&namespace, name),
                _ => KvLayerManager::relation_collection_with_namespace(&namespace, name),
            })
            .collect();
        let subscription = snapshot
            .log
            .subscribe(ChangeFeedRequest {
                start: ChangeStart::Latest,
                filter: ChangeFilter {
                    collections,
                    operations: vec![ChangeOperation::Put, ChangeOperation::Delete],
                },
            })
            .await
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        Ok((subscription, snapshot.generation))
    }

    /// Watch the event-log epoch: the receiver fires on every store
    /// cutover, positioned at the current generation. Long-lived feed
    /// consumers resubscribe when it moves.
    pub fn subscribe_event_epoch(&self) -> tokio::sync::watch::Receiver<u64> {
        self.kernel.persistence.event_epoch.subscribe()
    }
}
