use crate::cache::models::InstanceStatus;
use crate::store::prelude::*;

impl MCPStore {
    pub async fn instance_status(&self, instance_id: InstanceId) -> Result<serde_json::Value> {
        let status = self.instance_status_entry(instance_id).await?;
        serde_json::to_value(status).map_err(|error| {
            StoreError::Other(format!("Instance status serialization failed: {error}"))
        })
    }

    pub async fn instance_status_entry(&self, instance_id: InstanceId) -> Result<InstanceStatus> {
        match self.cached_instance_status(instance_id).await? {
            Some(status) => Ok(status),
            None => self.health_check(instance_id).await,
        }
    }

    pub async fn check_instances(
        &self,
        instance_ids: &[InstanceId],
    ) -> Result<Vec<InstanceStatus>> {
        let mut statuses = Vec::with_capacity(instance_ids.len());
        for instance_id in instance_ids {
            statuses.push(self.health_check(*instance_id).await?);
        }
        Ok(statuses)
    }

    pub async fn wait_instance_ready(
        &self,
        instance_id: InstanceId,
        timeout_secs: u64,
    ) -> Result<InstanceStatus> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            let status = self.health_check(instance_id).await?;
            if matches!(status.health_status, HealthStatus::Healthy) {
                return Ok(status);
            }
            if !self.pool.is_connected(instance_id).await {
                self.ensure_service_auto_start_allowed(instance_id).await?;
                match self.connect_service_internal(instance_id, true).await {
                    Ok(()) => continue,
                    Err(_) if tokio::time::Instant::now() < deadline => {
                        tokio::time::sleep(Self::retry_poll_interval(&status)).await;
                        continue;
                    }
                    Err(_) => {}
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(StoreError::Other(format!(
                    "Wait for instance ready timed out: {instance_id} (service={}, scope={:?}, status={}, error={})",
                    status.service_name,
                    status.scope,
                    Self::health_status_name(&status.health_status),
                    status.current_error.unwrap_or_else(|| "none".to_string()),
                )));
            }
            tokio::time::sleep(Self::retry_poll_interval(&status)).await;
        }
    }
}
