use crate::state::{HealthMetrics, HealthState};

use super::stats::{HealthSample, HealthWindow, WindowStats};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObservationKind {
    Startup,
    Liveness,
    ToolCall,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HealthAssessment {
    pub(crate) health: HealthState,
    pub(crate) metrics: HealthMetrics,
    pub(crate) failure_reason: Option<&'static str>,
}

pub(crate) struct HealthMonitor {
    policy: SupervisorPolicy,
    window: HealthWindow,
    consecutive_liveness_failures: usize,
    half_open_calls: usize,
    half_open_successes: usize,
}

impl HealthMonitor {
    pub(crate) fn new(policy: SupervisorPolicy) -> Self {
        assert!(policy.window_min_calls > 0);
        assert!(policy.liveness_failure_threshold > 0);
        Self {
            window: HealthWindow::new(policy.window_secs, policy.window_max_samples),
            policy,
            consecutive_liveness_failures: 0,
            half_open_calls: 0,
            half_open_successes: 0,
        }
    }

    pub(crate) fn begin_half_open(&mut self) {
        self.half_open_calls = 0;
        self.half_open_successes = 0;
    }

    pub(crate) fn record_half_open_probe(&mut self, succeeded: bool) -> bool {
        self.half_open_calls = self.half_open_calls.saturating_add(1);
        if succeeded {
            self.half_open_successes = self.half_open_successes.saturating_add(1);
        }
        let calls = self.half_open_calls;
        let success_rate = self.half_open_successes as f64 / calls as f64;
        calls >= self.policy.half_open_max_calls
            && success_rate >= self.policy.half_open_success_rate_threshold
    }

    pub(crate) fn half_open_calls_exhausted(&self) -> bool {
        self.half_open_calls >= self.policy.half_open_max_calls
    }

    pub(crate) fn observe(&mut self, observation: HealthObservation) -> HealthAssessment {
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

        let (health, failure_reason) = self.classify(observation, stats);
        HealthAssessment {
            health,
            metrics: HealthMetrics {
                error_rate: stats.error_rate,
                latency_p95_ms: stats.latency_p95,
                latency_p99_ms: stats.latency_p99,
                sample_size: stats.sample_size.min(u32::MAX as usize) as u32,
            },
            failure_reason,
        }
    }

    fn classify(
        &self,
        observation: HealthObservation,
        stats: WindowStats,
    ) -> (HealthState, Option<&'static str>) {
        if matches!(observation.kind, ObservationKind::ProcessExit) {
            return (HealthState::Unhealthy, Some("transport_unavailable"));
        }
        if observation.kind != ObservationKind::Startup
            && self.consecutive_liveness_failures >= self.policy.liveness_failure_threshold
        {
            return (HealthState::Unhealthy, Some("liveness_failure_threshold"));
        }
        if observation.kind != ObservationKind::Startup
            && stats.sample_size >= self.policy.window_min_calls
        {
            if stats
                .error_rate
                .is_some_and(|rate| rate >= self.policy.error_rate_threshold)
            {
                return (HealthState::Unhealthy, Some("error_rate_threshold"));
            }
            if stats
                .latency_p99
                .is_some_and(|latency| latency >= self.policy.latency_p99_critical_ms)
            {
                return (HealthState::Unhealthy, Some("latency_p99_critical"));
            }
        }
        if stats
            .latency_p95
            .is_some_and(|latency| latency >= self.policy.latency_p95_warn_ms)
        {
            return (HealthState::Degraded, None);
        }
        if observation.succeeded {
            (HealthState::Healthy, None)
        } else {
            (HealthState::Degraded, None)
        }
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
    fn startup_success_is_healthy() {
        let mut monitor = HealthMonitor::new(policy());
        assert_eq!(
            monitor
                .observe(observation(1.0, ObservationKind::Startup, true))
                .health,
            HealthState::Healthy
        );
    }

    #[test]
    fn consecutive_liveness_failures_are_unhealthy() {
        let mut monitor = HealthMonitor::new(policy());
        assert_eq!(
            monitor
                .observe(observation(1.0, ObservationKind::Liveness, false))
                .health,
            HealthState::Degraded
        );
        let assessment = monitor.observe(observation(2.0, ObservationKind::Liveness, false));
        assert_eq!(assessment.health, HealthState::Unhealthy);
        assert_eq!(
            assessment.failure_reason,
            Some("liveness_failure_threshold")
        );
    }

    #[test]
    fn warning_latency_is_degraded() {
        let mut monitor = HealthMonitor::new(policy());
        let mut sample = observation(1.0, ObservationKind::Liveness, true);
        sample.latency_ms = Some(250.0);
        assert_eq!(monitor.observe(sample).health, HealthState::Degraded);
    }

    #[test]
    fn startup_failures_are_observed_without_opening_liveness_circuit() {
        let mut monitor = HealthMonitor::new(policy());
        for at in 1..=4 {
            let assessment =
                monitor.observe(observation(at as f64, ObservationKind::Startup, false));
            assert_eq!(assessment.health, HealthState::Degraded);
            assert_eq!(assessment.failure_reason, None);
        }
    }

    #[test]
    fn half_open_requires_configured_sample_count_and_success_rate() {
        let mut monitor = HealthMonitor::new(policy());
        monitor.begin_half_open();
        assert!(!monitor.record_half_open_probe(true));
        assert!(monitor.record_half_open_probe(false));
        assert!(monitor.half_open_calls_exhausted());

        monitor.begin_half_open();
        assert!(!monitor.record_half_open_probe(false));
        assert!(!monitor.record_half_open_probe(false));
        assert!(monitor.half_open_calls_exhausted());
    }

    #[test]
    fn process_exit_is_unhealthy_immediately() {
        let mut monitor = HealthMonitor::new(policy());
        let assessment = monitor.observe(observation(1.0, ObservationKind::ProcessExit, false));
        assert_eq!(assessment.health, HealthState::Unhealthy);
        assert_eq!(assessment.failure_reason, Some("transport_unavailable"));
    }
}
