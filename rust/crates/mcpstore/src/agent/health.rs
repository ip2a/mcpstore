use crate::store::prelude::*;

impl MCPStore {
    pub async fn service_status_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
    ) -> Result<serde_json::Value> {
        let status = self
            .service_status_entry_scoped(agent_id, service_name)
            .await?;
        serde_json::to_value(status).map_err(|error| {
            StoreError::Other(format!("Service status serialization failed: {error}"))
        })
    }

    pub async fn service_status_entry_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
    ) -> Result<ServiceStatus> {
        let global_service_name = match agent_id {
            None => service_name.to_string(),
            Some(agent_id) => {
                self.resolve_service_name_for_agent(agent_id, service_name)
                    .await?
            }
        };
        let status = self
            .cached_service_status(&global_service_name)
            .await?
            .unwrap_or(self.health_check(&global_service_name).await?);
        Ok(status)
    }

    pub async fn check_services_scoped(&self, agent_id: Option<&str>) -> Result<serde_json::Value> {
        let statuses = self.check_service_health_scoped(agent_id).await?;
        let mut result = serde_json::Map::new();
        for status in statuses {
            result.insert(status.service_name, serde_json::json!(status.health_status));
        }
        Ok(serde_json::Value::Object(result))
    }

    pub async fn check_service_health_scoped(
        &self,
        agent_id: Option<&str>,
    ) -> Result<Vec<ScopedServiceHealth>> {
        let services = self.list_service_entries_scoped(agent_id).await?;
        let mut statuses = Vec::with_capacity(services.len());
        for service in services {
            let service_name = service.service.name.clone();
            let global_service_name = service
                .global_name
                .as_deref()
                .unwrap_or(service.service.name.as_str());
            let status = self.health_check(global_service_name).await?;
            statuses.push(ScopedServiceHealth {
                service_name,
                health_status: status.health_status,
            });
        }
        Ok(statuses)
    }

    pub async fn wait_service_ready(&self, name: &str, timeout_secs: u64) -> Result<ServiceStatus> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            let status = self.health_check(name).await?;
            if matches!(
                status.health_status,
                HealthStatus::Healthy | HealthStatus::Ready
            ) {
                return Ok(status);
            }
            if !self.pool.is_connected(name).await {
                match self.connect_service_internal(name, true).await {
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
                    "Wait for service ready timed out: {name} (status={}, error={})",
                    Self::health_status_name(&status.health_status),
                    status.current_error.unwrap_or_else(|| "none".to_string()),
                )));
            }
            tokio::time::sleep(Self::retry_poll_interval(&status)).await;
        }
    }
}
