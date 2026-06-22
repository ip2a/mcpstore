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
pub mod types;

use bus::{EventHandler, SubscriberMap};
pub use types::EventCapabilityReport;

/// Generic event wrapper.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub event_type: String,
    pub event_id: String,
    pub timestamp: i64,
    pub priority: i32,
    pub payload: serde_json::Value,
}

impl Event {
    pub fn new(event_type: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            event_type: event_type.into(),
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
            priority: 0,
            payload,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingHandler {
        counter: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl EventHandler for CountingHandler {
        async fn handle(&self, _event: &Event) {
            self.counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn test_publish_wait() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(CountingHandler {
            counter: counter.clone(),
        });

        bus.subscribe("TEST_EVENT", 0, handler).await;

        let event = Event::new("TEST_EVENT", serde_json::json!({}));
        bus.publish(event, true).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_publish_fire_and_forget() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(CountingHandler {
            counter: counter.clone(),
        });

        bus.subscribe("TEST_EVENT", 0, handler).await;

        let event = Event::new("TEST_EVENT", serde_json::json!({}));
        bus.publish(event, false).await;

        // Give spawned task a moment to run
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let bus = EventBus::new();
        let order = Arc::new(RwLock::new(Vec::new()));

        struct OrderHandler {
            name: String,
            order: Arc<RwLock<Vec<String>>>,
        }

        #[async_trait::async_trait]
        impl EventHandler for OrderHandler {
            async fn handle(&self, _event: &Event) {
                self.order.write().await.push(self.name.clone());
            }
        }

        bus.subscribe(
            "PRIO_EVENT",
            10,
            Arc::new(OrderHandler {
                name: "low".into(),
                order: order.clone(),
            }),
        )
        .await;

        bus.subscribe(
            "PRIO_EVENT",
            90,
            Arc::new(OrderHandler {
                name: "high".into(),
                order: order.clone(),
            }),
        )
        .await;

        let event = Event::new("PRIO_EVENT", serde_json::json!({}));
        bus.publish(event, true).await;

        let result = order.read().await;
        assert_eq!(result.as_slice(), &["high", "low"]);
    }
}
