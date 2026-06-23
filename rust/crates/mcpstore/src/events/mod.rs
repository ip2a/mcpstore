//! MCPStore Event Bus
//!
//! Async publish/subscribe event system migrated from Python core/events/.
//! Features:
//! - Priority subscriptions (higher priority executed first)
//! - Error isolation (one handler failure does not affect others)
//! - Optional per-handler timeout
//! - Critical event whitelist (forces synchronous dispatch)
//! - Optional capped event history
//!
//! P2 priority. Replaces Python asyncio queue with tokio channels/tasks.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

pub mod bus;
mod event;
mod store;
#[cfg(test)]
mod tests;
pub mod types;

use bus::{EventHandler, SubscriberMap};
pub use event::Event;
pub use types::EventCapabilityReport;

/// Core event bus using tokio async primitives.
pub struct EventBus {
    subscribers: Arc<RwLock<SubscriberMap>>,
    history: Option<Arc<RwLock<bus::EventHistory>>>,
    critical_events: HashSet<String>,
    handler_timeout: Option<Duration>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(SubscriberMap::new())),
            history: None,
            critical_events: HashSet::new(),
            handler_timeout: None,
        }
    }

    pub fn with_history(capacity: usize) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(SubscriberMap::new())),
            history: Some(Arc::new(RwLock::new(bus::EventHistory::new(capacity)))),
            critical_events: HashSet::new(),
            handler_timeout: None,
        }
    }

    /// Subscribe to an event type with a priority.
    pub async fn subscribe(
        &self,
        event_type: impl Into<String>,
        priority: i32,
        handler: Arc<dyn EventHandler>,
    ) {
        let mut subs = self.subscribers.write().await;
        subs.subscribe(event_type.into(), priority, handler);
    }

    /// Unsubscribe all handlers for an event type.
    pub async fn unsubscribe(&self, event_type: &str) {
        let mut subs = self.subscribers.write().await;
        subs.unsubscribe(event_type);
    }

    /// Publish an event to all matching subscribers.
    pub async fn publish(&self, event: Event, wait: bool) {
        let wait = wait || self.critical_events.contains(&event.event_type);

        // Record history
        if let Some(history) = &self.history {
            let mut hist = history.write().await;
            hist.push(event.clone()).await;
        }

        // Get subscribers
        let subs = {
            let subs = self.subscribers.read().await;
            subs.get(&event.event_type).cloned().unwrap_or_default()
        };

        if subs.is_empty() {
            tracing::debug!("[EVENT] No subscribers for {}", event.event_type);
            return;
        }

        if wait {
            // Sequential await, ordered by priority
            for sub in subs {
                let result = if let Some(timeout) = self.handler_timeout {
                    tokio::time::timeout(timeout, sub.handler.handle(&event)).await
                } else {
                    let _: () = sub.handler.handle(&event).await;
                    Ok(())
                };
                if let Err(e) = result {
                    tracing::error!("[EVENT] Handler failed for {}: {:?}", event.event_type, e);
                }
            }
        } else {
            // Fire-and-forget detached tasks
            for sub in subs {
                let event = event.clone();
                let handler = sub.handler.clone();
                let timeout = self.handler_timeout;
                tokio::spawn(async move {
                    let result = if let Some(t) = timeout {
                        tokio::time::timeout(t, handler.handle(&event)).await
                    } else {
                        let _: () = handler.handle(&event).await;
                        Ok(())
                    };
                    if let Err(e) = result {
                        tracing::error!("[EVENT] Handler failed for {}: {:?}", event.event_type, e);
                    }
                });
            }
        }
    }

    /// Get recent events from history.
    pub async fn get_history(&self, count: usize) -> Vec<Event> {
        match &self.history {
            Some(history) => {
                let hist = history.read().await;
                hist.get_recent(count).await
            }
            None => Vec::new(),
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
