use std::sync::atomic::Ordering;
use std::{sync::Arc, time::Duration};

use crate::store::MCPStore;

/// Periodic read-view reconcile: full KV scan -> rebuild -> atomic swap.
/// The event layer is allowed to be unreliable (shm KV has no watch); this
/// loop is the correctness floor (MinIO theorem). `load_from_db` parses
/// everything before touching registry/state/pool, so a failed reconcile
/// keeps the last-good view by construction.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
// ponytail: fixed interval, no config knob; promote to [daemon] config when a deployment needs tuning.

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
        let store = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(RECONCILE_INTERVAL);
            ticker.tick().await; // first tick fires immediately; startup hydration already ran
            loop {
                ticker.tick().await;
                if let Err(error) = store.load_from_db().await {
                    tracing::warn!(
                        "[STORE] registry reconcile failed, keeping last-good view: {error}"
                    );
                }
            }
        });
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
