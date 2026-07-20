//! EventReactor integration for typed control requests.

use std::sync::Arc;

use crate::control::request::{ControlRequest, ControlRequestStatus};
use crate::event_reactor::{ChangeContext, ReactionContext, ReactionOutcome, ReactorConfig, Rule};
use crate::store::prelude::*;

impl MCPStore {
    pub async fn restart_control_reactor(self: &Arc<Self>) -> Result<()> {
        self.process_control_requests().await?;
        let config = ReactorConfig {
            subscriber_id: format!("api-control-{}", std::process::id()),
            owner_id: format!("api-control-{}", std::process::id()),
            namespace: self.namespace(),
            ..Default::default()
        };
        self.setup_event_reactor(config).await?;
        self.register_rule(self.control_request_rule()).await?;
        self.start_reactor().await
    }

    pub fn control_request_rule(self: &Arc<Self>) -> Rule {
        let collection = self.cache().event_collection(CONTROL_REQUEST_EVENT_TYPE);

        Rule::new(
            "mcpstore:control-requests:v2",
            {
                let collection = collection.clone();
                move |ctx: ChangeContext| {
                    let collection = collection.clone();
                    Box::pin(async move {
                        if ctx.collection != collection {
                            return false;
                        }
                        ctx.value
                            .and_then(|value| serde_json::from_value::<ControlRequest>(value).ok())
                            .is_some_and(|request| request.status == ControlRequestStatus::Queued)
                    })
                }
            },
            {
                let store = Arc::clone(self);
                move |ctx: ReactionContext| {
                    let store = Arc::clone(&store);
                    Box::pin(async move {
                        let Some(value) = ctx.value else {
                            return ReactionOutcome::Failed(
                                "control request was deleted before execution".to_string(),
                            );
                        };
                        let mut request = match serde_json::from_value::<ControlRequest>(value) {
                            Ok(request) => request,
                            Err(error) => {
                                return ReactionOutcome::Failed(format!(
                                    "control request deserialization failed: {error}"
                                ));
                            }
                        };

                        request.status = ControlRequestStatus::Executing {
                            started_at: chrono::Utc::now().timestamp_millis(),
                        };
                        if let Err(error) = store
                            .cache()
                            .put_event(
                                CONTROL_REQUEST_EVENT_TYPE,
                                &ctx.key,
                                match serde_json::to_value(&request) {
                                    Ok(value) => value,
                                    Err(error) => {
                                        return ReactionOutcome::Failed(format!(
                                            "control request serialization failed: {error}"
                                        ));
                                    }
                                },
                            )
                            .await
                        {
                            return ReactionOutcome::Retryable(format!(
                                "failed to persist executing control request: {error}"
                            ));
                        }

                        let result = store.apply_control_request(&request).await;
                        request.status = match &result {
                            Ok(()) => ControlRequestStatus::Applied {
                                applied_at: chrono::Utc::now().timestamp_millis(),
                            },
                            Err(error) => ControlRequestStatus::Rejected {
                                rejected_at: chrono::Utc::now().timestamp_millis(),
                                reason: error.to_string(),
                            },
                        };

                        let value = match serde_json::to_value(request) {
                            Ok(value) => value,
                            Err(error) => {
                                return ReactionOutcome::Failed(format!(
                                    "control request serialization failed: {error}"
                                ));
                            }
                        };
                        if let Err(error) = store
                            .cache()
                            .put_event(CONTROL_REQUEST_EVENT_TYPE, &ctx.key, value)
                            .await
                        {
                            return ReactionOutcome::Retryable(format!(
                                "failed to persist final control request state: {error}"
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
