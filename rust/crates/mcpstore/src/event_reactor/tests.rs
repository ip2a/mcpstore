//! Integration tests for EventReactor using Memory backend.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use openkeyv::store::memory::MemoryStore;
use openkeyv::AsyncKeyValue;
use tokio::sync::Notify;

use super::*;

/// A simple rule that counts how many times its reaction fires.
async fn run_reactor_basic() {
    let store = MemoryStore::new();

    // Write an event to the event collection *before* starting the reactor.
    // Since we start from Beginning, the reactor should see it.
    let collection = "mcpstore:event:test.event";
    let payload = serde_json::json!({"hello": "world"});
    let value = crate::cache::codec::json_to_value(payload.clone()).unwrap();
    store.put("evt-1", value, Some(collection), None).await.unwrap();

    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();

    let config = ReactorConfig {
        subscriber_id: "test-sub-1".into(),
        owner_id: "test-owner-1".into(),
        namespace: "mcpstore".into(),
        watch_collections: vec![collection.into()],
        max_in_flight: 8,
        max_causation_depth: 16,
    };

    let reactor = Arc::new(EventReactor::new(store.clone(), config));

    reactor.register(Rule::new(
        "test.rule.v1",
        // when: match any put on this collection
        |_ctx| Box::pin(async { true }),
        // then: increment counter and notify
        move |ctx| {
            let c = counter_clone.clone();
            let n = notify_clone.clone();
            Box::pin(async move {
                assert_eq!(ctx.collection, "mcpstore:event:test.event");
                assert_eq!(ctx.key, "evt-1");
                assert_eq!(ctx.value, Some(serde_json::json!({"hello": "world"})));
                c.fetch_add(1, Ordering::SeqCst);
                n.notify_one();
                ReactionOutcome::Ok
            })
        },
    )).await;

    reactor.start().await.unwrap();

    // Wait for the reaction to fire
    tokio::time::timeout(Duration::from_secs(5), notify.notified())
        .await
        .expect("reaction did not fire within 5s");

    assert_eq!(counter.load(Ordering::SeqCst), 1, "reaction should have fired exactly once");

    reactor.shutdown().await;
}

/// Verify cursor persistence: reactor resumes from saved cursor, not from beginning.
async fn run_reactor_cursor_resume() {
    let store = MemoryStore::new();
    let collection = "mcpstore:event:resume.test";

    // Write first event
    let v1 = crate::cache::codec::json_to_value(serde_json::json!({"n": 1})).unwrap();
    store.put("e1", v1, Some(collection), None).await.unwrap();

    let config = ReactorConfig {
        subscriber_id: "test-sub-resume".into(),
        owner_id: "test-owner-resume".into(),
        namespace: "mcpstore".into(),
        watch_collections: vec![collection.into()],
        max_in_flight: 8,
        max_causation_depth: 16,
    };

    let reactor = Arc::new(EventReactor::new(store.clone(), config));

    let first_count = Arc::new(AtomicU32::new(0));
    let first_notify = Arc::new(Notify::new());

    let fc = first_count.clone();
    let fn_ = first_notify.clone();
    reactor.register(Rule::new(
        "resume.rule.v1",
        |_ctx| Box::pin(async { true }),
        move |_ctx| {
            let fc = fc.clone();
            let fn_ = fn_.clone();
            Box::pin(async move {
                fc.fetch_add(1, Ordering::SeqCst);
                fn_.notify_one();
                ReactionOutcome::Ok
            })
        },
    )).await;

    reactor.start().await.unwrap();

    // Wait for first event to be processed
    tokio::time::timeout(Duration::from_secs(5), first_notify.notified())
        .await
        .expect("first reaction did not fire");
    assert_eq!(first_count.load(Ordering::SeqCst), 1);

    reactor.shutdown().await;

    // Write a second event *after* shutdown
    let v2 = crate::cache::codec::json_to_value(serde_json::json!({"n": 2})).unwrap();
    store.put("e2", v2, Some(collection), None).await.unwrap();

    // Restart reactor — should resume from cursor and only see e2
    let second_count = Arc::new(AtomicU32::new(0));
    let second_notify = Arc::new(Notify::new());
    let sc = second_count.clone();
    let sn = second_notify.clone();

    // Re-register the same rule (fresh closure)
    // We need a new reactor because the old one consumed the Arc
    let config2 = ReactorConfig {
        subscriber_id: "test-sub-resume".into(),
        owner_id: "test-owner-resume".into(),
        namespace: "mcpstore".into(),
        watch_collections: vec![collection.into()],
        max_in_flight: 8,
        max_causation_depth: 16,
    };
    let reactor2 = Arc::new(EventReactor::new(store.clone(), config2));
    reactor2.register(Rule::new(
        "resume.rule.v1",
        |_ctx| Box::pin(async { true }),
        move |_ctx| {
            let sc = sc.clone();
            let sn = sn.clone();
            Box::pin(async move {
                sc.fetch_add(1, Ordering::SeqCst);
                sn.notify_one();
                ReactionOutcome::Ok
            })
        },
    )).await;

    reactor2.start().await.unwrap();

    tokio::time::timeout(Duration::from_secs(5), second_notify.notified())
        .await
        .expect("second reaction did not fire");

    // Should have fired only once (for e2, not re-firing e1)
    assert_eq!(
        second_count.load(Ordering::SeqCst),
        1,
        "should only process e2 after resume, not re-process e1"
    );

    reactor2.shutdown().await;
}

/// Verify distributed claim: two reactors share the same store, only one fires.
async fn run_reactor_distributed_claim() {
    let store = MemoryStore::new();
    let collection = "mcpstore:event:claim.test";

    // Both reactors share the same store but have different owner IDs
    let config_a = ReactorConfig {
        subscriber_id: "claim-sub-a".into(),
        owner_id: "claim-owner-a".into(),
        namespace: "mcpstore".into(),
        watch_collections: vec![collection.into()],
        max_in_flight: 8,
        max_causation_depth: 16,
    };
    let config_b = ReactorConfig {
        subscriber_id: "claim-sub-b".into(),
        owner_id: "claim-owner-b".into(),
        namespace: "mcpstore".into(),
        watch_collections: vec![collection.into()],
        max_in_flight: 8,
        max_causation_depth: 16,
    };

    let reactor_a = Arc::new(EventReactor::new(store.clone(), config_a));
    let reactor_b = Arc::new(EventReactor::new(store.clone(), config_b));

    let total_executions = Arc::new(AtomicU32::new(0));
    let notify = Arc::new(Notify::new());

    for reactor in [&reactor_a, &reactor_b] {
        let tc = total_executions.clone();
        let n = notify.clone();
        reactor.register(Rule::new(
            "claim.rule.v1",
            |_ctx| Box::pin(async { true }),
            move |_ctx| {
                let tc = tc.clone();
                let n = n.clone();
                Box::pin(async move {
                    tc.fetch_add(1, Ordering::SeqCst);
                    n.notify_one();
                    ReactionOutcome::Ok
                })
            },
        )).await;
    }

    reactor_a.start().await.unwrap();
    reactor_b.start().await.unwrap();

    // Now write an event — both should see it, but only one should execute
    let v = crate::cache::codec::json_to_value(serde_json::json!({"claim": "test"})).unwrap();
    store.put("claim-1", v, Some(collection), None).await.unwrap();

    tokio::time::timeout(Duration::from_secs(5), notify.notified())
        .await
        .expect("no reaction fired");

    // Give a brief moment for the second reactor to (correctly) skip
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        total_executions.load(Ordering::SeqCst),
        1,
        "exactly one reactor should have executed, not both"
    );

    reactor_a.shutdown().await;
    reactor_b.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reactor_basic() {
        run_reactor_basic().await;
    }

    #[tokio::test]
    async fn reactor_cursor_resume() {
        run_reactor_cursor_resume().await;
    }

    #[tokio::test]
    async fn reactor_distributed_claim() {
        run_reactor_distributed_claim().await;
    }
}

/// Verify recursion guard: reactor internal collections (cursors, claims) do not trigger user rules.
/// This test registers a rule that would fire on ANY collection change, writes to an event
/// collection, and then verifies the cursor/claim writes (which happen during processing)
/// do NOT re-trigger the rule.
async fn run_reactor_recursion_guard() {
    let store = MemoryStore::new();
    let collection = "mcpstore:event:recursion.test";

    let config = ReactorConfig {
        subscriber_id: "recursion-sub".into(),
        owner_id: "recursion-owner".into(),
        namespace: "mcpstore".into(),
        watch_collections: vec![collection.into()],
        max_in_flight: 8,
        max_causation_depth: 16,
    };

    let reactor = Arc::new(EventReactor::new(store.clone(), config));

    let call_count = Arc::new(AtomicU32::new(0));
    let notify = Arc::new(Notify::new());

    let cc = call_count.clone();
    let nc = notify.clone();

    reactor
        .register(Rule::new(
            "recursion.rule.v1",
            // when: match anything
            |_ctx| Box::pin(async { true }),
            move |_ctx| {
                let cc = cc.clone();
                let nc = nc.clone();
                Box::pin(async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    nc.notify_one();
                    ReactionOutcome::Ok
                })
            },
        ))
        .await;

    reactor.start().await.unwrap();

    // Write one event
    let v = crate::cache::codec::json_to_value(serde_json::json!({"test": "recursion"})).unwrap();
    store.put("evt-1", v, Some(collection), None).await.unwrap();

    tokio::time::timeout(Duration::from_secs(5), notify.notified())
        .await
        .expect("reaction did not fire");

    // The reaction fires, which causes cursor + claim writes.
    // Those writes go to internal collections and should NOT re-trigger the rule.
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "rule should fire exactly once — internal collection writes must not re-trigger"
    );

    reactor.shutdown().await;
}

/// Verify causation depth limit: events with _depth >= max_causation_depth are skipped.
async fn run_reactor_depth_limit() {
    let store = MemoryStore::new();
    let collection = "mcpstore:event:depth.test";

    let config = ReactorConfig {
        subscriber_id: "depth-sub".into(),
        owner_id: "depth-owner".into(),
        namespace: "mcpstore".into(),
        watch_collections: vec![collection.into()],
        max_in_flight: 8,
        max_causation_depth: 3,
    };

    let reactor = Arc::new(EventReactor::new(store.clone(), config));

    let call_count = Arc::new(AtomicU32::new(0));
    let notify = Arc::new(Notify::new());

    let cc = call_count.clone();
    let nc = notify.clone();
    reactor
        .register(Rule::new(
            "depth.rule.v1",
            |_ctx| Box::pin(async { true }),
            move |_ctx| {
                let cc = cc.clone();
                let nc = nc.clone();
                Box::pin(async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    nc.notify_one();
                    ReactionOutcome::Ok
                })
            },
        ))
        .await;

    reactor.start().await.unwrap();

    // Write an event with depth = max (should be skipped)
    let v_deep = crate::cache::codec::json_to_value(serde_json::json!({"_depth": 3, "msg": "too deep"}))
        .unwrap();
    store.put("deep", v_deep, Some(collection), None).await.unwrap();

    // Should NOT fire
    let result = tokio::time::timeout(Duration::from_millis(500), notify.notified()).await;
    assert!(result.is_err(), "event at max depth should be skipped");

    // Write an event with depth below max (should fire)
    let v_ok = crate::cache::codec::json_to_value(serde_json::json!({"_depth": 2, "msg": "ok"}))
        .unwrap();
    store.put("ok", v_ok, Some(collection), None).await.unwrap();

    tokio::time::timeout(Duration::from_secs(5), notify.notified())
        .await
        .expect("event below max depth should fire");

    assert_eq!(call_count.load(Ordering::SeqCst), 1, "only the depth-2 event should have fired");

    reactor.shutdown().await;
}

#[cfg(test)]
mod m4_tests {
    use super::*;

    #[tokio::test]
    async fn reactor_recursion_guard() {
        run_reactor_recursion_guard().await;
    }

    #[tokio::test]
    async fn reactor_depth_limit() {
        run_reactor_depth_limit().await;
    }
}
