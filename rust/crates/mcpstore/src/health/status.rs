use crate::store::prelude::*;

impl MCPStore {
    pub async fn cached_service_status(&self, name: &str) -> Result<Option<ServiceStatus>> {
        let value = self.cache.get_state("service_status", name).await?;
        match value {
            Some(value) => Ok(Some(serde_json::from_value(value).map_err(|e| {
                StoreError::Other(format!("Service status deserialization failed: {e}"))
            })?)),
            None => Ok(None),
        }
    }

    pub(crate) async fn set_service_status(
        &self,
        name: &str,
        health_status: HealthStatus,
        error: Option<String>,
        tools: Vec<ToolStatusItem>,
    ) -> Result<()> {
        let payload = self.service_status_payload(name, health_status, error, tools);
        self.put_service_status_payload(&payload).await
    }

    pub(crate) async fn put_service_status_payload(&self, payload: &ServiceStatus) -> Result<()> {
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

    pub(crate) fn service_status_payload(
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

    pub(crate) fn now_timestamp() -> i64 {
        chrono::Utc::now().timestamp()
    }

    pub(crate) fn now_timestamp_f64() -> f64 {
        Self::now_timestamp() as f64
    }

    pub(crate) fn health_status_name(status: &HealthStatus) -> &'static str {
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

    pub(crate) async fn hydrate_service_status_from_cache(
        &self,
        service: &mut ServiceEntry,
    ) -> Result<()> {
        if let Some(status) = self.cached_service_status(&service.name).await? {
            service.status = Self::connection_status_from_health_status(&status.health_status);
        }
        Ok(())
    }

    pub(crate) async fn load_or_default_status(&self, name: &str) -> Result<ServiceStatus> {
        Ok(self.cached_service_status(name).await?.unwrap_or_else(|| {
            self.service_status_payload(name, HealthStatus::Init, None, Vec::new())
        }))
    }

    pub(crate) async fn tool_statuses_with_availability(
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
}
