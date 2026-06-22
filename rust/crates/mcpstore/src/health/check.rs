use crate::store::prelude::*;

impl MCPStore {
    pub async fn health_check(&self, name: &str) -> Result<ServiceStatus> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_service(name).await.is_none() {
            return Err(StoreError::ServiceNotFound(name.to_string()));
        }

        let cached = self.sync_retry_window(name).await?;
        if self.pool.is_connected(name).await {
            if let Some(status) = cached {
                if matches!(
                    status.health_status,
                    HealthStatus::Healthy | HealthStatus::Ready | HealthStatus::Startup
                ) {
                    return Ok(status);
                }
            }
            return self
                .record_health_check_result(name, true, None, None)
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

        let status = if let Some(entry) = self.registry.find_service(name).await {
            match entry.status {
                ConnectionStatus::Connected => HealthStatus::Healthy,
                ConnectionStatus::Connecting => HealthStatus::Startup,
                ConnectionStatus::Disconnected => HealthStatus::Disconnected,
                ConnectionStatus::Error => HealthStatus::Degraded,
            }
        } else {
            return Err(StoreError::ServiceNotFound(name.to_string()));
        };

        let tools = if matches!(status, HealthStatus::Healthy | HealthStatus::Ready) {
            self.tool_statuses_with_availability(name, ToolAvailability::Available)
                .await?
        } else {
            self.tool_statuses_with_availability(name, ToolAvailability::Unavailable)
                .await?
        };
        let mut state = self.load_or_default_status(name).await?;
        state.health_status = status.clone();
        state.last_health_check = Self::now_timestamp();
        state.tools = tools;
        if matches!(status, HealthStatus::Healthy | HealthStatus::Ready) {
            state.connection_attempts = 0;
            state.current_error = None;
            state.window_error_rate = Some(0.0);
            state.next_retry_time = None;
            state.hard_deadline = None;
            state.lease_deadline = None;
        }
        self.put_service_status_payload(&state).await?;
        Ok(state)
    }

    pub async fn record_health_check_result(
        &self,
        name: &str,
        ok: bool,
        latency_ms: Option<f64>,
        error: Option<String>,
    ) -> Result<ServiceStatus> {
        if self.registry.find_service(name).await.is_none() {
            return Err(StoreError::ServiceNotFound(name.to_string()));
        }

        if ok {
            let mut payload = self.load_or_default_status(name).await?;
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
                .tool_statuses_with_availability(name, ToolAvailability::Available)
                .await?;
            payload.window_error_rate = Some(0.0);
            payload.latency_p95 = latency_ms;
            payload.latency_p99 = latency_ms;
            payload.sample_size = Some(1);
            payload.next_retry_time = None;
            payload.hard_deadline = None;
            payload.lease_deadline = None;
            if self.pool.is_connected(name).await {
                self.registry
                    .update_status(name, ConnectionStatus::Connected)
                    .await;
            }
            self.put_service_status_payload(&payload).await?;
            return Ok(payload);
        }

        self.pool.disconnect(name).await.ok();
        self.registry
            .update_status(name, ConnectionStatus::Error)
            .await;
        self.mark_service_retryable_failure(
            name,
            error.unwrap_or_else(|| "Health check failed".to_string()),
        )
        .await
    }

}
