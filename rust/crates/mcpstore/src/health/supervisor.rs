use std::collections::HashMap;
use std::sync::{Arc, OnceLock, Weak};

use tokio::sync::Mutex;

use crate::identity::InstanceId;
use crate::state::{
    FailureInfo, FailurePhase, HealthState, RecoveryState, RuntimePhase, ServiceStateEvent,
    ServiceStateManager,
};
use crate::store::MCPStore;

use super::probes::{ProbeKind, ProbeRunner};
use super::state_machine::{
    HealthAssessment, HealthMonitor, HealthObservation, ObservationKind, SupervisorPolicy,
};

pub(crate) enum StartupOutcome {
    Healthy,
    TimedOut,
}

pub(crate) struct InstanceSupervisor {
    policy: SupervisorPolicy,
    monitors: Mutex<HashMap<InstanceId, HealthMonitor>>,
    workers: Mutex<HashMap<InstanceId, tokio::task::JoinHandle<()>>>,
    state_manager: Arc<ServiceStateManager>,
    store: OnceLock<Weak<MCPStore>>,
}

impl InstanceSupervisor {
    pub(crate) fn new(policy: SupervisorPolicy, state_manager: Arc<ServiceStateManager>) -> Self {
        Self {
            policy,
            monitors: Mutex::new(HashMap::new()),
            workers: Mutex::new(HashMap::new()),
            state_manager,
            store: OnceLock::new(),
        }
    }

    pub(crate) fn attach_store(&self, store: Weak<MCPStore>) {
        let _ = self.store.set(store);
    }

    pub(crate) async fn run_startup_probe(
        self: &Arc<Self>,
        runner: Arc<dyn ProbeRunner>,
        instance_id: InstanceId,
    ) -> StartupOutcome {
        self.register(instance_id).await;
        let started_at = now_f64();
        let interval =
            std::time::Duration::from_secs_f64(self.policy.startup_interval_secs.max(0.1));
        let timeout = std::time::Duration::from_secs_f64(self.policy.startup_timeout_secs.max(0.1));
        let hard_deadline = started_at + self.policy.startup_hard_timeout_secs.max(1.0);
        let half_open = self
            .state_manager
            .get(instance_id)
            .await
            .ok()
            .flatten()
            .is_some_and(|state| matches!(state.recovery, RecoveryState::Probing { .. }));
        if half_open {
            if let Some(monitor) = self.monitors.lock().await.get_mut(&instance_id) {
                monitor.begin_half_open();
            }
        }

        loop {
            if now_f64() >= hard_deadline {
                return StartupOutcome::TimedOut;
            }
            let started_at = std::time::Instant::now();
            let succeeded = runner
                .run_probe(instance_id, ProbeKind::Startup, timeout)
                .await
                .is_ok();
            let committed = self
                .observe_and_commit(
                    instance_id,
                    HealthObservation {
                        observed_at: now_f64(),
                        kind: ObservationKind::Startup,
                        succeeded,
                        latency_ms: Some(started_at.elapsed().as_secs_f64() * 1000.0),
                    },
                )
                .await
                .is_some();
            if !half_open && succeeded && committed {
                return StartupOutcome::Healthy;
            }
            if half_open {
                let mut monitors = self.monitors.lock().await;
                let Some(monitor) = monitors.get_mut(&instance_id) else {
                    return StartupOutcome::TimedOut;
                };
                let recovered = monitor.record_half_open_probe(succeeded && committed);
                let exhausted = monitor.half_open_calls_exhausted();
                drop(monitors);
                if recovered {
                    if self
                        .state_manager
                        .dispatch(
                            instance_id,
                            ServiceStateEvent::RecoveryProbeSucceeded,
                            now_f64() as i64,
                        )
                        .await
                        .is_ok()
                    {
                        return StartupOutcome::Healthy;
                    }
                    tracing::error!(%instance_id, "failed to commit successful recovery probe");
                    return StartupOutcome::TimedOut;
                }
                if exhausted {
                    return StartupOutcome::TimedOut;
                }
            }
            if now_f64() >= hard_deadline {
                return StartupOutcome::TimedOut;
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
                let Ok(Some(state)) = supervisor.state_manager.get(instance_id).await else {
                    continue;
                };
                match state.recovery {
                    RecoveryState::Waiting { retry_at, .. } => {
                        if retry_at <= now_f64() {
                            if let Some(store) = supervisor.store.get().and_then(Weak::upgrade) {
                                let _ = store.connect_service_internal(instance_id, true).await;
                            }
                        }
                        continue;
                    }
                    RecoveryState::Probing { .. } | RecoveryState::Exhausted { .. } => continue,
                    RecoveryState::Idle => {}
                }
                if state.phase != RuntimePhase::Running {
                    continue;
                }
                let started_at = std::time::Instant::now();
                let succeeded = runner
                    .run_probe(instance_id, ProbeKind::Liveness, timeout)
                    .await
                    .is_ok();
                supervisor
                    .observe_and_commit(
                        instance_id,
                        HealthObservation {
                            observed_at: now_f64(),
                            kind: ObservationKind::Liveness,
                            succeeded,
                            latency_ms: Some(started_at.elapsed().as_secs_f64() * 1000.0),
                        },
                    )
                    .await;
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
        workers.insert(
            instance_id,
            self.spawn_health_worker(runner, instance_id, interval, timeout),
        );
    }

    pub(crate) async fn register(&self, instance_id: InstanceId) {
        self.monitors
            .lock()
            .await
            .entry(instance_id)
            .or_insert_with(|| HealthMonitor::new(self.policy));
    }

    pub(crate) async fn reset(&self, instance_id: InstanceId) {
        self.monitors
            .lock()
            .await
            .insert(instance_id, HealthMonitor::new(self.policy));
    }

    pub(crate) async fn remove(&self, instance_id: InstanceId) {
        if let Some(worker) = self.workers.lock().await.remove(&instance_id) {
            worker.abort();
        }
        self.monitors.lock().await.remove(&instance_id);
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
    ) -> Option<HealthAssessment> {
        self.register(instance_id).await;
        let assessment = self
            .monitors
            .lock()
            .await
            .get_mut(&instance_id)
            .map(|monitor| monitor.observe(observation))?;
        let now = observation.observed_at as i64;

        let failure_reason = assessment.failure_reason.unwrap_or("health_check_failed");
        let event = ServiceStateEvent::HealthObserved {
            health: assessment.health,
            metrics: assessment.metrics,
            failure: (assessment.health == HealthState::Unhealthy).then(|| FailureInfo {
                phase: FailurePhase::Health,
                code: failure_reason.to_string(),
                retryable: true,
                message: failure_reason.replace('_', " "),
                since: now,
            }),
        };
        if let Err(error) = self.state_manager.dispatch(instance_id, event, now).await {
            tracing::error!(%instance_id, %error, "failed to persist health observation");
            return None;
        }

        if assessment.health == HealthState::Unhealthy {
            if let Some(store) = self.store.get().and_then(Weak::upgrade) {
                if !store.is_db_source()
                    && store.registry.find_instance(instance_id).await.is_some()
                {
                    if let Err(error) = store.pool.disconnect(instance_id).await {
                        tracing::warn!(%instance_id, %error, "failed to disconnect unhealthy service");
                    }
                    if let Err(error) = store
                        .mark_instance_retryable_failure(
                            instance_id,
                            format!("Health check failed: {failure_reason}"),
                        )
                        .await
                    {
                        tracing::error!(%instance_id, %error, "failed to schedule service recovery");
                        return None;
                    }
                }
            }
        }
        Some(assessment)
    }
}

fn now_f64() -> f64 {
    chrono::Utc::now().timestamp_millis() as f64 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{memory_cache_store, CacheLayerManager};
    use crate::events::EventBus;
    use crate::identity::{ScopeRef, ServiceInstanceKey};
    use crate::state::{AuthState, DesiredState, ServiceState};
    use async_trait::async_trait;

    fn policy() -> SupervisorPolicy {
        SupervisorPolicy {
            startup_interval_secs: 0.01,
            startup_timeout_secs: 0.05,
            startup_hard_timeout_secs: 0.2,
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

    async fn supervisor(name: &str) -> (Arc<InstanceSupervisor>, InstanceId) {
        let cache = Arc::new(CacheLayerManager::new(memory_cache_store(), name));
        let manager = Arc::new(ServiceStateManager::new(cache, EventBus::new()));
        let id = ServiceInstanceKey::new(name, ScopeRef::Store).instance_id();
        manager
            .create(ServiceState::new(
                id,
                name.to_string(),
                ScopeRef::Store,
                DesiredState::Running,
                AuthState::NotRequired,
                1,
            ))
            .await
            .unwrap();
        manager
            .dispatch(id, ServiceStateEvent::StartRequested, 2)
            .await
            .unwrap();
        manager
            .dispatch(id, ServiceStateEvent::TransportConnected, 3)
            .await
            .unwrap();
        (Arc::new(InstanceSupervisor::new(policy(), manager)), id)
    }

    struct SuccessfulProbe;

    #[async_trait]
    impl ProbeRunner for SuccessfulProbe {
        async fn run_probe(
            &self,
            _instance_id: InstanceId,
            _kind: ProbeKind,
            _timeout: std::time::Duration,
        ) -> crate::transport::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn startup_probe_records_healthy_state() {
        let (supervisor, id) = supervisor("startup-success").await;
        assert!(matches!(
            supervisor
                .run_startup_probe(Arc::new(SuccessfulProbe), id)
                .await,
            StartupOutcome::Healthy
        ));
        assert_eq!(
            supervisor
                .state_manager
                .get(id)
                .await
                .unwrap()
                .unwrap()
                .health,
            HealthState::Healthy
        );
    }
}
