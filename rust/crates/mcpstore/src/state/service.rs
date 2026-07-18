use serde::{Deserialize, Serialize};

use crate::identity::{InstanceId, ScopeRef};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DesiredState {
    Running,
    Stopped,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePhase {
    Stopped,
    Starting,
    Running,
    Stopping,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Unknown,
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RecoveryState {
    Idle,
    Waiting {
        attempt: u32,
        retry_at: f64,
        hard_deadline: f64,
    },
    Probing {
        attempt: u32,
        hard_deadline: f64,
    },
    Exhausted {
        attempts: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuthState {
    NotRequired,
    Unauthenticated,
    Authorizing,
    Authenticated,
    Refreshing,
    ScopeUpgradeRequired { required_scope: Option<String> },
    Failed,
}

impl AuthState {
    fn satisfied(&self) -> bool {
        matches!(self, Self::NotRequired | Self::Authenticated)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolsStatus {
    Unknown,
    Syncing,
    Ready,
    Stale,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolStateItem {
    pub name: String,
    pub availability: ToolAvailability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolsState {
    pub status: ToolsStatus,
    pub items: Vec<ToolStateItem>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct HealthMetrics {
    pub error_rate: Option<f64>,
    pub latency_p95_ms: Option<f64>,
    pub latency_p99_ms: Option<f64>,
    pub sample_size: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessStatus {
    Unknown,
    Ready,
    NotReady,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessReason {
    Unknown,
    Stopped,
    Starting,
    Recovering,
    Unhealthy,
    AuthRequired,
    ToolsNotReady,
    Ready,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Readiness {
    pub status: ReadinessStatus,
    pub reason: ReadinessReason,
    pub message: Option<String>,
    pub last_transition_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailurePhase {
    Start,
    Transport,
    Health,
    Recovery,
    Auth,
    Tools,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureInfo {
    pub phase: FailurePhase,
    pub code: String,
    pub retryable: bool,
    pub message: String,
    pub since: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceState {
    pub instance_id: InstanceId,
    pub service_name: String,
    pub scope: ScopeRef,
    pub desired: DesiredState,
    pub phase: RuntimePhase,
    pub health: HealthState,
    pub health_metrics: HealthMetrics,
    pub last_observed_at: Option<i64>,
    pub recovery: RecoveryState,
    pub auth: AuthState,
    pub tools: ToolsState,
    pub readiness: Readiness,
    pub failure: Option<FailureInfo>,
    pub version: u64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServiceStateEvent {
    StartRequested,
    StopRequested,
    StopFailed(FailureInfo),
    TransportConnected,
    TransportStopped,
    TransportFailed(FailureInfo),
    HealthObserved {
        health: HealthState,
        metrics: HealthMetrics,
        failure: Option<FailureInfo>,
    },
    RecoveryScheduled {
        attempt: u32,
        retry_at: f64,
        hard_deadline: f64,
    },
    RecoveryProbeStarted {
        attempt: u32,
    },
    RecoveryExhausted {
        attempts: u32,
        failure: FailureInfo,
    },
    AuthChanged(AuthState, Option<FailureInfo>),
    ToolSyncStarted,
    ToolSyncSucceeded {
        tools: Vec<String>,
    },
    ToolSyncFailed(FailureInfo),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServiceStateError {
    #[error("event {event} is invalid while desired={desired:?}, phase={phase:?}")]
    InvalidTransition {
        event: &'static str,
        desired: DesiredState,
        phase: RuntimePhase,
    },
}

impl ServiceState {
    pub fn new(
        instance_id: InstanceId,
        service_name: String,
        scope: ScopeRef,
        desired: DesiredState,
        auth: AuthState,
        now: i64,
    ) -> Self {
        let phase = RuntimePhase::Stopped;
        let reason = if desired == DesiredState::Stopped {
            ReadinessReason::Stopped
        } else {
            ReadinessReason::Starting
        };
        Self {
            instance_id,
            service_name,
            scope,
            desired,
            phase,
            health: HealthState::Unknown,
            health_metrics: HealthMetrics::default(),
            last_observed_at: None,
            recovery: RecoveryState::Idle,
            auth,
            tools: ToolsState {
                status: ToolsStatus::Unknown,
                items: Vec::new(),
            },
            readiness: Readiness {
                status: ReadinessStatus::NotReady,
                reason,
                message: None,
                last_transition_at: now,
            },
            failure: None,
            version: 0,
            updated_at: now,
        }
    }

    pub fn apply(&mut self, event: ServiceStateEvent, now: i64) -> Result<(), ServiceStateError> {
        match event {
            ServiceStateEvent::StartRequested => {
                self.desired = DesiredState::Running;
                self.phase = RuntimePhase::Starting;
                self.health = HealthState::Unknown;
                self.health_metrics = HealthMetrics::default();
                self.last_observed_at = None;
                self.recovery = RecoveryState::Idle;
                self.failure = None;
            }
            ServiceStateEvent::StopRequested => {
                self.desired = DesiredState::Stopped;
                self.phase = if self.phase == RuntimePhase::Stopped {
                    RuntimePhase::Stopped
                } else {
                    RuntimePhase::Stopping
                };
                self.recovery = RecoveryState::Idle;
                self.failure = None;
            }
            ServiceStateEvent::StopFailed(failure) => {
                if self.desired != DesiredState::Stopped || self.phase != RuntimePhase::Stopping {
                    return Err(self.invalid("stop_failed"));
                }
                self.phase = RuntimePhase::Running;
                self.failure = Some(failure);
            }
            ServiceStateEvent::TransportConnected => {
                self.require_running("transport_connected")?;
                self.phase = RuntimePhase::Running;
                self.health = HealthState::Unknown;
                self.health_metrics = HealthMetrics::default();
                self.last_observed_at = None;
                self.recovery = RecoveryState::Idle;
                self.failure = None;
            }
            ServiceStateEvent::TransportStopped => {
                self.phase = RuntimePhase::Stopped;
                self.health = if self.desired == DesiredState::Running {
                    HealthState::Unhealthy
                } else {
                    HealthState::Unknown
                };
                self.tools.status = ToolsStatus::Stale;
                if self.desired == DesiredState::Stopped {
                    self.recovery = RecoveryState::Idle;
                }
            }
            ServiceStateEvent::TransportFailed(failure) => {
                self.require_running("transport_failed")?;
                self.phase = RuntimePhase::Stopped;
                self.health = HealthState::Unhealthy;
                self.tools.status = ToolsStatus::Stale;
                self.failure = Some(failure);
            }
            ServiceStateEvent::HealthObserved {
                health,
                metrics,
                failure,
            } => {
                if self.phase != RuntimePhase::Running {
                    return Err(self.invalid("health_observed"));
                }
                self.health = health;
                self.health_metrics = metrics;
                self.last_observed_at = Some(now);
                self.failure = failure;
            }
            ServiceStateEvent::RecoveryScheduled {
                attempt,
                retry_at,
                hard_deadline,
            } => {
                self.require_running("recovery_scheduled")?;
                self.recovery = RecoveryState::Waiting {
                    attempt,
                    retry_at,
                    hard_deadline,
                };
            }
            ServiceStateEvent::RecoveryProbeStarted { attempt } => {
                self.require_running("recovery_probe_started")?;
                if !matches!(self.recovery, RecoveryState::Waiting { .. }) {
                    return Err(self.invalid("recovery_probe_started"));
                }
                let hard_deadline = match self.recovery {
                    RecoveryState::Waiting { hard_deadline, .. } => hard_deadline,
                    _ => return Err(self.invalid("recovery_probe_started")),
                };
                self.phase = RuntimePhase::Starting;
                self.recovery = RecoveryState::Probing {
                    attempt,
                    hard_deadline,
                };
            }
            ServiceStateEvent::RecoveryExhausted { attempts, failure } => {
                self.require_running("recovery_exhausted")?;
                self.phase = RuntimePhase::Stopped;
                self.health = HealthState::Unhealthy;
                self.recovery = RecoveryState::Exhausted { attempts };
                self.failure = Some(failure);
            }
            ServiceStateEvent::AuthChanged(auth, failure) => {
                self.auth = auth;
                self.failure = failure;
            }
            ServiceStateEvent::ToolSyncStarted => self.tools.status = ToolsStatus::Syncing,
            ServiceStateEvent::ToolSyncSucceeded { tools } => {
                self.tools.status = ToolsStatus::Ready;
                self.tools.items = tools
                    .into_iter()
                    .map(|name| ToolStateItem {
                        name,
                        availability: ToolAvailability::Available,
                    })
                    .collect();
            }
            ServiceStateEvent::ToolSyncFailed(failure) => {
                self.tools.status = ToolsStatus::Failed;
                self.failure = Some(failure);
            }
        }
        self.version = self.version.saturating_add(1);
        self.updated_at = now;
        self.refresh_readiness(now);
        Ok(())
    }

    fn require_running(&self, event: &'static str) -> Result<(), ServiceStateError> {
        if self.desired == DesiredState::Running {
            Ok(())
        } else {
            Err(self.invalid(event))
        }
    }

    fn invalid(&self, event: &'static str) -> ServiceStateError {
        ServiceStateError::InvalidTransition {
            event,
            desired: self.desired,
            phase: self.phase,
        }
    }

    fn refresh_readiness(&mut self, now: i64) {
        let (status, reason) = if self.desired == DesiredState::Stopped {
            (ReadinessStatus::NotReady, ReadinessReason::Stopped)
        } else if self.phase != RuntimePhase::Running {
            (ReadinessStatus::NotReady, ReadinessReason::Starting)
        } else if !matches!(self.recovery, RecoveryState::Idle) {
            (ReadinessStatus::NotReady, ReadinessReason::Recovering)
        } else if self.health == HealthState::Unhealthy {
            (ReadinessStatus::NotReady, ReadinessReason::Unhealthy)
        } else if !self.auth.satisfied() {
            (ReadinessStatus::NotReady, ReadinessReason::AuthRequired)
        } else if self.tools.status != ToolsStatus::Ready {
            (ReadinessStatus::NotReady, ReadinessReason::ToolsNotReady)
        } else if matches!(self.health, HealthState::Healthy | HealthState::Degraded) {
            (ReadinessStatus::Ready, ReadinessReason::Ready)
        } else {
            (ReadinessStatus::Unknown, ReadinessReason::Unknown)
        };
        if self.readiness.status != status || self.readiness.reason != reason {
            self.readiness.last_transition_at = now;
        }
        self.readiness.status = status;
        self.readiness.reason = reason;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(desired: DesiredState) -> ServiceState {
        ServiceState::new(
            InstanceId::from_key(&crate::identity::ServiceInstanceKey::new(
                "test",
                ScopeRef::Store,
            )),
            "test".to_string(),
            ScopeRef::Store,
            desired,
            AuthState::NotRequired,
            1,
        )
    }

    fn failure(phase: FailurePhase, retryable: bool) -> FailureInfo {
        FailureInfo {
            phase,
            code: "test".to_string(),
            retryable,
            message: "failure".to_string(),
            since: 2,
        }
    }

    #[test]
    fn service_becomes_ready_only_after_transport_health_and_tools() {
        let mut state = state(DesiredState::Running);
        state.apply(ServiceStateEvent::StartRequested, 2).unwrap();
        state
            .apply(ServiceStateEvent::TransportConnected, 3)
            .unwrap();
        state
            .apply(ServiceStateEvent::ToolSyncSucceeded { tools: vec![] }, 4)
            .unwrap();
        state
            .apply(
                ServiceStateEvent::HealthObserved {
                    health: HealthState::Healthy,
                    metrics: HealthMetrics::default(),
                    failure: None,
                },
                5,
            )
            .unwrap();
        assert_eq!(state.readiness.status, ReadinessStatus::Ready);
        assert_eq!(state.version, 4);
    }

    #[test]
    fn degraded_service_remains_ready() {
        let mut state = state(DesiredState::Running);
        state.apply(ServiceStateEvent::StartRequested, 2).unwrap();
        state
            .apply(ServiceStateEvent::TransportConnected, 3)
            .unwrap();
        state
            .apply(ServiceStateEvent::ToolSyncSucceeded { tools: vec![] }, 4)
            .unwrap();
        state
            .apply(
                ServiceStateEvent::HealthObserved {
                    health: HealthState::Degraded,
                    metrics: HealthMetrics::default(),
                    failure: None,
                },
                5,
            )
            .unwrap();
        assert_eq!(state.readiness.status, ReadinessStatus::Ready);
    }

    #[test]
    fn stop_clears_recovery_and_readiness() {
        let mut state = state(DesiredState::Running);
        state.apply(ServiceStateEvent::StartRequested, 2).unwrap();
        state
            .apply(
                ServiceStateEvent::RecoveryScheduled {
                    attempt: 1,
                    retry_at: 10.0,
                    hard_deadline: 30.0,
                },
                3,
            )
            .unwrap();
        state.apply(ServiceStateEvent::StopRequested, 4).unwrap();
        assert_eq!(state.desired, DesiredState::Stopped);
        assert_eq!(state.recovery, RecoveryState::Idle);
        assert_eq!(state.readiness.reason, ReadinessReason::Stopped);
    }

    #[test]
    fn failed_stop_preserves_desired_stop_and_running_transport() {
        let mut state = state(DesiredState::Running);
        state.apply(ServiceStateEvent::StartRequested, 2).unwrap();
        state
            .apply(ServiceStateEvent::TransportConnected, 3)
            .unwrap();
        state.apply(ServiceStateEvent::StopRequested, 4).unwrap();
        state
            .apply(
                ServiceStateEvent::StopFailed(failure(FailurePhase::Transport, false)),
                5,
            )
            .unwrap();
        assert_eq!(state.desired, DesiredState::Stopped);
        assert_eq!(state.phase, RuntimePhase::Running);
        assert_eq!(state.readiness.status, ReadinessStatus::NotReady);
        assert!(state.failure.is_some());
    }

    #[test]
    fn unexpected_transport_loss_is_not_a_desired_stop() {
        let mut state = state(DesiredState::Running);
        state.apply(ServiceStateEvent::StartRequested, 2).unwrap();
        state
            .apply(ServiceStateEvent::TransportConnected, 3)
            .unwrap();
        state
            .apply(
                ServiceStateEvent::TransportFailed(failure(FailurePhase::Transport, true)),
                4,
            )
            .unwrap();
        assert_eq!(state.desired, DesiredState::Running);
        assert_eq!(state.phase, RuntimePhase::Stopped);
        assert_eq!(state.health, HealthState::Unhealthy);
    }

    #[test]
    fn recovery_must_wait_before_probing() {
        let mut state = state(DesiredState::Running);
        state.apply(ServiceStateEvent::StartRequested, 2).unwrap();
        let error = state
            .apply(ServiceStateEvent::RecoveryProbeStarted { attempt: 1 }, 3)
            .unwrap_err();
        assert!(matches!(error, ServiceStateError::InvalidTransition { .. }));
    }

    #[test]
    fn stopped_service_rejects_automatic_recovery() {
        let mut state = state(DesiredState::Stopped);
        let error = state
            .apply(
                ServiceStateEvent::RecoveryScheduled {
                    attempt: 1,
                    retry_at: 10.0,
                    hard_deadline: 30.0,
                },
                2,
            )
            .unwrap_err();
        assert!(matches!(error, ServiceStateError::InvalidTransition { .. }));
    }

    #[test]
    fn auth_and_tools_are_explicit_readiness_gates() {
        let mut state = ServiceState::new(
            InstanceId::from_key(&crate::identity::ServiceInstanceKey::new(
                "test",
                ScopeRef::Store,
            )),
            "test".to_string(),
            ScopeRef::Store,
            DesiredState::Running,
            AuthState::Unauthenticated,
            1,
        );
        state.apply(ServiceStateEvent::StartRequested, 2).unwrap();
        state
            .apply(ServiceStateEvent::TransportConnected, 3)
            .unwrap();
        state
            .apply(ServiceStateEvent::ToolSyncSucceeded { tools: vec![] }, 4)
            .unwrap();
        state
            .apply(
                ServiceStateEvent::HealthObserved {
                    health: HealthState::Healthy,
                    metrics: HealthMetrics::default(),
                    failure: None,
                },
                5,
            )
            .unwrap();
        assert_eq!(state.readiness.reason, ReadinessReason::AuthRequired);
        state
            .apply(
                ServiceStateEvent::AuthChanged(AuthState::Authenticated, None),
                6,
            )
            .unwrap();
        assert_eq!(state.readiness.status, ReadinessStatus::Ready);
    }

    #[test]
    fn exhausted_recovery_requires_manual_start_to_clear() {
        let mut state = state(DesiredState::Running);
        state.apply(ServiceStateEvent::StartRequested, 2).unwrap();
        state
            .apply(
                ServiceStateEvent::RecoveryExhausted {
                    attempts: 3,
                    failure: failure(FailurePhase::Recovery, false),
                },
                3,
            )
            .unwrap();
        assert!(matches!(
            state.recovery,
            RecoveryState::Exhausted { attempts: 3 }
        ));
        state.apply(ServiceStateEvent::StartRequested, 4).unwrap();
        assert_eq!(state.recovery, RecoveryState::Idle);
        assert_eq!(state.phase, RuntimePhase::Starting);
    }
}
