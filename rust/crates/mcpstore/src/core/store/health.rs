use super::*;

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

    pub async fn cached_service_status(&self, name: &str) -> Result<Option<ServiceStatus>> {
        let value = self.cache.get_state("service_status", name).await?;
        match value {
            Some(value) => Ok(Some(serde_json::from_value(value).map_err(|e| {
                StoreError::Other(format!("Service status deserialization failed: {e}"))
            })?)),
            None => Ok(None),
        }
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

    pub(super) async fn set_service_status(
        &self,
        name: &str,
        health_status: HealthStatus,
        error: Option<String>,
        tools: Vec<ToolStatusItem>,
    ) -> Result<()> {
        let payload = self.service_status_payload(name, health_status, error, tools);
        self.put_service_status_payload(&payload).await
    }

    pub(super) async fn put_service_status_payload(&self, payload: &ServiceStatus) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return Ok(());
        }

        self.cache
            .put_state(
                "service_status",
                &payload.service_global_name,
                serde_json::to_value(payload).unwrap_or_default(),
            )
            .await
            .map_err(Into::into)
    }

    pub(super) fn service_status_payload(
        &self,
        name: &str,
        health_status: HealthStatus,
        error: Option<String>,
        tools: Vec<ToolStatusItem>,
    ) -> ServiceStatus {
        ServiceStatus {
            service_global_name: name.to_string(),
            health_status,
            last_health_check: Self::now_timestamp(),
            connection_attempts: 0,
            max_connection_attempts: self.runtime_config.max_connection_attempts,
            current_error: error,
            tools,
            window_error_rate: None,
            latency_p95: None,
            latency_p99: None,
            sample_size: None,
            next_retry_time: None,
            hard_deadline: None,
            lease_deadline: None,
        }
    }

    pub(super) fn now_timestamp() -> i64 {
        chrono::Utc::now().timestamp()
    }

    pub(super) fn now_timestamp_f64() -> f64 {
        Self::now_timestamp() as f64
    }

    fn retry_delay_secs(&self, attempts: i32) -> i64 {
        let exponent = attempts.saturating_sub(1).clamp(0, 6) as u32;
        let delay = self
            .runtime_config
            .retry_backoff_base_secs
            .saturating_mul(2_i64.pow(exponent));
        delay.min(self.runtime_config.retry_backoff_max_secs)
    }

    pub(super) fn retry_wait_seconds(status: &ServiceStatus, now: f64) -> Option<i64> {
        if status.health_status != HealthStatus::CircuitOpen {
            return None;
        }
        let retry_at = status.next_retry_time?;
        if retry_at <= now {
            return None;
        }
        Some((retry_at - now).ceil() as i64)
    }

    pub(super) fn retry_exhausted(status: &ServiceStatus, now: f64) -> bool {
        status.health_status == HealthStatus::Disconnected
            && status.current_error.is_some()
            && (status.connection_attempts >= status.max_connection_attempts
                || status
                    .hard_deadline
                    .map(|deadline| now >= deadline)
                    .unwrap_or(false))
    }

    pub(super) fn retry_poll_interval(status: &ServiceStatus) -> std::time::Duration {
        let now = Self::now_timestamp_f64();
        if let Some(retry_at) = status.next_retry_time {
            if retry_at > now {
                let wait_secs = (retry_at - now).clamp(0.1, 1.0);
                return std::time::Duration::from_secs_f64(wait_secs);
            }
        }
        std::time::Duration::from_millis(300)
    }

    pub(super) fn health_status_name(status: &HealthStatus) -> &'static str {
        match status {
            HealthStatus::Init => "init",
            HealthStatus::Startup => "startup",
            HealthStatus::Ready => "ready",
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::CircuitOpen => "circuit_open",
            HealthStatus::HalfOpen => "half_open",
            HealthStatus::Disconnected => "disconnected",
        }
    }

    fn connection_status_from_health_status(status: &HealthStatus) -> ConnectionStatus {
        match status {
            HealthStatus::Healthy | HealthStatus::Ready => ConnectionStatus::Connected,
            HealthStatus::Startup | HealthStatus::HalfOpen => ConnectionStatus::Connecting,
            HealthStatus::Init | HealthStatus::Disconnected => ConnectionStatus::Disconnected,
            HealthStatus::Degraded | HealthStatus::CircuitOpen => ConnectionStatus::Error,
        }
    }

    pub(super) async fn hydrate_service_status_from_cache(
        &self,
        service: &mut ServiceEntry,
    ) -> Result<()> {
        if let Some(status) = self.cached_service_status(&service.name).await? {
            service.status = Self::connection_status_from_health_status(&status.health_status);
        }
        Ok(())
    }

    async fn load_or_default_status(&self, name: &str) -> Result<ServiceStatus> {
        Ok(self.cached_service_status(name).await?.unwrap_or_else(|| {
            self.service_status_payload(name, HealthStatus::Init, None, Vec::new())
        }))
    }

    async fn tool_statuses_with_availability(
        &self,
        name: &str,
        availability: ToolAvailability,
    ) -> Result<Vec<ToolStatusItem>> {
        if let Some(status) = self.cached_service_status(name).await? {
            if !status.tools.is_empty() {
                return Ok(status
                    .tools
                    .into_iter()
                    .map(|mut tool| {
                        tool.status = availability.clone();
                        tool
                    })
                    .collect());
            }
        }

        let tools = self.registry.list_service_tools(name).await;
        let mut statuses = Vec::with_capacity(tools.len());
        for tool in tools {
            statuses.push(ToolStatusItem {
                tool_global_name: generate_tool_global_name(name, &tool.name)?,
                tool_original_name: tool.name,
                status: availability.clone(),
            });
        }
        Ok(statuses)
    }

    pub(super) async fn mark_service_retryable_failure(
        &self,
        name: &str,
        error: String,
    ) -> Result<ServiceStatus> {
        let mut payload = self.load_or_default_status(name).await?;
        let now = Self::now_timestamp_f64();
        let attempts = payload.connection_attempts.saturating_add(1);
        let hard_deadline = payload
            .hard_deadline
            .unwrap_or(now + self.runtime_config.reconnect_hard_timeout_secs as f64);

        payload.last_health_check = now as i64;
        payload.connection_attempts = attempts;
        payload.current_error = Some(error);
        payload.tools = self
            .tool_statuses_with_availability(name, ToolAvailability::Unavailable)
            .await?;
        payload.window_error_rate = Some(1.0);
        payload.hard_deadline = Some(hard_deadline);
        payload.lease_deadline = None;

        if attempts >= payload.max_connection_attempts || now >= hard_deadline {
            payload.health_status = HealthStatus::Disconnected;
            payload.next_retry_time = None;
            self.registry
                .update_status(name, ConnectionStatus::Disconnected)
                .await;
        } else {
            payload.health_status = HealthStatus::CircuitOpen;
            payload.next_retry_time = Some(now + self.retry_delay_secs(attempts) as f64);
        }

        self.put_service_status_payload(&payload).await?;
        Ok(payload)
    }

    pub(super) async fn sync_retry_window(&self, name: &str) -> Result<Option<ServiceStatus>> {
        let Some(mut status) = self.cached_service_status(name).await? else {
            return Ok(None);
        };
        let now = Self::now_timestamp_f64();

        match status.health_status {
            HealthStatus::CircuitOpen => {
                if status.connection_attempts >= status.max_connection_attempts
                    || status
                        .hard_deadline
                        .map(|deadline| now >= deadline)
                        .unwrap_or(false)
                {
                    status.health_status = HealthStatus::Disconnected;
                    status.last_health_check = now as i64;
                    status.next_retry_time = None;
                    status.lease_deadline = None;
                    self.registry
                        .update_status(name, ConnectionStatus::Disconnected)
                        .await;
                    self.put_service_status_payload(&status).await?;
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
                    self.put_service_status_payload(&status).await?;
                }
            }
            HealthStatus::HalfOpen => {
                if status
                    .lease_deadline
                    .map(|deadline| now >= deadline)
                    .unwrap_or(false)
                {
                    let status = self
                        .mark_service_retryable_failure(name, "半开探测超时".to_string())
                        .await?;
                    return Ok(Some(status));
                }
            }
            _ => {}
        }

        Ok(Some(status))
    }
}
