use crate::cache::models::HealthStatus;

use super::stats::{HealthSample, HealthWindow, WindowStats};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObservationKind {
    Startup,
    Liveness,
    ToolCall,
    TransportFailure,
    ProcessExit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HealthObservation {
    pub(crate) observed_at: f64,
    pub(crate) kind: ObservationKind,
    pub(crate) succeeded: bool,
    pub(crate) latency_ms: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SupervisorPolicy {
    pub(crate) startup_interval_secs: f64,
    pub(crate) startup_timeout_secs: f64,
    pub(crate) startup_hard_timeout_secs: f64,
    pub(crate) window_secs: f64,
    pub(crate) window_max_samples: usize,
    pub(crate) window_min_calls: usize,
    pub(crate) error_rate_threshold: f64,
    pub(crate) latency_p95_warn_ms: f64,
    pub(crate) latency_p99_critical_ms: f64,
    pub(crate) liveness_failure_threshold: usize,
    pub(crate) half_open_max_calls: usize,
    pub(crate) half_open_success_rate_threshold: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StateTransition {
    pub(crate) from: HealthStatus,
    pub(crate) to: HealthStatus,
    pub(crate) reason: &'static str,
    pub(crate) stats: WindowStats,
}

pub(crate) struct HealthStateMachine {
    policy: SupervisorPolicy,
    status: HealthStatus,
    window: HealthWindow,
    consecutive_liveness_failures: usize,
    half_open_calls: usize,
    half_open_successes: usize,
}

impl HealthStateMachine {
    pub(crate) fn new(policy: SupervisorPolicy, status: HealthStatus) -> Self {
        assert!(policy.window_min_calls > 0);
        assert!(policy.liveness_failure_threshold > 0);
        assert!(policy.half_open_max_calls > 0);
        Self {
            window: HealthWindow::new(policy.window_secs, policy.window_max_samples),
            policy,
            status,
            consecutive_liveness_failures: 0,
            half_open_calls: 0,
            half_open_successes: 0,
        }
    }

    pub(crate) fn status_name(status: &HealthStatus) -> &'static str {
        match status {
            HealthStatus::Init => "init",
            HealthStatus::Startup => "startup",
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::CircuitOpen => "circuit_open",
            HealthStatus::HalfOpen => "half_open",
            HealthStatus::Disconnected => "disconnected",
        }
    }
    pub(crate) fn status(&self) -> HealthStatus {
        self.status.clone()
    }

    pub(crate) fn observe(&mut self, observation: HealthObservation) -> Option<StateTransition> {
        let stats = self.window.record(
            HealthSample::new(
                observation.observed_at,
                observation.succeeded,
                observation.latency_ms,
            ),
            observation.observed_at,
        );

        if observation.succeeded {
            self.consecutive_liveness_failures = 0;
        } else if observation.kind == ObservationKind::Liveness {
            self.consecutive_liveness_failures += 1;
        }

        let next = match self.status {
            HealthStatus::Init | HealthStatus::Startup if observation.succeeded => {
                Some((HealthStatus::Healthy, "startup_probe_succeeded"))
            }
            HealthStatus::Init | HealthStatus::Startup
                if observation.kind == ObservationKind::TransportFailure
                    || observation.kind == ObservationKind::ProcessExit =>
            {
                Some((HealthStatus::CircuitOpen, "startup_probe_failed"))
            }
            HealthStatus::HalfOpen => self.observe_half_open(observation, stats),
            HealthStatus::Healthy | HealthStatus::Degraded => {
                self.observe_open_state(observation, stats)
            }
            _ => None,
        };

        next.and_then(|(to, reason)| self.transition(to, reason, stats))
    }

    fn observe_open_state(
        &self,
        observation: HealthObservation,
        stats: WindowStats,
    ) -> Option<(HealthStatus, &'static str)> {
        if observation.kind == ObservationKind::ProcessExit
            || observation.kind == ObservationKind::TransportFailure
            || self.consecutive_liveness_failures >= self.policy.liveness_failure_threshold
        {
            return Some((HealthStatus::CircuitOpen, "liveness_failure_threshold"));
        }
        if stats.sample_size >= self.policy.window_min_calls
            && stats
                .error_rate
                .is_some_and(|rate| rate >= self.policy.error_rate_threshold)
        {
            return Some((HealthStatus::CircuitOpen, "error_rate_threshold"));
        }
        if stats
            .latency_p99
            .is_some_and(|latency| latency >= self.policy.latency_p99_critical_ms)
        {
            return Some((HealthStatus::CircuitOpen, "latency_p99_critical"));
        }
        if stats
            .latency_p95
            .is_some_and(|latency| latency >= self.policy.latency_p95_warn_ms)
        {
            Some((HealthStatus::Degraded, "latency_p95_warning"))
        } else if observation.succeeded {
            Some((HealthStatus::Healthy, "probe_succeeded"))
        } else {
            None
        }
    }

    fn observe_half_open(
        &mut self,
        observation: HealthObservation,
        stats: WindowStats,
    ) -> Option<(HealthStatus, &'static str)> {
        self.half_open_calls += 1;
        if observation.succeeded {
            self.half_open_successes += 1;
        }
        if !observation.succeeded
            || observation.kind == ObservationKind::TransportFailure
            || observation.kind == ObservationKind::ProcessExit
        {
            return Some((HealthStatus::CircuitOpen, "half_open_probe_failed"));
        }
        if self.half_open_calls >= self.policy.half_open_max_calls {
            let success_rate = self.half_open_successes as f64 / self.half_open_calls as f64;
            if success_rate >= self.policy.half_open_success_rate_threshold {
                return Some((HealthStatus::Healthy, "half_open_success_rate"));
            }
            return Some((HealthStatus::CircuitOpen, "half_open_success_rate_failed"));
        }
        let _ = stats;
        None
    }

    pub(crate) fn enter_half_open(&mut self, now: f64) -> Option<StateTransition> {
        let stats = self.window.stats(now);
        self.half_open_calls = 0;
        self.half_open_successes = 0;
        self.transition(HealthStatus::HalfOpen, "cooldown_elapsed", stats)
    }

    fn transition(
        &mut self,
        to: HealthStatus,
        reason: &'static str,
        stats: WindowStats,
    ) -> Option<StateTransition> {
        if self.status == to {
            return None;
        }
        let from = self.status.clone();
        self.status = to.clone();
        Some(StateTransition {
            from,
            to,
            reason,
            stats,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> SupervisorPolicy {
        SupervisorPolicy {
            startup_interval_secs: 1.0,
            startup_timeout_secs: 5.0,
            startup_hard_timeout_secs: 30.0,
            window_secs: 60.0,
            window_max_samples: 20,
            window_min_calls: 3,
            error_rate_threshold: 0.5,
            latency_p95_warn_ms: 200.0,
            latency_p99_critical_ms: 500.0,
            liveness_failure_threshold: 2,
            half_open_max_calls: 2,
            half_open_success_rate_threshold: 0.5,
        }
    }

    fn observation(at: f64, kind: ObservationKind, succeeded: bool) -> HealthObservation {
        HealthObservation {
            observed_at: at,
            kind,
            succeeded,
            latency_ms: Some(10.0),
        }
    }

    #[test]
    fn startup_success_enters_healthy() {
        let mut machine = HealthStateMachine::new(policy(), HealthStatus::Startup);
        let transition = machine.observe(observation(1.0, ObservationKind::Startup, true));
        assert_eq!(transition.unwrap().to, HealthStatus::Healthy);
    }

    #[test]
    fn consecutive_liveness_failures_open_circuit() {
        let mut machine = HealthStateMachine::new(policy(), HealthStatus::Healthy);
        assert!(machine
            .observe(observation(1.0, ObservationKind::Liveness, false))
            .is_none());
        let transition = machine.observe(observation(2.0, ObservationKind::Liveness, false));
        assert_eq!(transition.unwrap().to, HealthStatus::CircuitOpen);
    }

    #[test]
    fn error_rate_opens_circuit_after_minimum_samples() {
        let mut machine = HealthStateMachine::new(policy(), HealthStatus::Healthy);
        machine.observe(observation(1.0, ObservationKind::ToolCall, false));
        machine.observe(observation(2.0, ObservationKind::ToolCall, false));
        let transition = machine.observe(observation(3.0, ObservationKind::ToolCall, true));
        assert_eq!(transition.unwrap().to, HealthStatus::CircuitOpen);
    }

    #[test]
    fn warning_latency_enters_degraded() {
        let mut machine = HealthStateMachine::new(policy(), HealthStatus::Healthy);
        let mut sample = observation(1.0, ObservationKind::Liveness, true);
        sample.latency_ms = Some(250.0);
        assert_eq!(machine.observe(sample).unwrap().to, HealthStatus::Degraded);
    }

    #[test]
    fn half_open_requires_success_rate() {
        let mut machine = HealthStateMachine::new(policy(), HealthStatus::CircuitOpen);
        assert_eq!(
            machine.enter_half_open(1.0).unwrap().to,
            HealthStatus::HalfOpen
        );
        machine.observe(observation(2.0, ObservationKind::Liveness, true));
        let transition = machine.observe(observation(3.0, ObservationKind::Liveness, true));
        assert_eq!(transition.unwrap().to, HealthStatus::Healthy);
    }
}
