use std::collections::HashMap;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::cache::models::{HealthStatus, InstanceStatus};
use crate::cache::CacheLayerManager;
use crate::events::{Event, EventBus};
use crate::identity::InstanceId;

use super::actions::SupervisorActions;
use super::probes::{ProbeKind, ProbeRunner};
use super::state_machine::{
    HealthObservation, HealthStateMachine, ObservationKind, StateTransition, SupervisorPolicy,
};

pub(crate) enum StartupOutcome {
    Healthy,
    Failed(StateTransition),
    TimedOut,
}

pub(crate) struct InstanceSupervisor {
    policy: SupervisorPolicy,
    machines: Mutex<HashMap<InstanceId, HealthStateMachine>>,
    workers: Mutex<HashMap<InstanceId, tokio::task::JoinHandle<()>>>,
    cache: Arc<CacheLayerManager>,
    event_bus: EventBus,
    actions: std::sync::OnceLock<Arc<dyn SupervisorActions>>,
}

impl InstanceSupervisor {
    pub(crate) fn new(
        policy: SupervisorPolicy,
        cache: Arc<CacheLayerManager>,
        event_bus: EventBus,
    ) -> Self {
        Self {
            policy,
            machines: Mutex::new(HashMap::new()),
            workers: Mutex::new(HashMap::new()),
            cache,
            event_bus,
            actions: std::sync::OnceLock::new(),
        }
    }

    pub(crate) fn attach_actions(
        &self,
        actions: Arc<dyn SupervisorActions>,
    ) {
        let _ = self.actions.set(actions);
    }

    pub(crate) async fn run_startup_probe(
        self: &Arc<Self>,
        runner: Arc<dyn ProbeRunner>,
        instance_id: InstanceId,
    ) -> StartupOutcome {
        let started_at = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
        let interval =
            std::time::Duration::from_secs_f64(self.policy.startup_interval_secs.max(0.1));
        let timeout =
            std::time::Duration::from_secs_f64(self.policy.startup_timeout_secs.max(0.1));
        let hard_deadline = started_at + self.policy.startup_hard_timeout_secs.max(1.0);

        loop {
            let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
            if now >= hard_deadline {
                return StartupOutcome::TimedOut;
            }

            let result = runner
                .run_probe(instance_id, ProbeKind::Startup, timeout)
                .await;
            let succeeded = result.is_ok();
            let observed_at = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
            let observation = HealthObservation {
                observed_at,
                kind: ObservationKind::Startup,
                succeeded,
                latency_ms: None,
            };
            let transition = self.observe(instance_id, observation).await;
            if let Some(transition) = transition.as_ref() {
                let _ = self.commit_transition(instance_id, transition).await;
                if transition.to == HealthStatus::Healthy {
                    return StartupOutcome::Healthy;
                }
                if matches!(
                    transition.to,
                    HealthStatus::CircuitOpen | HealthStatus::Disconnected
                ) {
                    return StartupOutcome::Failed(transition.clone());
                }
            }

            if succeeded {
                // State machine should have transitioned to Healthy; if not, treat as success.
                if self.status(instance_id).await != Some(HealthStatus::Healthy) {
                    let forced = self
                        .observe(
                            instance_id,
                            HealthObservation {
                                observed_at,
                                kind: ObservationKind::Startup,
                                succeeded: true,
                                latency_ms: None,
                            },
                        )
                        .await;
                    if let Some(transition) = forced.as_ref() {
                        let _ = self.commit_transition(instance_id, transition).await;
                        if transition.to == HealthStatus::Healthy {
                            return StartupOutcome::Healthy;
                        }
                    }
                }
                return StartupOutcome::Healthy;
            }

            tokio::time::sleep(interval).await;
        }
    }

    fn spawn_health_worker(
        self: &Arc<Self>,
        runner: Arc<dyn ProbeRunner>,
        instance_id: InstanceId,
        interval: std::time::Duration,
        timeout: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        let supervisor = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let status = supervisor.status(instance_id).await;
                let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;

                if status == Some(HealthStatus::CircuitOpen) {
                    let _ = supervisor.enter_half_open(instance_id, now).await;
                }

                let (probe_kind, observation_kind) = match supervisor.status(instance_id).await {
                    Some(HealthStatus::Startup) => {
                        (ProbeKind::Startup, ObservationKind::Startup)
                    }
                    Some(HealthStatus::HalfOpen)
                    | Some(HealthStatus::Healthy)
                    | Some(HealthStatus::Degraded) => {
                        (ProbeKind::Liveness, ObservationKind::Liveness)
                    }
                    _ => continue,
                };

                let result = runner.run_probe(instance_id, probe_kind, timeout).await;
                let observation = HealthObservation {
                    observed_at: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
                    kind: observation_kind,
                    succeeded: result.is_ok(),
                    latency_ms: None,
                };
                let transition = supervisor.observe(instance_id, observation).await;
                if let Some(transition) = transition {
                    let _ = supervisor.commit_transition(instance_id, &transition).await;
                }
            }
        })
    }

    pub(crate) async fn start_health_worker(
        self: &Arc<Self>,
        runner: Arc<dyn ProbeRunner>,
        instance_id: InstanceId,
        interval: std::time::Duration,
        timeout: std::time::Duration,
    ) {
        let mut workers = self.workers.lock().await;
        if workers.contains_key(&instance_id) {
            return;
        }
        let handle = self.spawn_health_worker(runner, instance_id, interval, timeout);
        workers.insert(instance_id, handle);
    }

    async fn commit_transition(
        &self,
        instance_id: InstanceId,
        transition: &StateTransition,
    ) -> crate::cache::Result<()> {
        let Some(value) = self
            .cache
            .get_state("instance_status", &instance_id.to_string())
            .await?
        else {
            return Ok(());
        };
        let mut status: InstanceStatus = serde_json::from_value(value)
            .map_err(|error| crate::cache::CacheError::Serialization(error))?;
        status.health_status = transition.to.clone();
        status.last_health_check = chrono::Utc::now().timestamp();
        status.window_error_rate = transition.stats.error_rate;
        status.latency_p95 = transition.stats.latency_p95;
        status.latency_p99 = transition.stats.latency_p99;
        status.sample_size = Some(transition.stats.sample_size.min(i32::MAX as usize) as i32);
        self.cache
            .put_state(
                "instance_status",
                &instance_id.to_string(),
                serde_json::to_value(&status).unwrap_or_default(),
            )
            .await?;
        self.event_bus
            .publish(
                Event::new(
                    "HEALTH_STATUS_CHANGED",
                    serde_json::json!({
                        "instance_id": instance_id,
                        "from": HealthStateMachine::status_name(&transition.from),
                        "to": HealthStateMachine::status_name(&transition.to),
                        "reason": transition.reason,
                        "stats": transition.stats,
                    }),
                ),
                true,
            )
            .await;
        if let Some(actions) = self.actions.get() {
            let _ = actions
                .apply_transition(
                    instance_id,
                    transition.from.clone(),
                    transition.to.clone(),
                    transition.reason,
                )
                .await;
        }
        Ok(())
    }
    pub(crate) async fn register(&self, instance_id: InstanceId, status: HealthStatus) {
        self.machines
            .lock()
            .await
            .entry(instance_id)
            .or_insert_with(|| HealthStateMachine::new(self.policy, status));
    }

    pub(crate) async fn remove(&self, instance_id: InstanceId) {
        if let Some(worker) = self.workers.lock().await.remove(&instance_id) {
            worker.abort();
        }
        self.machines.lock().await.remove(&instance_id);
    }

    pub(crate) async fn shutdown(&self) {
        let workers = std::mem::take(&mut *self.workers.lock().await);
        for (_, worker) in workers {
            worker.abort();
            let _ = worker.await;
        }
    }

    pub(crate) async fn observe_and_commit(
        &self,
        instance_id: InstanceId,
        observation: HealthObservation,
    ) -> Option<StateTransition> {
        let transition = self.observe(instance_id, observation).await;
        if let Some(transition) = transition.as_ref() {
            let _ = self.commit_transition(instance_id, transition).await;
        }
        transition
    }

    pub(crate) async fn reset_machine(
        &self,
        instance_id: InstanceId,
        status: HealthStatus,
    ) {
        self.machines
            .lock()
            .await
            .insert(instance_id, HealthStateMachine::new(self.policy, status));
    }

    pub(crate) async fn observe(
        &self,
        instance_id: InstanceId,
        observation: HealthObservation,
    ) -> Option<StateTransition> {
        self.machines
            .lock()
            .await
            .get_mut(&instance_id)
            .and_then(|machine| machine.observe(observation))
    }

    pub(crate) async fn enter_half_open(
        &self,
        instance_id: InstanceId,
        now: f64,
    ) -> Option<StateTransition> {
        let transition = self
            .machines
            .lock()
            .await
            .get_mut(&instance_id)
            .and_then(|machine| machine.enter_half_open(now));
        if let Some(transition) = transition.as_ref() {
            let _ = self.commit_transition(instance_id, transition).await;
        }
        transition
    }

    pub(crate) async fn status(&self, instance_id: InstanceId) -> Option<HealthStatus> {
        self.machines
            .lock()
            .await
            .get(&instance_id)
            .map(HealthStateMachine::status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::state_machine::ObservationKind;
    use crate::identity::{ScopeRef, ServiceInstanceKey};
    use async_trait::async_trait;

    fn instance_id(name: &str) -> InstanceId {
        ServiceInstanceKey::new(name, ScopeRef::Store).instance_id()
    }

    fn policy() -> SupervisorPolicy {
        SupervisorPolicy {
            startup_interval_secs: 0.05,
            startup_timeout_secs: 0.5,
            startup_hard_timeout_secs: 2.0,
            window_secs: 60.0,
            window_max_samples: 20,
            window_min_calls: 3,
            error_rate_threshold: 0.5,
            latency_p95_warn_ms: 200.0,
            latency_p99_critical_ms: 500.0,
            liveness_failure_threshold: 2,
            half_open_max_calls: 2,
            half_open_success_rate_threshold: 1.0,
        }
    }

    #[tokio::test]
    async fn keeps_state_isolated_per_instance() {
        let (store, _) = crate::cache::storage::memory_cache_store_with_handle();
        let cache = Arc::new(CacheLayerManager::new(store, "health-test"));
        let supervisor = InstanceSupervisor::new(policy(), cache, EventBus::new());
        let first = instance_id("first");
        let second = instance_id("second");
        supervisor.register(first, HealthStatus::Startup).await;
        supervisor.register(second, HealthStatus::Startup).await;

        supervisor
            .observe(
                first,
                HealthObservation {
                    observed_at: 1.0,
                    kind: ObservationKind::Startup,
                    succeeded: true,
                    latency_ms: Some(10.0),
                },
            )
            .await;

        assert_eq!(supervisor.status(first).await, Some(HealthStatus::Healthy));
        assert_eq!(supervisor.status(second).await, Some(HealthStatus::Startup));
    }

    #[tokio::test]
    async fn removes_instance_state() {
        let (store, _) = crate::cache::storage::memory_cache_store_with_handle();
        let cache = Arc::new(CacheLayerManager::new(store, "health-test"));
        let supervisor = InstanceSupervisor::new(policy(), cache, EventBus::new());
        let instance_id = instance_id("service");
        supervisor
            .register(instance_id, HealthStatus::Healthy)
            .await;
        supervisor.remove(instance_id).await;
        assert_eq!(supervisor.status(instance_id).await, None);
    }

    #[derive(Default, Clone)]
    struct ProbeRecorder {
        calls: Arc<std::sync::Mutex<Vec<ProbeKind>>>,
    }

    impl ProbeRecorder {
        fn record(&self, kind: ProbeKind) {
            self.calls.lock().unwrap().push(kind);
        }

        fn calls(&self) -> Vec<ProbeKind> {
            self.calls.lock().unwrap().clone()
        }
    }

    struct CountingProbeRunner {
        recorder: ProbeRecorder,
        fail_first: bool,
    }

    #[async_trait]
    impl ProbeRunner for CountingProbeRunner {
        async fn run_probe(
            &self,
            _instance_id: InstanceId,
            kind: ProbeKind,
            _timeout: std::time::Duration,
        ) -> crate::transport::Result<()> {
            self.recorder.record(kind);
            if self.fail_first && self.recorder.calls().len() == 1 {
                Err(crate::transport::TransportError::RequestTimedOut {
                    timeout: std::time::Duration::from_millis(1),
                })
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn startup_probe_retries_until_success() {
        let (store, _) = crate::cache::storage::memory_cache_store_with_handle();
        let cache = Arc::new(CacheLayerManager::new(store, "health-test"));
        let supervisor = Arc::new(InstanceSupervisor::new(policy(), cache, EventBus::new()));
        let id = instance_id("startup-success");
        supervisor.register(id, HealthStatus::Startup).await;

        let recorder = ProbeRecorder::default();
        let runner: Arc<dyn ProbeRunner> = Arc::new(CountingProbeRunner {
            recorder: recorder.clone(),
            fail_first: true,
        });
        let outcome = supervisor.run_startup_probe(runner, id).await;

        assert!(matches!(outcome, StartupOutcome::Healthy));
        assert_eq!(supervisor.status(id).await, Some(HealthStatus::Healthy));
        let calls = recorder.calls();
        assert!(calls.iter().all(|k| *k == ProbeKind::Startup));
        assert!(calls.len() >= 2);
    }

    struct FailingProbeRunner {
        recorder: ProbeRecorder,
    }

    #[async_trait]
    impl ProbeRunner for FailingProbeRunner {
        async fn run_probe(
            &self,
            _instance_id: InstanceId,
            kind: ProbeKind,
            _timeout: std::time::Duration,
        ) -> crate::transport::Result<()> {
            self.recorder.record(kind);
            Err(crate::transport::TransportError::RequestTimedOut {
                timeout: std::time::Duration::from_millis(1),
            })
        }
    }

    #[tokio::test]
    async fn startup_probe_times_out_when_probe_never_succeeds() {
        let (store, _) = crate::cache::storage::memory_cache_store_with_handle();
        let cache = Arc::new(CacheLayerManager::new(store, "health-test"));
        let supervisor = Arc::new(InstanceSupervisor::new(
            SupervisorPolicy {
                startup_interval_secs: 0.05,
                startup_timeout_secs: 0.05,
                startup_hard_timeout_secs: 0.15,
                ..policy()
            },
            cache,
            EventBus::new(),
        ));
        let id = instance_id("startup-timeout");
        supervisor.register(id, HealthStatus::Startup).await;

        let recorder = ProbeRecorder::default();
        let runner: Arc<dyn ProbeRunner> = Arc::new(FailingProbeRunner {
            recorder: recorder.clone(),
        });
        let outcome = supervisor.run_startup_probe(runner, id).await;

        assert!(matches!(outcome, StartupOutcome::TimedOut));
        let calls = recorder.calls();
        assert!(calls.iter().all(|k| *k == ProbeKind::Startup));
        assert!(!calls.is_empty());
    }

    #[tokio::test]
    async fn health_worker_switches_probe_kind_by_state() {
        let (store, _) = crate::cache::storage::memory_cache_store_with_handle();
        let cache = Arc::new(CacheLayerManager::new(store, "health-test"));
        let supervisor = Arc::new(InstanceSupervisor::new(
            SupervisorPolicy {
                startup_interval_secs: 0.05,
                startup_timeout_secs: 0.5,
                startup_hard_timeout_secs: 2.0,
                ..policy()
            },
            cache,
            EventBus::new(),
        ));
        let id = instance_id("worker-switch");
        supervisor.register(id, HealthStatus::Startup).await;

        let recorder = ProbeRecorder::default();
        let runner: Arc<dyn ProbeRunner> = Arc::new(CountingProbeRunner {
            recorder: recorder.clone(),
            fail_first: true,
        });
        supervisor
            .start_health_worker(
                runner,
                id,
                std::time::Duration::from_millis(50),
                std::time::Duration::from_millis(100),
            )
            .await;

        // Wait for startup probe to succeed and liveness probe to start.
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let calls = recorder.calls();
                if calls.iter().any(|k| *k == ProbeKind::Liveness) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();

        let calls = recorder.calls();
        assert!(calls.iter().any(|k| *k == ProbeKind::Startup));
        assert!(calls.iter().any(|k| *k == ProbeKind::Liveness));
    }
}
