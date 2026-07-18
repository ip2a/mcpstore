//! EventReactor: subscribes to openkeyv ChangeFeed, matches rules, executes reactions.
//!
//! This is mcpstore's event-reaction mechanism. It replaces the old polling
//! scanner (`control/queue.rs`) with a push-based model:
//!
//! 1. openkeyv store mutation → atomically records a StoreChange
//! 2. ChangeFeed pushes the change to all subscribers
//! 3. EventReactor reads the change, matches registered Rules (when)
//! 4. For each matching rule, claims execution via CAS ((change_id, rule_id))
//! 5. The claiming instance executes the reaction (then) in a bounded task pool
//! 6. Success/failure state written back to openkeyv; cursor advances at ack point
//!
//! Recursion guard: internal collections (`reactor:cursors`, `reactor:claims`, `reactor:executions`)
//! are always filtered out before rule matching, preventing self-triggered loops.
//! Causation depth is tracked and capped at `max_causation_depth`.

mod backend;
mod claim;
mod cursor;
mod execution;
mod rule;
#[cfg(test)]
mod tests;

pub use backend::EventBackend;

use std::sync::Arc;

use openkeyv::{
    AsyncChangeFeed, AsyncCompareAndSwap, AsyncKeyValue, ChangeFeedRequest, ChangeFilter,
    ChangeOperation, ChangeStart, ChangeSubscription,
};
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub use rule::{ChangeContext, ReactionContext, ReactionOutcome, Rule};

use crate::events::{Event, EventBus};

use claim::{ClaimResult, ClaimStore};
use cursor::CursorStore;
use execution::{ReactionExecutionStatus, ReactionExecutionStore};

/// Internal collection suffixes that must never trigger reactions.
const INTERNAL_SUFFIXES: &[&str] = &["reactor:cursors", "reactor:claims", "reactor:executions"];

/// Configuration for creating an EventReactor.
#[derive(Clone, Debug)]
pub struct ReactorConfig {
    /// Stable subscriber identity for cursor persistence.
    pub subscriber_id: String,
    /// Unique instance identity for claim ownership.
    pub owner_id: String,
    /// Namespace prefix (same as the CacheLayerManager namespace).
    pub namespace: String,
    /// Collections to watch. If empty, watches ALL collections.
    pub watch_collections: Vec<String>,
    /// Maximum causation chain depth. When a reaction writes new events that
    /// trigger further reactions, the depth increases. At `max_causation_depth`
    /// the reactor stops the chain to prevent infinite recursion.
    pub max_causation_depth: u32,
}

impl Default for ReactorConfig {
    fn default() -> Self {
        Self {
            subscriber_id: "reactor-default".into(),
            owner_id: "reactor-default".into(),
            namespace: "mcpstore".into(),
            watch_collections: Vec::new(),
            max_causation_depth: 16,
        }
    }
}

/// The EventReactor: owns the feed subscription loop and dispatches reactions.
pub struct EventReactor<S>
where
    S: AsyncChangeFeed + AsyncCompareAndSwap + AsyncKeyValue + Clone + Send + Sync + 'static,
{
    store: S,
    config: ReactorConfig,
    rules: RwLock<Vec<Rule>>,
    cursor_store: CursorStore<S>,
    claim_store: ClaimStore<S>,
    execution_store: ReactionExecutionStore<S>,
    /// Optional EventBus bridge: when set, reaction outcomes are published
    /// here so they become visible via `/events/history` and TUI.
    event_bus: Option<crate::events::EventBus>,
    shutdown_tx: RwLock<Option<mpsc::Sender<()>>>,
    feed_task: RwLock<Option<tokio::task::JoinHandle<()>>>,
}

#[derive(Debug)]
pub enum ReactorError {
    Store(openkeyv::Error),
    Cursor(String),
    Claim(String),
    AlreadyRunning,
}

impl std::fmt::Display for ReactorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "store error: {e}"),
            Self::Cursor(e) => write!(f, "cursor error: {e}"),
            Self::Claim(e) => write!(f, "claim error: {e}"),
            Self::AlreadyRunning => write!(f, "reactor already running"),
        }
    }
}

impl std::error::Error for ReactorError {}

/// Check if a collection belongs to the reactor's internal state.
/// These must never trigger user rules.
fn is_internal_collection(collection: &str, namespace: &str) -> bool {
    INTERNAL_SUFFIXES
        .iter()
        .any(|suffix| collection == &format!("{namespace}:{suffix}"))
}

impl<S> EventReactor<S>
where
    S: AsyncChangeFeed + AsyncCompareAndSwap + AsyncKeyValue + Clone + Send + Sync + 'static,
{
    pub fn new(store: S, config: ReactorConfig) -> Self {
        let cursor_store =
            CursorStore::new(store.clone(), &config.namespace, &config.subscriber_id);
        let claim_store = ClaimStore::new(store.clone(), &config.namespace, &config.owner_id);
        let execution_store = ReactionExecutionStore::new(store.clone(), &config.namespace);
        Self {
            store,
            config,
            rules: RwLock::new(Vec::new()),
            cursor_store,
            claim_store,
            execution_store,
            event_bus: None,
            shutdown_tx: RwLock::new(None),
            feed_task: RwLock::new(None),
        }
    }

    /// Attach an EventBus so reaction outcomes are published as events, making
    /// them visible via `/events/history` and TUI. Should be called before `start()`.
    pub fn with_event_bus(mut self, bus: EventBus) -> Self {
        self.event_bus = Some(bus);
        self
    }

    pub async fn register(&self, rule: Rule) {
        let mut rules = self.rules.write().await;
        info!(rule_id = rule.id(), "registered reactor rule");
        rules.push(rule);
    }

    pub async fn start(self: &Arc<Self>) -> Result<(), ReactorError> {
        let mut shutdown = self.shutdown_tx.write().await;
        if shutdown.is_some() {
            return Err(ReactorError::AlreadyRunning);
        }
        let (tx, rx) = mpsc::channel::<()>(1);
        *shutdown = Some(tx);

        let start = match self
            .cursor_store
            .load()
            .await
            .map_err(|e| ReactorError::Cursor(e.to_string()))?
        {
            Some(cursor_str) => {
                info!(subscriber = %self.config.subscriber_id, cursor = %cursor_str, "resuming from saved cursor");
                ChangeStart::After(openkeyv::ChangeCursor::new(cursor_str))
            }
            None => {
                info!(subscriber = %self.config.subscriber_id, "no saved cursor, starting from beginning");
                ChangeStart::Beginning
            }
        };

        let filter = ChangeFilter {
            collections: self.config.watch_collections.clone(),
            operations: Vec::new(),
        };

        let subscription = match self
            .store
            .subscribe(ChangeFeedRequest {
                start: start.clone(),
                filter: filter.clone(),
            })
            .await
        {
            Ok(sub) => sub,
            Err(openkeyv::Error::ChangeCursorExpired { requested, oldest }) => {
                warn!(
                    subscriber = %self.config.subscriber_id,
                    requested = %requested,
                    oldest = %oldest,
                    "cursor expired, falling back to beginning"
                );
                // Reset persisted cursor and resubscribe from beginning
                self.cursor_store
                    .save(&openkeyv::ChangeCursor::new(&oldest).to_string())
                    .await
                    .map_err(|e| ReactorError::Cursor(e.to_string()))?;
                self.store
                    .subscribe(ChangeFeedRequest {
                        start: ChangeStart::Beginning,
                        filter,
                    })
                    .await
                    .map_err(ReactorError::Store)?
            }
            Err(e) => return Err(ReactorError::Store(e)),
        };

        info!("event reactor started, dispatching feed loop");

        let this = self.clone();
        let handle = tokio::spawn(async move {
            this.feed_loop(subscription, rx).await;
        });
        *self.feed_task.write().await = Some(handle);

        Ok(())
    }

    pub async fn shutdown(&self) {
        let tx = {
            let mut shutdown = self.shutdown_tx.write().await;
            shutdown.take()
        };
        if let Some(tx) = tx {
            let _ = tx.send(()).await;
        }
        let handle = {
            let mut feed = self.feed_task.write().await;
            feed.take()
        };
        if let Some(handle) = handle {
            let _ = handle.await;
        }
    }

    async fn feed_loop(
        self: Arc<Self>,
        mut subscription: ChangeSubscription,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("event reactor shutting down");
                    break;
                }
                recv = subscription.recv() => {
                    match recv {
                        Err(openkeyv::Error::ChangeCursorExpired { requested, oldest }) => {
                            warn!(
                                subscriber = %self.config.subscriber_id,
                                requested = %requested,
                                oldest = %oldest,
                                "cursor expired during feed, resubscribing from oldest available"
                            );
                            let _ = self
                                .cursor_store
                                .save(&openkeyv::ChangeCursor::new(&oldest).to_string())
                                .await;
                            let cursor = openkeyv::ChangeCursor::new(oldest);
                            match self.store.subscribe(ChangeFeedRequest {
                                start: ChangeStart::After(cursor),
                                filter: ChangeFilter {
                                    collections: self.config.watch_collections.clone(),
                                    operations: Vec::new(),
                                },
                            }).await {
                                Ok(new_sub) => {
                                    info!("resubscribed after cursor expiry");
                                    subscription = new_sub;
                                }
                                Err(e) => {
                                    error!(error = %e, "failed to resubscribe after cursor expiry, reactor stopping");
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "change feed error, reactor stopping");
                            break;
                        }
                        Ok(None) => {
                            info!("change feed closed, reactor stopping");
                            break;
                        }
                        Ok(Some(change)) => {
                            self.handle_change(change).await;
                        }
                    }
                }
            }
        }
    }

    /// Process a single StoreChange: filter internals, read value, match rules,
    /// claim, dispatch reaction via bounded channel, advance cursor at ack point.
    async fn handle_change(&self, change: openkeyv::StoreChange) {
        let collection = change.collection.clone();
        let key = change.key.clone();
        let change_id = change.cursor.to_string();
        let namespace = self.config.namespace.clone();

        // ── Recursion guard: skip internal collections ──
        if is_internal_collection(&collection, &namespace) {
            debug!(collection = %collection, "skipping internal collection");
            // Still advance cursor
            if let Err(e) = self.cursor_store.save(&change_id).await {
                warn!(error = ?e, "failed to save cursor for internal collection");
            }
            return;
        }

        // ── Read current value ──
        let current_value = if change.operation == ChangeOperation::Delete {
            None
        } else {
            match self.store.get(&key, Some(&collection)).await {
                Ok(v) => v,
                Err(e) => {
                    error!(collection = %collection, key = %key, error = %e, "failed to read value for change, skipping");
                    if let Err(e) = self.cursor_store.save(&change_id).await {
                        warn!(error = ?e, "failed to save cursor after read error");
                    }
                    return;
                }
            }
        };

        let json_value = match current_value.as_ref() {
            Some(v) => match crate::cache::codec::value_to_json(v.clone()) {
                Ok(j) => Some(j),
                Err(e) => {
                    error!(collection = %collection, key = %key, error = ?e, "failed to decode value, skipping");
                    if let Err(e) = self.cursor_store.save(&change_id).await {
                        warn!(error = ?e, "failed to save cursor after decode error");
                    }
                    return;
                }
            },
            None => None,
        };

        // ── Extract causation depth from value metadata ──
        let depth = json_value
            .as_ref()
            .and_then(|v| v.get("_depth"))
            .and_then(|d| d.as_u64())
            .map(|d| d as u32)
            .unwrap_or(0);

        if depth >= self.config.max_causation_depth {
            warn!(
                collection = %collection,
                key = %key,
                depth = depth,
                max = self.config.max_causation_depth,
                "causation depth exceeded, stopping chain"
            );
            if let Err(e) = self.cursor_store.save(&change_id).await {
                warn!(error = ?e, "failed to save cursor after depth limit");
            }
            return;
        }

        let ctx = ChangeContext {
            collection: collection.clone(),
            key: key.clone(),
            value: json_value.clone(),
        };

        // ── Match rules ──
        let rules = self.rules.read().await;
        let mut matched = Vec::new();
        for rule in rules.iter() {
            if rule.matches(ctx.clone()).await {
                matched.push(rule.clone());
            }
        }
        drop(rules);

        debug!(collection = %collection, key = %key, matched = matched.len(), "rule matching complete");

        // ── Claim + execute each matched rule ──
        let mut any_retryable = false;

        for rule in matched {
            let rule_id = rule.id();
            let now = chrono::Utc::now().timestamp_millis();
            let execution = match self
                .execution_store
                .ensure_pending(&change_id, rule_id)
                .await
            {
                Ok(execution) => execution,
                Err(error) => {
                    error!(rule = rule_id, change = %change_id, error = ?error, "execution state error");
                    any_retryable = true;
                    continue;
                }
            };
            match execution {
                ReactionExecutionStatus::Succeeded { .. }
                | ReactionExecutionStatus::Failed { .. } => continue,
                ReactionExecutionStatus::RetryWaiting { retry_at, .. } if retry_at > now => {
                    any_retryable = true;
                    continue;
                }
                ReactionExecutionStatus::Pending
                | ReactionExecutionStatus::Running { .. }
                | ReactionExecutionStatus::RetryWaiting { .. } => {}
            }

            match self.claim_store.try_claim(&change_id, rule_id).await {
                Ok(ClaimResult::Claimed) => {}
                Ok(ClaimResult::AlreadyClaimed { owner }) => {
                    debug!(rule = rule_id, change = %change_id, owner = %owner, "reaction lease held");
                    any_retryable = true;
                    continue;
                }
                Err(error) => {
                    error!(rule = rule_id, change = %change_id, error = ?error, "reaction lease error");
                    any_retryable = true;
                    continue;
                }
            }

            let running = ReactionExecutionStatus::Running {
                owner: self.config.owner_id.clone(),
                started_at: now,
            };
            if let Err(error) = self.execution_store.set(&change_id, rule_id, running).await {
                error!(rule = rule_id, change = %change_id, error = ?error, "failed to persist running reaction");
                let _ = self.claim_store.release(&change_id, rule_id).await;
                any_retryable = true;
                continue;
            }

            let reaction_ctx = ReactionContext {
                collection: collection.clone(),
                key: key.clone(),
                value: json_value.clone(),
                change_id: change_id.clone(),
            };
            let outcome = rule.execute(reaction_ctx).await;
            let finished_at = chrono::Utc::now().timestamp_millis();
            let execution = match outcome {
                ReactionOutcome::Ok => ReactionExecutionStatus::Succeeded { finished_at },
                ReactionOutcome::Retryable(reason) => {
                    any_retryable = true;
                    ReactionExecutionStatus::RetryWaiting {
                        retry_at: finished_at + 300_000,
                        reason,
                    }
                }
                ReactionOutcome::Failed(reason) => ReactionExecutionStatus::Failed {
                    finished_at,
                    reason,
                },
            };

            if let Err(error) = self
                .execution_store
                .set(&change_id, rule_id, execution.clone())
                .await
            {
                error!(rule = rule_id, change = %change_id, error = ?error, "failed to persist reaction result");
                any_retryable = true;
            } else {
                self.publish_outcome(rule_id, &change_id, &collection, &key, &execution)
                    .await;
            }
            if let Err(error) = self.claim_store.release(&change_id, rule_id).await {
                warn!(rule = rule_id, change = %change_id, error = ?error, "failed to release reaction lease");
            }
        }

        // ── Advance cursor (ack point) ──
        // Cursor advances only when every matched reaction has a persisted
        // terminal execution state.
        if any_retryable {
            debug!(change = %change_id, "cursor held back due to retryable outcome");
            return;
        }

        if let Err(e) = self.cursor_store.save(&change_id).await {
            warn!(error = ?e, "failed to save cursor after processing");
        }
    }

    /// Publish a reaction outcome event to the EventBus bridge.
    /// This makes reactor activity visible via `/events/history` and TUI.
    /// Uses `wait=false` — the event is published best-effort.
    async fn publish_outcome(
        &self,
        rule_id: &str,
        change_id: &str,
        collection: &str,
        key: &str,
        execution: &ReactionExecutionStatus,
    ) {
        let Some(bus) = &self.event_bus else { return };
        bus.publish(
            Event::new(
                "REACTION_STATE_CHANGED",
                serde_json::json!({
                    "ruleId": rule_id,
                    "changeId": change_id,
                    "collection": collection,
                    "key": key,
                    "execution": execution,
                    "ownerId": self.config.owner_id,
                }),
            ),
            false,
        )
        .await;
    }
}
