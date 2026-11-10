//! Cross-panel tool-call RPC (plan B): panels share no connections; everything goes through the shared store.
//!
//! Data panel: `call_tool_remote` subscribes to the response stream first, writes the request, then blocks in select until
//! the control panel writes back. Control panel: subscribes to `tool_call_requests`, atomically claims on wake
//! (claim = get_with_revision + compare_and_delete, exactly one winner),
//! executes locally, then writes the response into `tool_call_responses`.
//!
//! Streaming responses are unsupported: the response is written only after the result fully completes.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use openkeyv::{AsyncChangeFeed, ChangeFeedRequest, ChangeFilter, ChangeOperation, ChangeStart};
use serde_json::Value;

use crate::cache::models::{ToolCallRequestEntity, ToolCallResponseEntity, ToolCallResponseError};
use crate::cache::CacheLayerManager;
use crate::store::prelude::*;
use crate::store::{EventBackendSnapshot, MCPStore, PanelRole};

const REQUESTS: &str = "tool_call_requests";
const RESPONSES: &str = "tool_call_responses";
/// Upper bound for the data panel waiting on a response; the control panel claims expired requests and drops them.
const CALL_DEADLINE: Duration = Duration::from_secs(30);
/// Response TTL: normally deleted when the data panel reads it; if the data panel dies, the control panel cleans it up.
const RESPONSE_TTL: Duration = Duration::from_secs(300);

impl MCPStore {
    /// Data panel: write the request into the shared store and wait for the control panel to write the response back.
    /// ponytail: a fresh subscription per call; switch to a long-lived subscription + waiter table under load.
    pub(crate) async fn call_tool_remote(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: Value,
    ) -> Result<crate::transport::ToolCallResult> {
        let PanelRole::DataPanel { panel_id } = &self.kernel.runtime.panel_role else {
            return Err(Error::new(
                FailureCode::Internal,
                "remote tool call is data-panel only".to_string(),
            ));
        };
        let request_id = uuid::Uuid::new_v4().to_string();
        let response_key = format!("{panel_id}:{request_id}");

        // Subscribe before writing: Latest subscriptions can't see writes made before subscribing; the reverse order loses responses.
        let mut snapshot = self.event_backend_snapshot().await?;
        let mut subscription = self.subscribe_response_feed(&snapshot).await?;

        let request = serde_json::to_value(ToolCallRequestEntity {
            request_id: request_id.clone(),
            panel_id: panel_id.clone(),
            instance_id,
            tool_name: tool_name.to_string(),
            arguments: args,
            deadline_unix_ms: (chrono::Utc::now() + CALL_DEADLINE).timestamp_millis(),
        })
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        self.cache()
            .put_entity(REQUESTS, &request_id, request)
            .await?;
        // Total deadline, measured from the first request. Rebuilt subscriptions
        // must not reset it — a cutover would otherwise give the call 30 more
        // seconds every time it lands.
        let deadline = tokio::time::Instant::now() + CALL_DEADLINE;

        loop {
            tokio::select! {
                change = subscription.recv() => {
                    let change = change
                        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?
                        .ok_or_else(|| {
                            Error::new(FailureCode::Internal, "tool call feed closed".to_string())
                        })?;
                    if change.key != response_key {
                        continue;
                    }
                    match self.take_tool_call_response(&response_key).await? {
                        Some(outcome) => return outcome,
                        // Someone else consumed it or it vanished mid-read; keep waiting.
                        None => continue,
                    }
                }
                changed = snapshot.changes.changed() => {
                    // The backend was swapped under us. Re-resolve, rebuild the
                    // subscription on the new generation and read the response
                    // once — a response written into the new store while the old
                    // subscription was dying must not be missed.
                    changed.map_err(|_| {
                        Error::new(FailureCode::Internal, "event epoch watch closed")
                    })?;
                }
                _ = tokio::time::sleep_until(deadline) => {
                    // On timeout delete the request; if the control panel already claimed it, the TTL cleans up the response.
                    self.cache().delete_entity(REQUESTS, &request_id).await.ok();
                    return Err(Error::new(
                        FailureCode::CallTimedOut,
                        format!("remote tool call '{tool_name}' timed out"),
                    ));
                }
            }
            snapshot = self.event_backend_snapshot().await?;
            subscription = self.subscribe_response_feed(&snapshot).await?;
            if let Some(outcome) = self.take_tool_call_response(&response_key).await? {
                return outcome;
            }
        }
    }

    /// Subscribe to response puts on the current generation's backend.
    async fn subscribe_response_feed(
        &self,
        snapshot: &EventBackendSnapshot,
    ) -> Result<openkeyv::ChangeSubscription> {
        let collection =
            CacheLayerManager::entity_collection_with_namespace(&self.namespace(), RESPONSES);
        snapshot
            .backend
            .subscribe(ChangeFeedRequest {
                start: ChangeStart::Latest,
                filter: ChangeFilter {
                    collections: vec![collection],
                    operations: vec![ChangeOperation::Put],
                },
            })
            .await
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))
    }

    /// Read-and-delete one response entity. `Ok(None)` means it isn't there —
    /// not yet written, or already consumed by the TTL cleanup. Shared by the
    /// resubscribe path and the event path so a response that landed while a
    /// subscription was being rebuilt is still picked up.
    async fn take_tool_call_response(
        &self,
        response_key: &str,
    ) -> Result<Option<Result<crate::transport::ToolCallResult>>> {
        let Some(value) = self.cache().get_entity(RESPONSES, response_key).await? else {
            return Ok(None);
        };
        self.cache().delete_entity(RESPONSES, response_key).await.ok();
        let response: ToolCallResponseEntity = serde_json::from_value(value)
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        Ok(Some(match (response.result, response.error) {
            (Some(result), _) => Ok(result),
            (None, Some(error)) => Err(remote_error(&error)),
            (None, None) => Err(Error::new(
                FailureCode::Internal,
                "tool call response carries neither result nor error".to_string(),
            )),
        }))
    }

    /// Control panel: subscribe to tool-call requests and proxy-execute them. Call after setup (or load_from_config).
    pub(crate) fn spawn_tool_call_request_feed(self: &Arc<Self>) {
        if !matches!(self.kernel.runtime.panel_role, PanelRole::ControlPanel) {
            return;
        }
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        if self
            .kernel
            .runtime
            .tool_call_feed_started
            .swap(true, Ordering::AcqRel)
        {
            return;
        }
        let store = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                match store.consume_tool_call_requests().await {
                    Ok(()) => {}
                    Err(error) if error.message().contains("does not provide ChangeFeed") => {
                        tracing::warn!("[STORE] tool call feed: {error}");
                        break;
                    }
                    Err(error) => {
                        tracing::warn!("[STORE] tool call feed resubscribe: {error}");
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            store
                .kernel
                .runtime
                .tool_call_feed_started
                .store(false, Ordering::Release);
        });
    }

    pub(crate) fn spawn_tool_call_request_feed_from_ref(&self) {
        let Some(store) = self
            .kernel
            .runtime
            .self_weak
            .get()
            .and_then(|weak| weak.upgrade())
        else {
            return;
        };
        store.spawn_tool_call_request_feed();
    }

    async fn consume_tool_call_requests(self: &Arc<Self>) -> Result<()> {
        // Loop over backend generations: a cutover (epoch changed) resolves
        // the new backend, subscribes on it and drains it — never falls back
        // to the 1s retry sleep.
        loop {
            let mut snapshot = self.event_backend_snapshot().await?;
            let collection = CacheLayerManager::entity_collection_with_namespace(
                &self.namespace(),
                REQUESTS,
            );
            let mut subscription = snapshot
                .backend
                .subscribe(ChangeFeedRequest {
                    start: ChangeStart::Latest,
                    filter: ChangeFilter {
                        collections: vec![collection],
                        operations: vec![ChangeOperation::Put],
                    },
                })
                .await
                .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
            // After subscribing, scan existing requests: ones the data panel wrote before the control panel started, not yet expired.
            self.drain_tool_call_requests().await;
            loop {
                tokio::select! {
                    change = subscription.recv() => match change {
                        Ok(Some(change)) => {
                            let store = Arc::clone(self);
                            // Tool calls can be slow; spawn per request so the consume loop isn't blocked.
                            // ponytail: no concurrency cap; add a semaphore under pressure.
                            let request_id = change.key;
                            tokio::spawn(async move {
                                store.handle_tool_call_request(&request_id).await;
                            });
                        }
                        Ok(None) => return Ok(()),
                        Err(error) => {
                            return Err(Error::new(FailureCode::Internal, error.to_string()));
                        }
                    },
                    changed = snapshot.changes.changed() => {
                        changed.map_err(|_| {
                            Error::new(FailureCode::Internal, "event epoch watch closed")
                        })?;
                        tracing::debug!(
                            "[STORE] tool call request feed: backend generation {}, resubscribing",
                            snapshot.generation
                        );
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        // Safety rescan: store-swap self-healing (drain uses the active store) + missed-push recovery.
                        // Expired requests are dropped once claimed; rescans don't revive them.
                        self.drain_tool_call_requests().await;
                    }
                }
            }
        }
    }

    async fn drain_tool_call_requests(self: &Arc<Self>) {
        let Ok(requests) = self.cache().get_all_entities_async(REQUESTS).await else {
            return;
        };
        for request_id in requests.keys() {
            let store = Arc::clone(self);
            let request_id = request_id.clone();
            tokio::spawn(async move {
                store.handle_tool_call_request(&request_id).await;
            });
        }
    }

    async fn handle_tool_call_request(self: &Arc<Self>, request_id: &str) {
        let Some(value) = self
            .cache()
            .claim_entity(REQUESTS, request_id)
            .await
            .ok()
            .flatten()
        else {
            // Absent or already atomically claimed by someone else
            return;
        };
        let request: ToolCallRequestEntity = match serde_json::from_value(value) {
            Ok(request) => request,
            Err(error) => {
                tracing::warn!("[STORE] tool call request {request_id} unreadable: {error}");
                return;
            }
        };
        if chrono::Utc::now().timestamp_millis() > request.deadline_unix_ms {
            tracing::debug!("[STORE] tool call request {request_id} expired; dropped");
            return;
        }

        // Connect before calling: override resolution validates tool names against synced tools (same order as the local path).
        let started = std::time::Instant::now();
        let executed = async {
            self.ensure_instance_connected(request.instance_id).await?;
            self.call_tool(
                request.instance_id,
                &request.tool_name,
                request.arguments.clone(),
            )
            .await
        }
        .await;
        tracing::info!(
            "[STORE] remote tool call '{}' for panel '{}' took {}ms: {}",
            request.tool_name,
            request.panel_id,
            started.elapsed().as_millis(),
            if executed.is_ok() { "ok" } else { "failed" }
        );
        let response = match executed {
            Ok(result) => ToolCallResponseEntity {
                request_id: request.request_id.clone(),
                panel_id: request.panel_id.clone(),
                result: Some(result),
                error: None,
            },
            Err(error) => ToolCallResponseEntity {
                request_id: request.request_id.clone(),
                panel_id: request.panel_id.clone(),
                result: None,
                error: Some(ToolCallResponseError {
                    code: error.code(),
                    message: error.message().to_string(),
                }),
            },
        };

        let response_key = format!("{}:{}", request.panel_id, request.request_id);
        let payload = serde_json::to_value(&response).unwrap_or_default();
        if let Err(error) = self
            .cache()
            .put_entity(RESPONSES, &response_key, payload)
            .await
        {
            tracing::error!("[STORE] tool call response {response_key} unwritable: {error}");
            return;
        }

        // TTL safety net: if the data panel dies and nobody reads the response, clean it up after 5 minutes.
        let store = Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(RESPONSE_TTL).await;
            store
                .cache()
                .delete_entity(RESPONSES, &response_key)
                .await
                .ok();
        });
    }
}

fn remote_error(error: &ToolCallResponseError) -> Error {
    Error::new(error.code, error.message.clone())
}
