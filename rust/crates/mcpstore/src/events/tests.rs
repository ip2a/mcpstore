use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use super::bus::EventHandler;
use super::{Event, EventBus};

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

    // Give spawned task a moment to run.
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
