use crate::state::{FailureInfo, FailurePhase, RecoveryState, ServiceState, ServiceStateEvent};
use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn record_instance_failure(
        &self,
        instance_id: InstanceId,
        error: String,
    ) -> Result<ServiceState> {
        if self.is_db_source() {
            return self
                .state_manager
                .get(instance_id)
                .await?
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()));
        }
        self.mark_instance_retryable_failure(instance_id, error)
            .await
    }

    fn jitter_for_instance(&self, instance_id: InstanceId, attempts: u32) -> f64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        instance_id.hash(&mut hasher);
        attempts.hash(&mut hasher);
        let hash = hasher.finish();
        (hash as f64 / u64::MAX as f64) * 2.0 - 1.0
    }

    fn retry_delay_secs(&self, attempts: u32, instance_id: InstanceId) -> i64 {
        let exponent = attempts.saturating_sub(1).min(6);
        let delay = self
            .runtime_config
            .retry_backoff_base_secs
            .saturating_mul(2_i64.pow(exponent));
        let capped = delay.min(self.runtime_config.retry_backoff_max_secs);
        let jitter_ratio = self.runtime_config.backoff_jitter_ratio.clamp(0.0, 1.0);
        if jitter_ratio == 0.0 {
            return capped;
        }
        let jitter = self.jitter_for_instance(instance_id, attempts) * jitter_ratio;
        (capped as f64 * (1.0 + jitter)).max(0.0) as i64
    }

    pub(crate) fn retry_wait_seconds(state: &ServiceState, now: f64) -> Option<i64> {
        let RecoveryState::Waiting { retry_at, .. } = state.recovery else {
            return None;
        };
        (retry_at > now).then(|| (retry_at - now).ceil() as i64)
    }

    pub(crate) fn retry_exhausted(state: &ServiceState, now: f64) -> bool {
        match state.recovery {
            RecoveryState::Exhausted { .. } => true,
            RecoveryState::Waiting { hard_deadline, .. }
            | RecoveryState::Probing { hard_deadline, .. } => now >= hard_deadline,
            RecoveryState::Idle => false,
        }
    }

    pub(crate) fn retry_poll_interval(state: &ServiceState) -> std::time::Duration {
        let now = Self::now_timestamp_f64();
        if let RecoveryState::Waiting { retry_at, .. } = state.recovery {
            if retry_at > now {
                return std::time::Duration::from_secs_f64((retry_at - now).clamp(0.1, 1.0));
            }
        }
        std::time::Duration::from_millis(300)
    }

    pub(crate) async fn mark_instance_retryable_failure(
        &self,
        instance_id: InstanceId,
        error: String,
    ) -> Result<ServiceState> {
        let now = Self::now_timestamp();
        let now_f64 = now as f64;
        let current = self
            .state_manager
            .get(instance_id)
            .await?
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let attempts = match current.recovery {
            RecoveryState::Waiting { attempt, .. }
            | RecoveryState::Probing { attempt, .. }
            | RecoveryState::Exhausted { attempts: attempt } => attempt.saturating_add(1),
            RecoveryState::Idle => 1,
        };
        let hard_deadline = match current.recovery {
            RecoveryState::Waiting { hard_deadline, .. }
            | RecoveryState::Probing { hard_deadline, .. } => hard_deadline,
            RecoveryState::Idle | RecoveryState::Exhausted { .. } => {
                now_f64 + self.runtime_config.reconnect_hard_timeout_secs as f64
            }
        };
        let lifecycle = self.resolved_instance_lifecycle(instance_id).await?;
        let should_restart = lifecycle
            .restart_policy
            .should_restart_after_failure(attempts as i32)
            && now_f64 < hard_deadline;
        let failure = FailureInfo {
            phase: FailurePhase::Recovery,
            code: "service_unavailable".to_string(),
            retryable: should_restart,
            message: error,
            since: now,
        };

        let failed = self
            .state_manager
            .dispatch(
                instance_id,
                ServiceStateEvent::TransportFailed(failure.clone()),
                now,
            )
            .await?;
        if should_restart {
            let retry_at = now_f64 + self.retry_delay_secs(attempts, instance_id) as f64;
            Ok(self
                .state_manager
                .dispatch(
                    instance_id,
                    ServiceStateEvent::RecoveryScheduled {
                        attempt: attempts,
                        retry_at,
                        hard_deadline,
                    },
                    now,
                )
                .await?)
        } else {
            let _ = failed;
            Ok(self
                .state_manager
                .dispatch(
                    instance_id,
                    ServiceStateEvent::RecoveryExhausted { attempts, failure },
                    now,
                )
                .await?)
        }
    }
}
