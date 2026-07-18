use crate::state::{ReadinessStatus, ServiceState};
use crate::store::prelude::*;

impl MCPStore {
    pub async fn instance_status(&self, instance_id: InstanceId) -> Result<serde_json::Value> {
        let state = self.instance_status_entry(instance_id).await?;
        serde_json::to_value(state).map_err(|error| {
            StoreError::Other(format!("Instance status serialization failed: {error}"))
        })
    }

    pub async fn instance_status_entry(&self, instance_id: InstanceId) -> Result<ServiceState> {
        self.state_manager
            .get(instance_id)
            .await?
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))
    }

    pub async fn check_instances(&self, instance_ids: &[InstanceId]) -> Result<Vec<ServiceState>> {
        let mut states = Vec::with_capacity(instance_ids.len());
        for instance_id in instance_ids {
            states.push(self.health_check(*instance_id).await?);
        }
        Ok(states)
    }

    pub async fn wait_instance_ready(
        &self,
        instance_id: InstanceId,
        timeout_secs: u64,
    ) -> Result<ServiceState> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            let state = self.health_check(instance_id).await?;
            if state.readiness.status == ReadinessStatus::Ready {
                return Ok(state);
            }
            if !self.pool.is_connected(instance_id).await {
                match self.ensure_instance_connected(instance_id).await {
                    Ok(()) => continue,
                    Err(_) if tokio::time::Instant::now() < deadline => {
                        tokio::time::sleep(Self::retry_poll_interval(&state)).await;
                        continue;
                    }
                    Err(_) => {}
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(StoreError::Other(format!(
                    "Wait for instance ready timed out: {instance_id} (service={}, scope={:?}, readiness={:?}, error={})",
                    state.service_name,
                    state.scope,
                    state.readiness.reason,
                    state
                        .failure
                        .as_ref()
                        .map(|failure| failure.message.as_str())
                        .unwrap_or("none"),
                )));
            }
            tokio::time::sleep(Self::retry_poll_interval(&state)).await;
        }
    }
}
