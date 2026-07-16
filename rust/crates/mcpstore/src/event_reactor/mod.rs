//! EventReactor: subscribes to openkeyv ChangeFeed, matches rules, executes reactions.
//!
//! This is mcpstore's event-reaction mechanism. It replaces the old polling
//! scanner (`control/queue.rs`) with a push-based model:
//!
//! 1. openkeyv store mutation → atomically records a StoreChange
//! 2. ChangeFeed pushes the change to all subscribers
//! 3. EventReactor reads the change, matches registered Rules (when)
//! 4. For each matching rule, claims execution via CAS ((change_id, rule_id))
//! 5. The claiming instance executes the reaction (then)
//! 6. Success/failure state written back to openkeyv

mod claim;
#[cfg(test)]
mod tests;
mod cursor;
mod rule;

use std::sync::Arc;

use openkeyv::{
    AsyncChangeFeed, AsyncCompareAndSwap, AsyncKeyValue, ChangeFeedRequest, ChangeFilter,
    ChangeOperation, ChangeStart, ChangeSubscription,
};
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub use rule::{ChangeContext, ReactionContext, ReactionOutcome, Rule};

use claim::{ClaimResult, ClaimStore};
use cursor::{CursorStore};

/// Configuration for creating an EventReactor.
#[derive(Clone, Debug)]
pub struct ReactorConfig {
    /// Stable subscriber identity for cursor persistence.
    /// Must be unique per instance and stable across restarts.
    pub subscriber_id: String,
    /// Unique instance identity for claim ownership.
    /// Can be the same as subscriber_id.
    pub owner_id: String,
    /// Namespace prefix (same as the CacheLayerManager namespace).
    pub namespace: String,
    /// Collections to watch. If empty, watches ALL collections.
    pub watch_collections: Vec<String>,
    /// Maximum in-flight reactions (tokio channel capacity).
    pub max_in_flight: usize,
}

impl Default for ReactorConfig {
    fn default() -> Self {
        Self {
            subscriber_id: "reactor-default".into(),
            owner_id: "reactor-default".into(),
            namespace: "mcpstore".into(),
            watch_collections: Vec::new(),
            max_in_flight: 64,
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
    shutdown_tx: RwLock<Option<mpsc::Sender<()>>>,
}

/// Error returned by EventReactor operations.
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

impl<S> EventReactor<S>
where
    S: AsyncChangeFeed + AsyncCompareAndSwap + AsyncKeyValue + Clone + Send + Sync + 'static,
{
    /// Create a new EventReactor backed by the given openkeyv store.
    pub fn new(store: S, config: ReactorConfig) -> Self {
        let cursor_store = CursorStore::new(store.clone(), &config.namespace, &config.subscriber_id);
        let claim_store = ClaimStore::new(store.clone(), &config.namespace, &config.owner_id);
        Self {
            store,
            config,
            rules: RwLock::new(Vec::new()),
            cursor_store,
            claim_store,
            shutdown_tx: RwLock::new(None),
        }
    }

    /// Register a rule. Must be called before [`start`].
    pub async fn register(&self, rule: Rule) {
        let mut rules = self.rules.write().await;
        info!(rule_id = rule.id(), "registered reactor rule");
        rules.push(rule);
    }

    /// Start the reactor: load cursor, subscribe to ChangeFeed, begin dispatch loop.
    ///
    /// Returns immediately; the feed loop runs in a background tokio task.
    /// Call [`shutdown`] to stop.
    pub async fn start(self: &Arc<Self>) -> Result<(), ReactorError> {
        let mut shutdown = self.shutdown_tx.write().await;
        if shutdown.is_some() {
            return Err(ReactorError::AlreadyRunning);
        }
        let (tx, rx) = mpsc::channel::<()>(1);
        *shutdown = Some(tx);

        // Load persisted cursor
        let start = match self.cursor_store.load().await.map_err(|e| ReactorError::Cursor(e.to_string()))? {
            Some(cursor_str) => {
                info!(subscriber = %self.config.subscriber_id, cursor = %cursor_str, "resuming from saved cursor");
                ChangeStart::After(openkeyv::ChangeCursor::new(cursor_str))
            }
            None => {
                info!(subscriber = %self.config.subscriber_id, "no saved cursor, starting from beginning");
                ChangeStart::Beginning
            }
        };

        // Build filter
        let filter = ChangeFilter {
            collections: self.config.watch_collections.clone(),
            operations: Vec::new(),
        };

        // Subscribe
        let subscription = self
            .store
            .subscribe(ChangeFeedRequest { start, filter })
            .await
            .map_err(ReactorError::Store)?;

        info!("event reactor started, dispatching feed loop");

        let this = self.clone();
        tokio::spawn(async move {
            this.feed_loop(subscription, rx).await;
        });

        Ok(())
    }

    /// Shut down the reactor: signals the feed loop to stop.
    pub async fn shutdown(&self) {
        let mut shutdown = self.shutdown_tx.write().await;
        if let Some(tx) = shutdown.take() {
            let _ = tx.send(()).await;
        }
    }

    /// The core feed consumption loop.
    async fn feed_loop(self: Arc<Self>, mut subscription: ChangeSubscription, mut shutdown_rx: mpsc::Receiver<()>) {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("event reactor shutting down");
                    break;
                }
                recv = subscription.recv() => {
                    match recv {
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

    /// Process a single StoreChange: read value, match rules, dispatch reactions.
    async fn handle_change(&self, change: openkeyv::StoreChange) {
        let collection = change.collection.clone();
        let key = change.key.clone();
        let change_id = change.cursor.to_string();

        // Read current value (None for deletes — events are append-only so this is fine)
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

        // Convert to JSON for rule matching
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

        let ctx = ChangeContext {
            collection: collection.clone(),
            key: key.clone(),
            value: json_value.clone(),
        };

        // Match rules
        let rules = self.rules.read().await;
        let mut matched = Vec::new();
        for rule in rules.iter() {
            if rule.matches(ctx.clone()).await {
                matched.push(rule.clone());
            }
        }
        drop(rules);

        debug!(collection = %collection, key = %key, matched = matched.len(), "rule matching complete");

        // Dispatch: claim + execute each matched rule
        for rule in matched {
            let reaction_ctx = ReactionContext {
                collection: collection.clone(),
                key: key.clone(),
                value: json_value.clone(),
                change_id: change_id.clone(),
            };

            // Claim
            match self.claim_store.try_claim(&change_id, rule.id()).await {
                Ok(ClaimResult::Claimed) => {
                    debug!(rule = rule.id(), change = %change_id, "claim acquired");
                }
                Ok(ClaimResult::AlreadyClaimed { owner }) => {
                    debug!(rule = rule.id(), change = %change_id, owner = %owner, "already claimed, skipping");
                    continue;
                }
                Err(e) => {
                    error!(rule = rule.id(), change = %change_id, error = ?e, "claim error, skipping");
                    continue;
                }
            }

            // Execute
            let outcome = rule.execute(reaction_ctx).await;
            match &outcome {
                ReactionOutcome::Ok => {
                    debug!(rule = rule.id(), change = %change_id, "reaction succeeded");
                    if let Err(e) = self.claim_store.mark_succeeded(&change_id, rule.id()).await {
                        warn!(rule = rule.id(), error = ?e, "failed to mark claim succeeded");
                    }
                }
                ReactionOutcome::Retryable(reason) => {
                    warn!(rule = rule.id(), change = %change_id, reason = %reason, "reaction retryable");
                    if let Err(e) = self
                        .claim_store
                        .mark_failed(&change_id, rule.id(), Some(std::time::Duration::from_secs(300)))
                        .await
                    {
                        warn!(rule = rule.id(), error = ?e, "failed to mark claim failed");
                    }
                }
                ReactionOutcome::Failed(reason) => {
                    error!(rule = rule.id(), change = %change_id, reason = %reason, "reaction failed permanently");
                    if let Err(e) = self
                        .claim_store
                        .mark_failed(&change_id, rule.id(), Some(std::time::Duration::from_secs(3600)))
                        .await
                    {
                        warn!(rule = rule.id(), error = ?e, "failed to mark claim failed");
                    }
                }
            }
        }

        // Advance cursor
        if let Err(e) = self.cursor_store.save(&change_id).await {
            warn!(error = ?e, "failed to save cursor after processing");
        }
    }
}
