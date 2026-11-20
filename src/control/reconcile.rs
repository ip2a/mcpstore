use std::sync::atomic::{AtomicBool, Ordering};
use std::{sync::Arc, time::Duration};

use openkeyv::{AsyncChangeFeed, ChangeFeedRequest, ChangeFilter, ChangeOperation, ChangeStart};

use crate::error::{Error, FailureCode, Result};
use crate::kv::KvLayerManager;
use crate::store::MCPStore;

/// Registry read-view maintenance = event feed + health-gated periodic floor:
///
/// - A change feed over exactly the collections `load_from_db` reads triggers a
///   debounced full rebuild — convergence well under a second instead of 30s.
/// - The periodic loop stays as the correctness floor (MinIO theorem: the
///   event layer is allowed to be unreliable), but skips its scan while the
///   feed is provably alive. That skip is the watermark equivalent: "nothing
///   changed since the last scan" is carried by the live subscription itself
///   (every reconnect rescans first — the client-go resync-on-watch invariant),
///   not by a write-path counter writers could forget to bump.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
const EVENT_DEBOUNCE: Duration = Duration::from_millis(100);
// ponytail: fixed intervals, no config knob; promote to [daemon] config when a deployment needs tuning.

/// The registry read view is rebuilt from these four collections; a change to
/// anything else cannot change what `load_from_db` produces.
const RECONCILE_ENTITY_TYPES: &[&str] = &["service_definitions", "service_instances", "tools"];
const RECONCILE_RELATION_TYPES: &[&str] = &["instance_tools"];

impl MCPStore {
    pub(crate) fn spawn_registry_reconcile(self: &Arc<Self>) {
        if self
            .kernel
            .runtime
            .registry_reconcile_started
            .swap(true, Ordering::AcqRel)
        {
            return;
        }
        let feed_alive = Arc::new(AtomicBool::new(false));
        let reconcile_requested = Arc::new(tokio::sync::Notify::new());
        self.spawn_registry_change_feed(Arc::clone(&feed_alive), Arc::clone(&reconcile_requested));
        let store = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(RECONCILE_INTERVAL);
            ticker.tick().await; // first tick fires immediately; startup hydration already ran
            loop {
                let event_triggered = tokio::select! {
                    _ = ticker.tick() => false,
                    _ = reconcile_requested.notified() => true,
                };
                if !event_triggered && feed_alive.load(Ordering::Acquire) {
                    continue; // live feed: every change already reconciled, nothing to catch up on
                }
                if event_triggered {
                    // Debounce: merge a write burst into one rebuild.
                    tokio::time::sleep(EVENT_DEBOUNCE).await;
                }
                if let Err(error) = store.load_from_db().await {
                    tracing::warn!(
                        "[STORE] registry reconcile failed, keeping last-good view: {error}"
                    );
                }
            }
        });
    }

    fn spawn_registry_change_feed(
        self: &Arc<Self>,
        feed_alive: Arc<AtomicBool>,
        reconcile_requested: Arc<tokio::sync::Notify>,
    ) {
        let store = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                match store
                    .consume_registry_changes(&feed_alive, &reconcile_requested)
                    .await
                {
                    Ok(()) => {}
                    // openkeyv handle: "Store does not provide change feed";
                    // mcpstore's lazy-open error: "...does not provide ChangeFeed support".
                    // Match both spellings, case-insensitively.
                    Err(error)
                        if error.message().to_lowercase().contains("change feed")
                            || error.message().to_lowercase().contains("changefeed") =>
                    {
                        tracing::warn!("[STORE] registry change feed: {error}");
                        break;
                    }
                    Err(error) => {
                        tracing::warn!("[STORE] registry change feed resubscribe: {error}");
                    }
                }
                feed_alive.store(false, Ordering::Release);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            feed_alive.store(false, Ordering::Release);
        });
    }

    async fn consume_registry_changes(
        self: &Arc<Self>,
        feed_alive: &AtomicBool,
        reconcile_requested: &tokio::sync::Notify,
    ) -> Result<()> {
        let namespace = self.namespace();
        let mut collections: Vec<String> = RECONCILE_ENTITY_TYPES
            .iter()
            .map(|entity_type| {
                KvLayerManager::entity_collection_with_namespace(&namespace, entity_type)
            })
            .collect();
        collections.extend(RECONCILE_RELATION_TYPES.iter().map(|relation_type| {
            KvLayerManager::relation_collection_with_namespace(&namespace, relation_type)
        }));
        loop {
            let mut snapshot = self.event_log_snapshot().await?;
            let mut subscription = snapshot
                .log
                .subscribe(ChangeFeedRequest {
                    start: ChangeStart::Latest,
                    filter: ChangeFilter {
                        collections: collections.clone(),
                        operations: vec![ChangeOperation::Put, ChangeOperation::Delete],
                    },
                })
                .await
                .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
            // (Re)connected: reconcile now. Events between the previous
            // subscription's death and this reconnect are invisible to
            // ChangeStart::Latest, so the health-gated floor skip relies on
            // "alive ⇒ scanned since connect".
            feed_alive.store(true, Ordering::Release);
            reconcile_requested.notify_one();
            loop {
                tokio::select! {
                    change = subscription.recv() => match change {
                        Ok(Some(_)) => reconcile_requested.notify_one(),
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
                            "[STORE] registry change feed: store generation {}, resubscribing",
                            snapshot.generation
                        );
                        break;
                    }
                }
            }
            feed_alive.store(false, Ordering::Release);
        }
    }

    /// Weak-ref entry point (same pattern as spawn_service_event_feed_from_ref):
    /// no runtime / no self_weak -> no-op, so bare unit-test stores stay unaffected.
    pub(crate) fn spawn_registry_reconcile_from_ref(&self) {
        let Some(store) = self
            .kernel
            .runtime
            .self_weak
            .get()
            .and_then(|weak| weak.upgrade())
        else {
            return;
        };
        store.spawn_registry_reconcile();
    }
}
