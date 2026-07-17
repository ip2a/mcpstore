use crate::cache::models::InstanceStatus;
use crate::store::prelude::*;

impl MCPStore {
    pub async fn health_check(&self, instance_id: InstanceId) -> Result<InstanceStatus> {
        self.refresh_from_db_if_needed().await?;
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;

        let cached = self.sync_retry_window(instance_id).await?;
        if self.pool.is_connected(instance_id).await {
            if let Some(status) = cached {
                if matches!(
                    status.health_status,
                    HealthStatus::Healthy | HealthStatus::Startup
                ) {
                    return Ok(status);
                }
            }
            return self
                .record_health_check_result(instance_id, true, None, None)
                .await;
        }

        if let Some(status) = cached {
            if matches!(
                status.health_status,
                HealthStatus::Init
                    | HealthStatus::Startup
                    | HealthStatus::Degraded
                    | HealthStatus::CircuitOpen
                    | HealthStatus::HalfOpen
                    | HealthStatus::Disconnected
            ) {
                return Ok(status);
            }
        }

        let health_status = match instance.status {
            ConnectionStatus::Connected => HealthStatus::Healthy,
            ConnectionStatus::Connecting => HealthStatus::Startup,
            ConnectionStatus::Disconnected => HealthStatus::Disconnected,
            ConnectionStatus::Error => HealthStatus::Degraded,
        };

        let availability = if matches!(health_status, HealthStatus::Healthy) {
            ToolAvailability::Available
        } else {
            ToolAvailability::Unavailable
        };
        let tools = self
            .tool_statuses_with_availability(instance_id, availability)
            .await?;
        let mut state = self.load_or_default_status(instance_id).await?;
        state.health_status = health_status.clone();
        state.last_health_check = Self::now_timestamp();
        state.tools = tools;
        if matches!(health_status, HealthStatus::Healthy) {
            state.connection_attempts = 0;
            state.current_error = None;
            state.window_error_rate = Some(0.0);
            state.next_retry_time = None;
            state.hard_deadline = None;
            state.lease_deadline = None;
            state.lifecycle_state.restart_attempts = 0;
        }
        self.put_instance_status(&state).await?;
        Ok(state)
    }

    pub async fn record_health_check_result(
        &self,
        instance_id: InstanceId,
        ok: bool,
        latency_ms: Option<f64>,
        error: Option<String>,
    ) -> Result<InstanceStatus> {
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }

        if ok {
            let mut payload = self.load_or_default_status(instance_id).await?;
            payload.health_status = if latency_ms
                .map(|value| value >= self.runtime_config.health_warn_latency_ms)
                .unwrap_or(false)
            {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            };
            payload.last_health_check = Self::now_timestamp();
            payload.connection_attempts = 0;
            payload.current_error = None;
            payload.tools = self
                .tool_statuses_with_availability(instance_id, ToolAvailability::Available)
                .await?;
            payload.window_error_rate = Some(0.0);
            payload.latency_p95 = latency_ms;
            payload.latency_p99 = latency_ms;
            payload.sample_size = Some(1);
            payload.next_retry_time = None;
            payload.hard_deadline = None;
            payload.lease_deadline = None;
            payload.lifecycle_state.restart_attempts = 0;
            if self.pool.is_connected(instance_id).await {
                self.registry
                    .update_status(instance_id, ConnectionStatus::Connected)
                    .await;
            }
            self.put_instance_status(&payload).await?;
            return Ok(payload);
        }

        self.pool.disconnect(instance_id).await.ok();
        self.registry
            .update_status(instance_id, ConnectionStatus::Error)
            .await;
        self.mark_instance_retryable_failure(
            instance_id,
            error.unwrap_or_else(|| "Health check failed".to_string()),
        )
        .await
    }
}
