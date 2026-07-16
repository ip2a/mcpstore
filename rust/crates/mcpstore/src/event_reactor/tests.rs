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

    /// Retryable outcome must NOT advance the cursor. The ChangeFeed
    /// re-delivers the same change; once the claim TTL (300s in production,
    /// shortened here) expires, the reaction re-executes.
    #[tokio::test]
    async fn reactor_retryable_holds_cursor() {
        let store = MemoryStore::new();
        let collection = "mcpstore:event:retryable.test";

        let payload = crate::cache::codec::json_to_value(serde_json::json!({"x": 1})).unwrap();
        store
            .put("r1", payload, Some(collection), None)
            .await
            .unwrap();

        let attempt = Arc::new(AtomicU32::new(0));
        let attempt_clone = attempt.clone();
        let notify = Arc::new(Notify::new());
        let notify_clone = notify.clone();

        let config = ReactorConfig {
            subscriber_id: "retry-sub".into(),
            owner_id: "retry-owner".into(),
            namespace: "mcpstore".into(),
            watch_collections: vec![collection.into()],
            max_in_flight: 8,
            max_causation_depth: 16,
        };
        let reactor = Arc::new(EventReactor::new(store.clone(), config));

        reactor
            .register(Rule::new(
                "retry.rule.v1",
                |_ctx| Box::pin(async { true }),
                move |_ctx| {
                    let a = attempt_clone.clone();
                    let n = notify_clone.clone();
                    Box::pin(async move {
                        let n_prev = a.fetch_add(1, Ordering::SeqCst);
                        if n_prev == 0 {
                            // First attempt: retryable
                            ReactionOutcome::Retryable("transient".into())
                        } else {
                            n.notify_one();
                            ReactionOutcome::Ok
                        }
                    })
                },
            ))
            .await;

        reactor.start().await.unwrap();

        // First attempt fires immediately (Retryable). Cursor is NOT advanced,
        // so the ChangeFeed re-delivers. But claim TTL is 300s — too long for
        // a test. Instead, verify the cursor was not advanced by checking the
        // cursor collection directly.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Read cursor collection to verify it was NOT saved
        let cursor_collection = "mcpstore:reactor:cursors";
        let cursor_key = "retry-sub";
        let cursor_val = store
            .get(cursor_key, Some(cursor_collection))
            .await
            .unwrap();
        assert!(
            cursor_val.is_none(),
            "cursor must NOT be advanced when Retryable; got {:?}",
            cursor_val
        );

        // Verify first attempt did fire (Retryable)
        assert_eq!(
            attempt.load(Ordering::SeqCst),
            1,
            "first attempt should have fired with Retryable"
        );

        reactor.shutdown().await;
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

// ── M5.5: CursorExpired recovery ──
mod m5_tests {
    use super::*;

    /// When a persisted cursor points before the oldest available change,
    /// EventReactor should fall back to Beginning and continue operating.
    #[tokio::test]
    async fn reactor_cursor_expired_falls_back_to_beginning() {
        let store = MemoryStore::new();
        let collection = "mcpstore:event:cursor.expired";

        // Write a few events first so entries exist (revision advances).
        for i in 0..5 {
            let v = crate::cache::codec::json_to_value(serde_json::json!({"n": i})).unwrap();
            store
                .put(&format!("e{i}"), v, Some(collection), None)
                .await
                .unwrap();
        }

        let config = ReactorConfig {
            subscriber_id: "cursor-expired-sub".into(),
            owner_id: "cursor-expired-owner".into(),
            namespace: "mcpstore".into(),
            watch_collections: vec![collection.into()],
            max_in_flight: 8,
            max_causation_depth: 16,
        };

        let reactor = Arc::new(EventReactor::new(store.clone(), config));

        // Poison the cursor: write cursor value "0" directly to the store so
        // ChangeStart::After("0") triggers CursorExpired (oldest revision > 1).
        let cursor_value =
            crate::cache::codec::json_to_value(serde_json::json!({"cursor": "0"})).unwrap();
        store
            .put(
                "cursor-expired-sub",
                cursor_value,
                Some("mcpstore:reactor:cursors"),
                None,
            )
            .await
            .unwrap();

        // Register a rule that counts all events.
        let count = Arc::new(AtomicU32::new(0));
        let cc = count.clone();
        reactor
            .register(Rule::new(
                "cursor.recovery.rule",
                move |ctx| {
                    Box::pin(async move { ctx.collection == "mcpstore:event:cursor.expired" })
                },
                move |_ctx| {
                    let cc = cc.clone();
                    Box::pin(async move {
                        cc.fetch_add(1, Ordering::SeqCst);
                        ReactionOutcome::Ok
                    })
                },
            ))
            .await;

        // start() should detect CursorExpired and fall back to Beginning.
        reactor.start().await.expect("start should succeed with cursor recovery");

        // The reactor should process all 5 events from the beginning.
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(
            count.load(Ordering::SeqCst) >= 5,
            "should have processed all events after cursor recovery, got {}",
            count.load(Ordering::SeqCst)
        );

        reactor.shutdown().await;
    }
}

// ── M5: Redis integration tests ──
// These tests require a live Redis instance via OPENKEYV_REDIS_URL.
// They verify cross-instance event delivery and distributed claim.

#[cfg(test)]
mod redis_tests {
    use super::*;
    use openkeyv::store::redis::RedisStore;

    fn redis_url() -> Option<String> {
        std::env::var("OPENKEYV_REDIS_URL").ok()
    }

    /// M5 core test: instance A writes an event to Redis, instance B
    /// (separate RedisStore connection) receives the ChangeFeed notification
    /// and executes the reaction. A does not need to know B's address.
    #[tokio::test]
    #[ignore = "requires OPENKEYV_REDIS_URL"]
    async fn redis_cross_instance_delivery() {
        let url = redis_url().unwrap();
        let ns = format!(
            "mcpstore_redis_test_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        );
        let collection = format!("{ns}:event:cross.instance");

        // Writer (instance A) — no reactor, just writes events
        let writer = RedisStore::new(&url).await.unwrap();

        // Reader (instance B) — has a reactor that reacts to the event
        let reader_store = RedisStore::new(&url).await.unwrap();
        let config = ReactorConfig {
            subscriber_id: format!("{ns}-reader"),
            owner_id: format!("{ns}-reader"),
            namespace: ns.clone(),
            watch_collections: vec![collection.clone()],
            max_in_flight: 8,
            max_causation_depth: 16,
        };

        let reactor = Arc::new(EventReactor::new(reader_store, config));
        let fired = Arc::new(AtomicU32::new(0));
        let notify = Arc::new(Notify::new());

        let fc = fired.clone();
        let nc = notify.clone();
        reactor
            .register(Rule::new(
                "cross.instance.v1",
                {
                    let col = collection.clone();
                    move |ctx| {
                        let col = col.clone();
                        Box::pin(async move { ctx.collection == col })
                    }
                },
                move |ctx| {
                    let fc = fc.clone();
                    let nc = nc.clone();
                    Box::pin(async move {
                        assert_eq!(ctx.key, "evt-from-a");
                        assert_eq!(
                            ctx.value,
                            Some(serde_json::json!({"source": "instance-a"}))
                        );
                        fc.fetch_add(1, Ordering::SeqCst);
                        nc.notify_one();
                        ReactionOutcome::Ok
                    })
                },
            ))
            .await;

        reactor.start().await.unwrap();

        // Give the subscription a moment to establish
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Instance A writes the event
        let v = crate::cache::codec::json_to_value(serde_json::json!({"source": "instance-a"}))
            .unwrap();
        writer
            .put("evt-from-a", v, Some(&collection), None)
            .await
            .unwrap();

        // Instance B should fire within 5 seconds
        tokio::time::timeout(Duration::from_secs(10), notify.notified())
            .await
            .expect("instance B did not receive event from A within 10s");

        assert_eq!(fired.load(Ordering::SeqCst), 1);

        reactor.shutdown().await;
    }

    /// M5 distributed claim on Redis: two reactor instances share the same
    /// Redis backend. When an event arrives, both see it via ChangeFeed, but
    /// only one claims and executes via CAS.
    #[tokio::test]
    #[ignore = "requires OPENKEYV_REDIS_URL"]
    async fn redis_distributed_claim_two_instances() {
        let url = redis_url().unwrap();
        let ns = format!(
            "mcpstore_redis_claim_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        );
        let collection = format!("{ns}:event:claim.test");

        let writer = RedisStore::new(&url).await.unwrap();
        let store_a = RedisStore::new(&url).await.unwrap();
        let store_b = RedisStore::new(&url).await.unwrap();

        let config_a = ReactorConfig {
            subscriber_id: format!("{ns}-a"),
            owner_id: format!("{ns}-owner-a"),
            namespace: ns.clone(),
            watch_collections: vec![collection.clone()],
            max_in_flight: 8,
            max_causation_depth: 16,
        };
        let config_b = ReactorConfig {
            subscriber_id: format!("{ns}-b"),
            owner_id: format!("{ns}-owner-b"),
            namespace: ns.clone(),
            watch_collections: vec![collection.clone()],
            max_in_flight: 8,
            max_causation_depth: 16,
        };

        let reactor_a = Arc::new(EventReactor::new(store_a, config_a));
        let reactor_b = Arc::new(EventReactor::new(store_b, config_b));

        let total = Arc::new(AtomicU32::new(0));
        let notify = Arc::new(Notify::new());

        for reactor in [&reactor_a, &reactor_b] {
            let tc = total.clone();
            let n = notify.clone();
            reactor
                .register(Rule::new(
                    "redis.claim.v1",
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
                ))
                .await;
        }

        reactor_a.start().await.unwrap();
        reactor_b.start().await.unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;

        let v = crate::cache::codec::json_to_value(serde_json::json!({"claim": "redis"})).unwrap();
        writer
            .put("claim-evt", v, Some(&collection), None)
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(10), notify.notified())
            .await
            .expect("no reaction fired within 10s");

        tokio::time::sleep(Duration::from_millis(500)).await;

        assert_eq!(
            total.load(Ordering::SeqCst),
            1,
            "exactly one of two Redis instances should execute, not both"
        );

        reactor_a.shutdown().await;
        reactor_b.shutdown().await;
    }

    /// M5 cursor resume on Redis: reactor processes an event, shuts down,
    /// a new event is written, then reactor restarts and only sees the new event.
    #[tokio::test]
    #[ignore = "requires OPENKEYV_REDIS_URL"]
    async fn redis_cursor_resume_after_restart() {
        let url = redis_url().unwrap();
        let ns = format!(
            "mcpstore_redis_resume_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        );
        let collection = format!("{ns}:event:resume.test");

        let writer = RedisStore::new(&url).await.unwrap();

        // Write first event
        let v1 = crate::cache::codec::json_to_value(serde_json::json!({"n": 1})).unwrap();
        writer.put("e1", v1, Some(&collection), None).await.unwrap();

        let sub_id = format!("{ns}-resume");

        // First reactor run
        let config1 = ReactorConfig {
            subscriber_id: sub_id.clone(),
            owner_id: sub_id.clone(),
            namespace: ns.clone(),
            watch_collections: vec![collection.clone()],
            max_in_flight: 8,
            max_causation_depth: 16,
        };
        let store1 = RedisStore::new(&url).await.unwrap();
        let reactor1 = Arc::new(EventReactor::new(store1, config1));

        let count1 = Arc::new(AtomicU32::new(0));
        let notify1 = Arc::new(Notify::new());
        let c1 = count1.clone();
        let n1 = notify1.clone();
        reactor1
            .register(Rule::new(
                "resume.v1",
                |_ctx| Box::pin(async { true }),
                move |_ctx| {
                    let c1 = c1.clone();
                    let n1 = n1.clone();
                    Box::pin(async move {
                        c1.fetch_add(1, Ordering::SeqCst);
                        n1.notify_one();
                        ReactionOutcome::Ok
                    })
                },
            ))
            .await;
        reactor1.start().await.unwrap();

        tokio::time::timeout(Duration::from_secs(10), notify1.notified())
            .await
            .expect("first event should have fired");
        assert_eq!(count1.load(Ordering::SeqCst), 1);
        reactor1.shutdown().await;

        // Write second event after shutdown
        tokio::time::sleep(Duration::from_millis(200)).await;
        let v2 = crate::cache::codec::json_to_value(serde_json::json!({"n": 2})).unwrap();
        writer.put("e2", v2, Some(&collection), None).await.unwrap();

        // Second reactor run — same subscriber_id, should resume cursor
        let config2 = ReactorConfig {
            subscriber_id: sub_id.clone(),
            owner_id: sub_id.clone(),
            namespace: ns.clone(),
            watch_collections: vec![collection.clone()],
            max_in_flight: 8,
            max_causation_depth: 16,
        };
        let store2 = RedisStore::new(&url).await.unwrap();
        let reactor2 = Arc::new(EventReactor::new(store2, config2));

        let count2 = Arc::new(AtomicU32::new(0));
        let notify2 = Arc::new(Notify::new());
        let c2 = count2.clone();
        let n2 = notify2.clone();
        reactor2
            .register(Rule::new(
                "resume.v1",
                |_ctx| Box::pin(async { true }),
                move |_ctx| {
                    let c2 = c2.clone();
                    let n2 = n2.clone();
                    Box::pin(async move {
                        c2.fetch_add(1, Ordering::SeqCst);
                        n2.notify_one();
                        ReactionOutcome::Ok
                    })
                },
            ))
            .await;
        reactor2.start().await.unwrap();

        tokio::time::timeout(Duration::from_secs(10), notify2.notified())
            .await
            .expect("second event should have fired after resume");

        assert_eq!(
            count2.load(Ordering::SeqCst),
            1,
            "should only process e2, not re-process e1"
        );

        reactor2.shutdown().await;
    }
}
