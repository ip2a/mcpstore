use crate::cache::models::{InstanceStatus, ToolStatusItem};
use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn set_instance_status(
        &self,
        instance_id: InstanceId,
        health_status: HealthStatus,
        error: Option<String>,
        tools: Vec<ToolStatusItem>,
    ) -> Result<()> {
        let payload = self
            .new_instance_status(instance_id, health_status, error, tools)
            .await?;
        self.put_instance_status(&payload).await
    }

    pub(crate) async fn new_instance_status(
        &self,
        instance_id: InstanceId,
        health_status: HealthStatus,
        error: Option<String>,
        tools: Vec<ToolStatusItem>,
    ) -> Result<InstanceStatus> {
        let instance = self.instance(instance_id).await?;
        Ok(InstanceStatus {
            instance_id,
            service_name: instance.service_name,
            scope: instance.scope,
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
            lifecycle_state: ServiceLifecycleState::default(),
        })
    }

    pub(crate) async fn update_lifecycle_state<F>(
        &self,
        instance_id: InstanceId,
        update: F,
    ) -> Result<InstanceStatus>
    where
        F: FnOnce(&mut ServiceLifecycleState),
    {
        let mut status = self.load_or_default_status(instance_id).await?;
        update(&mut status.lifecycle_state);
        self.put_instance_status(&status).await?;
        Ok(status)
    }

    pub(crate) async fn clear_lifecycle_manual_stop(&self, instance_id: InstanceId) -> Result<()> {
        self.update_lifecycle_state(instance_id, |state| {
            state.manually_stopped = false;
            state.manually_stopped_at = None;
            state.manual_stop_persistent = false;
            state.restart_attempts = 0;
        })
        .await?;
        Ok(())
    }

    pub(crate) async fn rebuild_observed_status_for_startup(
        &self,
        instance_id: InstanceId,
    ) -> Result<InstanceStatus> {
        let persisted_lifecycle = self
            .cached_instance_status(instance_id)
            .await?
            .map(|status| status.lifecycle_state)
            .filter(|state| state.manual_stop_persistent);
        let mut status = self
            .new_instance_status(instance_id, HealthStatus::Init, None, Vec::new())
            .await?;
        if let Some(lifecycle) = persisted_lifecycle {
            status.lifecycle_state.manually_stopped = true;
            status.lifecycle_state.manually_stopped_at = lifecycle.manually_stopped_at;
            status.lifecycle_state.manual_stop_persistent = true;
        }
        self.put_instance_status(&status).await?;
        Ok(status)
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
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::CircuitOpen => "circuit_open",
            HealthStatus::HalfOpen => "half_open",
            HealthStatus::Disconnected => "disconnected",
        }
    }

    pub(crate) fn connection_status_from_health_status(status: &HealthStatus) -> ConnectionStatus {
        match status {
            HealthStatus::Healthy => ConnectionStatus::Connected,
            HealthStatus::Startup | HealthStatus::HalfOpen => ConnectionStatus::Connecting,
            HealthStatus::Init | HealthStatus::Disconnected => ConnectionStatus::Disconnected,
            HealthStatus::Degraded | HealthStatus::CircuitOpen => ConnectionStatus::Error,
        }
    }

    pub(crate) async fn hydrate_instance_status_from_cache(
        &self,
        instance: &mut ServiceInstance,
    ) -> Result<()> {
        if let Some(status) = self.cached_instance_status(instance.instance_id).await? {
            instance.status = Self::connection_status_from_health_status(&status.health_status);
        }
        Ok(())
    }

    pub(crate) async fn load_or_default_status(
        &self,
        instance_id: InstanceId,
    ) -> Result<InstanceStatus> {
        match self.cached_instance_status(instance_id).await? {
            Some(status) => Ok(status),
            None => {
                self.new_instance_status(instance_id, HealthStatus::Init, None, Vec::new())
                    .await
            }
        }
    }

    pub(crate) async fn tool_statuses_with_availability(
        &self,
        instance_id: InstanceId,
        availability: ToolAvailability,
    ) -> Result<Vec<ToolStatusItem>> {
        if let Some(status) = self.cached_instance_status(instance_id).await? {
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

        Ok(self
            .registry
            .list_instance_tools(instance_id)
            .await
            .into_iter()
            .map(|tool| ToolStatusItem {
                tool_name: tool.name,
                status: availability.clone(),
            })
            .collect())
    }

    async fn instance(&self, instance_id: InstanceId) -> Result<ServiceInstance> {
        self.registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))
    }
}
