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
    AsyncChangeFeed, AsyncCompareAndSwap, AsyncEnumerateKeys, AsyncKeyValue, ChangeFeedRequest,
    ChangeFilter, ChangeOperation, ChangeStart, ChangeSubscription,
};
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub use rule::{ChangeContext, ReactionContext, ReactionOutcome, Rule};

use crate::events::{Event, EventBus};

use claim::{ClaimResult, ClaimStore};
use cursor::CursorStore;
use execution::{ReactionExecutionError, ReactionExecutionStatus, ReactionExecutionStore};

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
    /// Interval for recovering persisted RetryWaiting/Running reactions.
    pub recovery_interval: std::time::Duration,
    /// Delay before resubscribing after the feed closes or errors.
    pub feed_retry_interval: std::time::Duration,
}

impl Default for ReactorConfig {
    fn default() -> Self {
        Self {
            subscriber_id: "reactor-default".into(),
            owner_id: "reactor-default".into(),
            namespace: "mcpstore".into(),
            watch_collections: Vec::new(),
            max_causation_depth: 16,
            recovery_interval: std::time::Duration::from_secs(60),
            feed_retry_interval: std::time::Duration::from_secs(1),
        }
    }
}

/// The EventReactor: owns the feed subscription loop and dispatches reactions.
pub struct EventReactor<S>
where
    S: AsyncChangeFeed
        + AsyncCompareAndSwap
        + AsyncEnumerateKeys
        + AsyncKeyValue
        + Clone
        + Send
        + Sync
        + 'static,
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
    recovery_shutdown_tx: RwLock<Option<mpsc::Sender<()>>>,
    feed_task: RwLock<Option<tokio::task::JoinHandle<()>>>,
    recovery_task: RwLock<Option<tokio::task::JoinHandle<()>>>,
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
    S: AsyncChangeFeed
        + AsyncCompareAndSwap
        + AsyncEnumerateKeys
        + AsyncKeyValue
        + Clone
        + Send
        + Sync
        + 'static,
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
            recovery_shutdown_tx: RwLock::new(None),
            feed_task: RwLock::new(None),
            recovery_task: RwLock::new(None),
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
            let mut rx = rx;
            this.feed_loop(subscription, &mut rx).await;
        });
        *self.feed_task.write().await = Some(handle);

        let (recovery_tx, recovery_rx) = mpsc::channel::<()>(1);
        let this = self.clone();
        let recovery_handle = tokio::spawn(async move {
            this.recovery_loop(recovery_rx).await;
        });
        *self.recovery_shutdown_tx.write().await = Some(recovery_tx);
        *self.recovery_task.write().await = Some(recovery_handle);

        Ok(())
    }

    pub async fn shutdown(&self) {
        let senders = [
            self.shutdown_tx.write().await.take(),
            self.recovery_shutdown_tx.write().await.take(),
        ];
        for tx in senders.into_iter().flatten() {
            let _ = tx.send(()).await;
        }
        let handles = [
            self.feed_task.write().await.take(),
            self.recovery_task.write().await.take(),
        ];
        for handle in handles.into_iter().flatten() {
            let _ = handle.await;
        }
    }

    async fn feed_loop(
        self: Arc<Self>,
        mut subscription: ChangeSubscription,
        shutdown_rx: &mut mpsc::Receiver<()>,
    ) {
        let filter = ChangeFilter {
            collections: self.config.watch_collections.clone(),
            operations: Vec::new(),
        };
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
                            if !self
                                .resubscribe(&mut subscription, filter.clone(), shutdown_rx)
                                .await
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "change feed error");
                            if !self
                                .resubscribe(&mut subscription, filter.clone(), shutdown_rx)
                                .await
                            {
                                break;
                            }
                        }
                        Ok(None) => {
                            info!("change feed closed");
                            if !self
                                .resubscribe(&mut subscription, filter.clone(), shutdown_rx)
                                .await
                            {
                                break;
                            }
                        }
                        Ok(Some(change)) => {
                            self.handle_change(change).await;
                        }
                    }
                }
            }
        }
    }

    async fn resubscribe(
        self: &Arc<Self>,
        subscription: &mut ChangeSubscription,
        filter: ChangeFilter,
        shutdown_rx: &mut mpsc::Receiver<()>,
    ) -> bool {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => return false,
                _ = tokio::time::sleep(self.config.feed_retry_interval) => {}
            }
            let cursor = self.cursor_store.load().await.ok().flatten();
            let start = cursor.map_or(ChangeStart::Beginning, |cursor| {
                ChangeStart::After(openkeyv::ChangeCursor::new(cursor))
            });
            match self
                .store
                .subscribe(ChangeFeedRequest {
                    start,
                    filter: filter.clone(),
                })
                .await
            {
                Ok(new_sub) => {
                    *subscription = new_sub;
                    return true;
                }
                Err(openkeyv::Error::ChangeCursorExpired { oldest, .. }) => {
                    let _ = self
                        .cursor_store
                        .save(&openkeyv::ChangeCursor::new(&oldest).to_string())
                        .await;
                }
                Err(error) => error!(%error, "failed to resubscribe change feed"),
            }
        }
    }

    async fn recovery_loop(self: Arc<Self>, mut shutdown_rx: mpsc::Receiver<()>) {
        loop {
            if let Err(error) = self.recover_reactions().await {
                error!(%error, "reaction recovery scan failed");
            }
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                _ = tokio::time::sleep(self.config.recovery_interval) => {}
            }
        }
    }

    async fn recover_reactions(&self) -> Result<(), ReactionExecutionError> {
        let records = self.execution_store.list().await?;
        for record in records {
            let (Some(collection), Some(key)) = (record.collection, record.key) else {
                continue;
            };
            let value = match self.store.get(&key, Some(&collection)).await {
                Ok(Some(value)) => Some(crate::cache::codec::value_to_json(value)?),
                Ok(None) => None,
                Err(error) => {
                    warn!(collection = %collection, key = %key, error = %error, "failed to read reaction source");
                    continue;
                }
            };
            let mut ctx = ChangeContext {
                collection: collection.clone(),
                key: key.clone(),
                value,
            };
            // A control request is moved to Executing before its side effect runs.
            // A crash leaves the source record non-terminal but no longer Queued,
            // so reset it once before rule matching. Reactions themselves must be
            // idempotent; this is the same guarantee as re-executing a retry.
            if let Some(value) = ctx.value.as_mut() {
                if value.get("status").and_then(serde_json::Value::as_str) == Some("executing") {
                    if let Some(id) = value.get("id").and_then(serde_json::Value::as_str) {
                        if key == id {
                            if let Some(object) = value.as_object_mut() {
                                object.insert(
                                    "status".into(),
                                    serde_json::Value::String("queued".into()),
                                );
                            }
                        }
                    }
                }
            }

            let rules = self.rules.read().await;
            let mut matched = Vec::new();
            for rule in rules.iter() {
                if rule.matches(ctx.clone()).await {
                    matched.push(rule.clone());
                }
            }
            drop(rules);
            if matched.is_empty() {
                let status = ReactionExecutionStatus::Failed {
                    finished_at: chrono::Utc::now().timestamp_millis(),
                    reason: "reaction source no longer matches its rule".to_string(),
                };
                self.execution_store
                    .set(
                        &record.change_id,
                        &record.rule_id,
                        &collection,
                        &key,
                        status,
                    )
                    .await?;
                continue;
            }
            for rule in matched {
                self.execute_rule(
                    rule,
                    &collection,
                    &key,
                    ctx.value.clone(),
                    &record.change_id,
                )
                .await;
            }
        }
        Ok(())
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

        let mut any_retryable = false;
        for rule in matched {
            if self
                .execute_rule(rule, &collection, &key, json_value.clone(), &change_id)
                .await
            {
                any_retryable = true;
            }
        }

        // Cursor tracks feed progress. Retries live in reaction execution state,
        // so a retry must not block later changes from being observed.
        if any_retryable {
            debug!(change = %change_id, "reaction left persisted retry state");
        }
        if let Err(e) = self.cursor_store.save(&change_id).await {
            warn!(error = ?e, "failed to save cursor after processing");
        }
    }

    async fn execute_rule(
        &self,
        rule: Rule,
        collection: &str,
        event_key: &str,
        json_value: Option<serde_json::Value>,
        change_id: &str,
    ) -> bool {
        let rule_id = rule.id().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let execution = match self
            .execution_store
            .ensure_pending(change_id, &rule_id, collection, event_key)
            .await
        {
            Ok(execution) => execution,
            Err(error) => {
                error!(rule = %rule_id, change = %change_id, error = ?error, "execution state error");
                return true;
            }
        };
        match execution {
            ReactionExecutionStatus::Succeeded { .. } | ReactionExecutionStatus::Failed { .. } => {
                return false
            }
            ReactionExecutionStatus::RetryWaiting { retry_at, .. } if retry_at > now => {
                return true
            }
            ReactionExecutionStatus::Pending
            | ReactionExecutionStatus::Running { .. }
            | ReactionExecutionStatus::RetryWaiting { .. } => {}
        }

        match self.claim_store.try_claim(change_id, &rule_id).await {
            Ok(ClaimResult::Claimed) => {}
            Ok(ClaimResult::AlreadyClaimed { owner }) => {
                debug!(rule = %rule_id, change = %change_id, owner = %owner, "reaction lease held");
                return true;
            }
            Err(error) => {
                error!(rule = %rule_id, change = %change_id, error = ?error, "reaction lease error");
                return true;
            }
        }

        let running = ReactionExecutionStatus::Running {
            owner: self.config.owner_id.clone(),
            started_at: now,
        };
        if let Err(error) = self
            .execution_store
            .set(change_id, &rule_id, collection, event_key, running)
            .await
        {
            error!(rule = %rule_id, change = %change_id, error = ?error, "failed to persist running reaction");
            let _ = self.claim_store.release(change_id, &rule_id).await;
            return true;
        }

        let reaction_ctx = ReactionContext {
            collection: collection.to_string(),
            key: event_key.to_string(),
            value: json_value,
            change_id: change_id.to_string(),
        };
        let outcome = rule.execute(reaction_ctx).await;
        let finished_at = chrono::Utc::now().timestamp_millis();
        let execution = match outcome {
            ReactionOutcome::Ok => ReactionExecutionStatus::Succeeded { finished_at },
            ReactionOutcome::Retryable(reason) => ReactionExecutionStatus::RetryWaiting {
                retry_at: finished_at + 300_000,
                reason,
            },
            ReactionOutcome::Failed(reason) => ReactionExecutionStatus::Failed {
                finished_at,
                reason,
            },
        };
        let retryable = matches!(execution, ReactionExecutionStatus::RetryWaiting { .. });

        if let Err(error) = self
            .execution_store
            .set(
                change_id,
                &rule_id,
                collection,
                event_key,
                execution.clone(),
            )
            .await
        {
            error!(rule = %rule_id, change = %change_id, error = ?error, "failed to persist reaction result");
            return true;
        }
        self.publish_outcome(&rule_id, change_id, collection, event_key, &execution)
            .await;
        if let Err(error) = self.claim_store.release(change_id, &rule_id).await {
            warn!(rule = %rule_id, change = %change_id, error = %error, "failed to release reaction lease");
        }
        retryable
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
