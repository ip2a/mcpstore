use crate::cache::models::InstanceStatus;
use crate::health::state_machine::{HealthObservation, ObservationKind};
use crate::store::prelude::*;

impl MCPStore {
    pub async fn health_check(&self, instance_id: InstanceId) -> Result<InstanceStatus> {
        self.refresh_from_db_if_needed().await?;
        if self.is_db_source() {
            return self
                .cached_instance_status(instance_id)
                .await?
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()));
        }
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;

        let cached = self.cached_instance_status(instance_id).await?;
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
        self.record_observation(
            instance_id,
            ObservationKind::Liveness,
            ok,
            latency_ms,
            error,
        )
        .await
    }

    pub(crate) async fn record_tool_observation(
        &self,
        instance_id: InstanceId,
        ok: bool,
        latency_ms: Option<f64>,
        error: Option<String>,
    ) -> Result<InstanceStatus> {
        self.record_observation(
            instance_id,
            ObservationKind::ToolCall,
            ok,
            latency_ms,
            error,
        )
        .await
    }

    async fn record_observation(
        &self,
        instance_id: InstanceId,
        observation_kind: ObservationKind,
        ok: bool,
        latency_ms: Option<f64>,
        error: Option<String>,
    ) -> Result<InstanceStatus> {
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }

        if let Some(supervisor) = &self.supervisor {
            supervisor
                .register(instance_id, HealthStatus::Healthy)
                .await;
            let observed_at = Self::now_timestamp_f64();
            supervisor
                .observe_and_commit(
                    instance_id,
                    HealthObservation {
                        observed_at,
                        kind: observation_kind,
                        succeeded: ok,
                        latency_ms,
                    },
                )
                .await;
            return self
                .cached_instance_status(instance_id)
                .await?
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()));
        }

        let mut payload = self.load_or_default_status(instance_id).await?;
        payload.last_health_check = Self::now_timestamp();

        if ok {
            payload.health_status = if latency_ms
                .map(|value| value >= self.runtime_config.supervisor_policy.latency_p95_warn_ms)
                .unwrap_or(false)
            {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            };
            payload.connection_attempts = 0;
            payload.current_error = None;
            payload.tools = self
                .tool_statuses_with_availability(instance_id, ToolAvailability::Available)
                .await?;
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

        payload.current_error = error;
        self.put_instance_status(&payload).await?;
        Ok(payload)
    }
}
