use crate::cache::models::InstanceStatus;
use crate::store::prelude::*;

impl MCPStore {
    fn retry_delay_secs(&self, attempts: i32) -> i64 {
        let exponent = attempts.saturating_sub(1).clamp(0, 6) as u32;
        let delay = self
            .runtime_config
            .retry_backoff_base_secs
            .saturating_mul(2_i64.pow(exponent));
        delay.min(self.runtime_config.retry_backoff_max_secs)
    }

    pub(crate) fn retry_wait_seconds(status: &InstanceStatus, now: f64) -> Option<i64> {
        if status.health_status != HealthStatus::CircuitOpen {
            return None;
        }
        let retry_at = status.next_retry_time?;
        if retry_at <= now {
            return None;
        }
        Some((retry_at - now).ceil() as i64)
    }

    pub(crate) fn retry_exhausted(status: &InstanceStatus, now: f64) -> bool {
        status.health_status == HealthStatus::Disconnected
            && status.current_error.is_some()
            && (status.connection_attempts >= status.max_connection_attempts
                || status
                    .hard_deadline
                    .map(|deadline| now >= deadline)
                    .unwrap_or(true))
    }

    pub(crate) fn retry_poll_interval(status: &InstanceStatus) -> std::time::Duration {
        let now = Self::now_timestamp_f64();
        if let Some(retry_at) = status.next_retry_time {
            if retry_at > now {
                let wait_secs = (retry_at - now).clamp(0.1, 1.0);
                return std::time::Duration::from_secs_f64(wait_secs);
            }
        }
        std::time::Duration::from_millis(300)
    }

    pub(crate) async fn mark_instance_retryable_failure(
        &self,
        instance_id: InstanceId,
        error: String,
    ) -> Result<InstanceStatus> {
        let mut payload = self.load_or_default_status(instance_id).await?;
        let now = Self::now_timestamp_f64();
        let attempts = payload.connection_attempts.saturating_add(1);
        let lifecycle = self.resolved_instance_lifecycle(instance_id).await?;
        let restart_attempts = payload.lifecycle_state.restart_attempts.saturating_add(1);
        payload.lifecycle_state.restart_attempts = restart_attempts;
        let hard_deadline = payload
            .hard_deadline
            .unwrap_or(now + self.runtime_config.reconnect_hard_timeout_secs as f64);
        let should_restart = !payload.lifecycle_state.manually_stopped
            && lifecycle
                .restart_policy
                .should_restart_after_failure(restart_attempts);

        payload.last_health_check = now as i64;
        payload.connection_attempts = attempts;
        payload.current_error = Some(error);
        payload.tools = self
            .tool_statuses_with_availability(instance_id, ToolAvailability::Unavailable)
            .await?;
        payload.window_error_rate = Some(1.0);
        payload.hard_deadline = Some(hard_deadline);
        payload.lease_deadline = None;

        if !should_restart {
            payload.health_status = HealthStatus::Disconnected;
            payload.next_retry_time = None;
            self.registry
                .update_status(instance_id, ConnectionStatus::Disconnected)
                .await;
        } else {
            payload.health_status = HealthStatus::CircuitOpen;
            payload.next_retry_time = Some(now + self.retry_delay_secs(attempts) as f64);
        }

        self.put_instance_status(&payload).await?;
        Ok(payload)
    }

    pub(crate) async fn sync_retry_window(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<InstanceStatus>> {
        let Some(mut status) = self.cached_instance_status(instance_id).await? else {
            return Ok(None);
        };
        let now = Self::now_timestamp_f64();
        let lifecycle = self.resolved_instance_lifecycle(instance_id).await?;
        let should_restart = !status.lifecycle_state.manually_stopped
            && lifecycle
                .restart_policy
                .should_restart_after_failure(status.lifecycle_state.restart_attempts.max(1));

        match status.health_status {
            HealthStatus::CircuitOpen => {
                if !should_restart {
                    status.health_status = HealthStatus::Disconnected;
                    status.last_health_check = now as i64;
                    status.next_retry_time = None;
                    status.lease_deadline = None;
                    self.registry
                        .update_status(instance_id, ConnectionStatus::Disconnected)
                        .await;
                    self.put_instance_status(&status).await?;
                    return Ok(Some(status));
                }
                if status
                    .next_retry_time
                    .map(|retry_at| retry_at <= now)
                    .unwrap_or(true)
                {
                    status.health_status = HealthStatus::HalfOpen;
                    status.last_health_check = now as i64;
                    status.next_retry_time = None;
                    status.lease_deadline =
                        Some(now + self.runtime_config.half_open_lease_secs as f64);
                    self.put_instance_status(&status).await?;
                }
            }
            HealthStatus::HalfOpen => {
                if status
                    .lease_deadline
                    .map(|deadline| now >= deadline)
                    .unwrap_or(false)
                {
                    let status = self
                        .mark_instance_retryable_failure(
                            instance_id,
                            "Half-open health probe timed out".to_string(),
                        )
                        .await?;
                    return Ok(Some(status));
                }
            }
            _ => {}
        }

        Ok(Some(status))
    }
}
