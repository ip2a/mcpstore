use crate::health::state_machine::{HealthObservation, ObservationKind};
use crate::state::{
    FailureInfo, FailurePhase, HealthMetrics, HealthState, RecoveryState, ServiceState,
    ServiceStateEvent,
};
use crate::store::prelude::*;

impl MCPStore {
    pub async fn health_check(&self, instance_id: InstanceId) -> Result<ServiceState> {
        let current = self
            .state_manager
            .get(instance_id)
            .await?
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        if self.is_db_source() || self.is_openapi_virtual_instance(instance_id).await? {
            return Ok(current);
        }

        if !self.pool.is_connected(instance_id).await {
            if current.desired == crate::state::DesiredState::Running
                && matches!(current.recovery, RecoveryState::Idle)
            {
                return self
                    .mark_instance_retryable_failure(
                        instance_id,
                        "Transport is not connected".to_string(),
                    )
                    .await;
            }
            return Ok(current);
        }

        let started_at = std::time::Instant::now();
        let timeout =
            std::time::Duration::from_secs_f64(self.runtime_config.ping_timeout_http_secs.max(0.1));
        let ok = self.pool.ping(instance_id, timeout).await.is_ok();
        let latency_ms = Some(started_at.elapsed().as_secs_f64() * 1000.0);
        self.record_health_check_result(
            instance_id,
            ok,
            latency_ms,
            (!ok).then(|| "Health probe failed".to_string()),
        )
        .await
    }

    pub(crate) async fn record_openapi_availability(
        &self,
        instance_id: InstanceId,
        available: bool,
        latency_ms: Option<f64>,
        error: Option<String>,
    ) -> Result<ServiceState> {
        self.record_observation(
            instance_id,
            ObservationKind::Liveness,
            available,
            latency_ms,
            error,
        )
        .await
    }

    pub async fn record_health_check_result(
        &self,
        instance_id: InstanceId,
        ok: bool,
        latency_ms: Option<f64>,
        error: Option<String>,
    ) -> Result<ServiceState> {
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
    ) -> Result<ServiceState> {
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
    ) -> Result<ServiceState> {
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }
        if self.is_db_source() {
            return self
                .state_manager
                .get(instance_id)
                .await?
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()));
        }

        if let Some(supervisor) = &self.supervisor {
            supervisor
                .observe_and_commit(
                    instance_id,
                    HealthObservation {
                        observed_at: Self::now_timestamp_f64(),
                        kind: observation_kind,
                        succeeded: ok,
                        latency_ms,
                    },
                )
                .await;
            return self
                .state_manager
                .get(instance_id)
                .await?
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()));
        }

        let health = if ok {
            if latency_ms.is_some_and(|value| {
                value >= self.runtime_config.supervisor_policy.latency_p95_warn_ms
            }) {
                HealthState::Degraded
            } else {
                HealthState::Healthy
            }
        } else {
            HealthState::Unhealthy
        };
        let now = Self::now_timestamp();
        Ok(self
            .state_manager
            .dispatch(
                instance_id,
                ServiceStateEvent::HealthObserved {
                    health,
                    metrics: HealthMetrics {
                        error_rate: Some(if ok { 0.0 } else { 1.0 }),
                        latency_p95_ms: latency_ms,
                        latency_p99_ms: latency_ms,
                        sample_size: 1,
                    },
                    failure: error.map(|message| FailureInfo {
                        phase: FailurePhase::Health,
                        code: "health_observation_failed".to_string(),
                        retryable: true,
                        message,
                        since: now,
                    }),
                },
                now,
            )
            .await?)
    }
}
