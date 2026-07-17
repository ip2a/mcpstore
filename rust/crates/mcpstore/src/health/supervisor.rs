use std::collections::HashMap;

use tokio::sync::Mutex;

use crate::cache::models::HealthStatus;
use crate::identity::InstanceId;

use super::state_machine::{
    HealthObservation, HealthStateMachine, StateTransition, SupervisorPolicy,
};
use super::stats::WindowStats;

pub(crate) struct InstanceSupervisor {
    policy: SupervisorPolicy,
    machines: Mutex<HashMap<InstanceId, HealthStateMachine>>,
}

impl InstanceSupervisor {
    pub(crate) fn new(policy: SupervisorPolicy) -> Self {
        Self {
            policy,
            machines: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn register(&self, instance_id: InstanceId, status: HealthStatus) {
        self.machines
            .lock()
            .await
            .entry(instance_id)
            .or_insert_with(|| HealthStateMachine::new(self.policy, status));
    }

    pub(crate) async fn remove(&self, instance_id: InstanceId) {
        self.machines.lock().await.remove(&instance_id);
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
        self.machines
            .lock()
            .await
            .get_mut(&instance_id)
            .and_then(|machine| machine.enter_half_open(now))
    }

    pub(crate) async fn status(&self, instance_id: InstanceId) -> Option<HealthStatus> {
        self.machines
            .lock()
            .await
            .get(&instance_id)
            .map(HealthStateMachine::status)
    }

    pub(crate) async fn stats(&self, instance_id: InstanceId, now: f64) -> Option<WindowStats> {
        self.machines
            .lock()
            .await
            .get_mut(&instance_id)
            .map(|machine| machine.stats(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::state_machine::ObservationKind;
    use crate::identity::{ScopeRef, ServiceInstanceKey};

    fn instance_id(name: &str) -> InstanceId {
        ServiceInstanceKey::new(name, ScopeRef::Store).instance_id()
    }

    fn policy() -> SupervisorPolicy {
        SupervisorPolicy {
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
        let supervisor = InstanceSupervisor::new(policy());
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
        let supervisor = InstanceSupervisor::new(policy());
        let instance_id = instance_id("service");
        supervisor
            .register(instance_id, HealthStatus::Healthy)
            .await;
        supervisor.remove(instance_id).await;
        assert_eq!(supervisor.status(instance_id).await, None);
    }
}
