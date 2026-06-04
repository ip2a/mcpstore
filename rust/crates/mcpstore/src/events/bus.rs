//! Event bus implementation with priority, error isolation, and optional history.

use crate::Event;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Handler trait for event subscribers.
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &Event);
}

/// Subscription with metadata.
#[derive(Clone)]
pub struct Subscription {
    pub priority: i32,
    pub handler: Arc<dyn EventHandler>,
}

/// Internal subscription storage.
pub struct SubscriberMap {
    pub(crate) inner: std::collections::HashMap<String, Vec<Subscription>>,
}

impl Default for SubscriberMap {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriberMap {
    pub fn new() -> Self {
        Self {
            inner: std::collections::HashMap::new(),
        }
    }

    pub fn subscribe(&mut self, event_type: String, priority: i32, handler: Arc<dyn EventHandler>) {
        let subs = self.inner.entry(event_type).or_default();
        subs.push(Subscription { priority, handler });
        subs.sort_by_key(|s| -s.priority);
    }

    pub fn unsubscribe(&mut self, event_type: &str) {
        self.inner.remove(event_type);
    }

    pub fn get(&self, event_type: &str) -> Option<&Vec<Subscription>> {
        self.inner.get(event_type)
    }
}

/// Optional capped event history.
pub struct EventHistory {
    pub(crate) events: Vec<Event>,
    pub(crate) limit: usize,
    pub(crate) lock: Mutex<()>,
}

impl EventHistory {
    pub fn new(limit: usize) -> Self {
        Self {
            events: Vec::with_capacity(limit),
            limit,
            lock: Mutex::new(()),
        }
    }

    pub async fn push(&mut self, event: Event) {
        let _guard = self.lock.lock().await;
        self.events.push(event);
        if self.events.len() > self.limit {
            self.events.remove(0);
        }
    }

    pub async fn get_recent(&self, count: usize) -> Vec<Event> {
        let _guard = self.lock.lock().await;
        self.events.iter().rev().take(count).cloned().collect()
    }
}
