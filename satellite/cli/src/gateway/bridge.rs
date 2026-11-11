//! Projection bridge: shared-store ChangeFeed → local `MCP_TOOLS_CHANGED`.
//!
//! In shm mode the daemon and the gateway are separate processes on one KV
//! store. Projection writes (tool syncs, override edits, visibility flips)
//! land in the other process; the only signal that crosses the process
//! boundary is the store's ChangeFeed. This single process-level task consumes
//! the projection subscription and re-publishes each change on the local event
//! bus, so the existing catalog invalidation and tools/list_changed chain
//! works unchanged — no second invalidation mechanism.

use std::sync::Arc;
use std::time::Duration;

use mcpstore::events::types::EventKind;
use mcpstore::{InstanceId, MCPStore};
use openkeyv::StoreChange;
use serde_json::{json, Value};

/// Backoff bounds for resubscribing after a stream error; reset on success.
const BRIDGE_BACKOFF_START: Duration = Duration::from_millis(100);
const BRIDGE_BACKOFF_MAX: Duration = Duration::from_secs(5);

/// Consume the projection subscription and republish changes locally. Runs
/// for the gateway process lifetime; on stream end or error it resubscribes
/// with bounded backoff and heals the gap with a broadcast invalidation.
pub(crate) fn spawn_projection_bridge(
    store: Arc<MCPStore>,
    subscription: openkeyv::ChangeSubscription,
    generation: u64,
) {
    tokio::spawn(async move {
        let mut subscription = subscription;
        let mut backoff = BRIDGE_BACKOFF_START;
        // A cutover never announces itself on an alive-but-stale stream
        // (redis/valkey feeds outlive the handle they were built on), so the
        // epoch watch is the only reliable cutover signal.
        let mut epoch = store.subscribe_event_epoch();
        // A cutover between the caller's subscribe and here leaves us holding
        // a live stream on the old store — start in resubscribe mode.
        let mut needs_resubscribe = *epoch.borrow_and_update() != generation;
        loop {
            if !needs_resubscribe {
                tokio::select! {
                    change = subscription.recv() => {
                        match change {
                            Ok(Some(change)) => {
                                backoff = BRIDGE_BACKOFF_START;
                                if let Err(error) = store
                                    .publish_event(
                                        EventKind::McpToolsChanged.as_str(),
                                        projection_event_payload(&change),
                                        false,
                                    )
                                    .await
                                {
                                    tracing::warn!("[GATEWAY] projection bridge publish: {error}");
                                }
                                continue;
                            }
                            Ok(None) => {}
                            Err(error) => {
                                tracing::warn!("[GATEWAY] projection bridge: {error}");
                            }
                        }
                    }
                    changed = epoch.changed() => {
                        // The watch closes only when the store itself goes
                        // down — and takes this gateway process with it.
                        if changed.is_err() {
                            return;
                        }
                        let generation = *epoch.borrow_and_update();
                        tracing::debug!(
                            "[GATEWAY] projection bridge: store generation {generation} cutover, resubscribing"
                        );
                    }
                }
            }
            // Stream ended/errored or the store was cut over: back off,
            // resubscribe, then heal the gap. The null instanceId is a
            // broadcast — every mount's CatalogInvalidator drains (invalidate
            // + warm rebuild) and every subscribed client gets
            // tools/list_changed.
            needs_resubscribe = false;
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(BRIDGE_BACKOFF_MAX);
            match store.subscribe_tool_projection_changes().await {
                Ok((fresh, _)) => {
                    subscription = fresh;
                    backoff = BRIDGE_BACKOFF_START;
                    if let Err(error) = store
                        .publish_event(
                            EventKind::McpToolsChanged.as_str(),
                            json!({ "instanceId": Value::Null, "reason": "bridge_resubscribed" }),
                            false,
                        )
                        .await
                    {
                        tracing::warn!("[GATEWAY] projection bridge heal publish: {error}");
                    }
                }
                Err(error) => {
                    tracing::warn!("[GATEWAY] projection bridge resubscribe: {error}");
                    // Stay in resubscribe mode until one succeeds; a dead
                    // stream would otherwise spin the select below.
                    needs_resubscribe = true;
                }
            }
        }
    });
}

/// Map one StoreChange to the local event payload. The instance-id parsing
/// follows each collection's key layout:
/// - `tools` / `service_instances` / `service_overrides` / `tool_overrides` /
///   `instance_tools`: the instance is the first `:`-segment (bare instance
///   keys have none).
/// - `context_tool_visibility`: keys are `store:<instance>` — the last segment.
/// - `service_definitions` / `session_tools` / `session_services`: instance
///   unknown → broadcast.
fn projection_event_payload(change: &StoreChange) -> Value {
    let type_name = change
        .collection
        .rsplit(':')
        .next()
        .unwrap_or(&change.collection);
    let instance_id = match type_name {
        "tools" | "service_instances" | "service_overrides" | "tool_overrides"
        | "instance_tools" => instance_from_segment(change.key.split(':').next()),
        "context_tool_visibility" => instance_from_segment(change.key.rsplit(':').next()),
        _ => None,
    };
    json!({
        "reason": "store_projection",
        "collection": type_name,
        "instanceId": instance_id
            .map(|instance| json!(instance.to_string()))
            .unwrap_or(Value::Null),
    })
}

fn instance_from_segment(segment: Option<&str>) -> Option<InstanceId> {
    segment?.parse::<InstanceId>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openkeyv::{ChangeCursor, ChangeOperation};

    fn change(collection: &str, key: &str) -> StoreChange {
        StoreChange {
            cursor: ChangeCursor::new("c1"),
            revision: 1,
            collection: collection.to_string(),
            key: key.to_string(),
            operation: ChangeOperation::Put,
            occurred_at: chrono::Utc::now(),
        }
    }

    fn instance_id_of(payload: &Value) -> Option<&str> {
        payload["instanceId"].as_str()
    }

    #[test]
    fn tools_key_yields_first_segment_instance() {
        let payload = projection_event_payload(&change("ns-a:entity:tools", "c81af510-755b-55c7-8487-5668ab36e06e:echo"));
        assert_eq!(instance_id_of(&payload), Some("c81af510-755b-55c7-8487-5668ab36e06e"));
    }

    #[test]
    fn override_keys_yield_their_instance() {
        let service = projection_event_payload(&change(
            "ns-a:state:service_overrides",
            "127ce370-1ed6-5b00-9713-e88d01b3010d",
        ));
        assert_eq!(instance_id_of(&service), Some("127ce370-1ed6-5b00-9713-e88d01b3010d"));
        let tool = projection_event_payload(&change(
            "ns-a:state:tool_overrides",
            "90e3880b-4a70-5f0e-a9b3-9f24c2f6b7aa:orig_tool",
        ));
        assert_eq!(instance_id_of(&tool), Some("90e3880b-4a70-5f0e-a9b3-9f24c2f6b7aa"));
    }

    #[test]
    fn visibility_key_yields_last_segment_instance() {
        let payload = projection_event_payload(&change(
            "ns-a:state:context_tool_visibility",
            "store:6c9d24ee-1f28-5dd9-9c8d-8d81b4a3e5f1",
        ));
        assert_eq!(instance_id_of(&payload), Some("6c9d24ee-1f28-5dd9-9c8d-8d81b4a3e5f1"));
    }

    #[test]
    fn instance_tools_relation_key_yields_its_instance() {
        let payload = projection_event_payload(&change(
            "ns-a:relations:instance_tools",
            "6c9d24ee-1f28-5dd9-9c8d-8d81b4a3e5f1",
        ));
        assert_eq!(instance_id_of(&payload), Some("6c9d24ee-1f28-5dd9-9c8d-8d81b4a3e5f1"));
    }

    #[test]
    fn definitions_and_session_relations_broadcast() {
        let definitions =
            projection_event_payload(&change("ns-a:entity:service_definitions", "svc-a"));
        assert_eq!(definitions["instanceId"], Value::Null);
        let sessions = projection_event_payload(&change(
            "ns-a:relations:session_tools",
            "sess-1:inst-5",
        ));
        assert_eq!(sessions["instanceId"], Value::Null);
    }
}
