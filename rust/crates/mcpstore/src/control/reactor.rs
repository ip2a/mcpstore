//! EventReactor integration: builds a [`Rule`] that reacts to `control_requests`
//! changes and dispatches them through [`MCPStore::apply_control_request`].
//!
//! This replaces the old 1-second polling scanner in the API server. Control
//! requests are now processed via push-based ChangeFeed events.

use std::sync::Arc;

use crate::event_reactor::{ChangeContext, ReactionContext, ReactionOutcome, Rule};
use crate::store::prelude::*;

impl MCPStore {
    /// Build a [`Rule`] that processes control_requests via the EventReactor.
    ///
    /// Pass the same `Arc<MCPStore>` that owns the reactor. The Rule matches
    /// when the change is in the `control_requests` collection **and** the
    /// record status is `"pending"`. The reaction applies the request and
    /// writes back `completed` or `failed`.
    pub fn control_request_rule(self: &Arc<Self>) -> Rule {
        let collection = self
            .cache()
            .event_collection(CONTROL_REQUEST_EVENT_TYPE);

        Rule::new(
            "mcpstore:control-requests:v1",
            {
                let collection = collection.clone();
                move |ctx: ChangeContext| {
                    let collection = collection.clone();
                    Box::pin(async move {
                        if ctx.collection != collection {
                            return false;
                        }
                        ctx.value
                            .as_ref()
                            .and_then(|v| v.get("status"))
                            .and_then(|s| s.as_str())
                            == Some("pending")
                    })
                }
            },
            {
                let store = Arc::clone(self);
                move |ctx: ReactionContext| {
                    let store = Arc::clone(&store);
                    Box::pin(async move {
                        let Some(mut event) = ctx.value.clone() else {
                            return ReactionOutcome::Failed(
                                "control request value is None (deleted?)".into(),
                            );
                        };

                        let result = store.apply_control_request(&event).await;
                        if let Some(obj) = event.as_object_mut() {
                            obj.insert(
                                "processed_at".to_string(),
                                serde_json::json!(chrono::Utc::now().timestamp_millis()),
                            );
                            match &result {
                                Ok(()) => {
                                    obj.insert(
                                        "status".to_string(),
                                        serde_json::json!("completed"),
                                    );
                                }
                                Err(error) => {
                                    obj.insert(
                                        "status".to_string(),
                                        serde_json::json!("failed"),
                                    );
                                    obj.insert(
                                        "error".to_string(),
                                        serde_json::json!(error.to_string()),
                                    );
                                }
                            }
                        }

                        if let Err(error) = store
                            .cache()
                            .put_event(CONTROL_REQUEST_EVENT_TYPE, &ctx.key, event)
                            .await
                        {
                            return ReactionOutcome::Retryable(format!(
                                "failed to write back control request status: {error}"
                            ));
                        }

                        match result {
                            Ok(()) => ReactionOutcome::Ok,
                            Err(error) => ReactionOutcome::Failed(error.to_string()),
                        }
                    })
                }
            },
        )
    }
}
