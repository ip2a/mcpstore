use crate::store::prelude::*;

impl MCPStore {
    pub async fn assign_service_to_agent(&self, agent_id: &str, service_name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceAssignRequested",
                    serde_json::json!({
                        "agent_id": agent_id,
                        "service_name": service_name,
                    }),
                )
                .await;
        }

        if self.registry.find_service(service_name).await.is_none() {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        }
        self.registry
            .add_to_agent_scope(agent_id, service_name)
            .await;
        self.cache_agent_scope(agent_id).await?;
        Ok(())
    }

    pub async fn unassign_service_from_agent(
        &self,
        agent_id: &str,
        service_name: &str,
    ) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceUnassignRequested",
                    serde_json::json!({
                        "agent_id": agent_id,
                        "service_name": service_name,
                    }),
                )
                .await;
        }

        let mut services = self.cached_agent_scope(agent_id).await?;
        services.retain(|name| name != service_name);
        self.registry.clear_agent_scope(agent_id).await;
        for name in &services {
            self.registry.add_to_agent_scope(agent_id, name).await;
        }
        self.cache_agent_scope_names(agent_id, services).await?;
        Ok(())
    }

    pub async fn list_agent_service_names(&self, agent_id: &str) -> Result<Vec<String>> {
        self.refresh_from_db_if_needed().await?;
        let registry_services = self.registry.list_agent_services(agent_id).await;
        if !registry_services.is_empty() {
            return Ok(registry_services);
        }
        self.cached_agent_scope(agent_id).await
    }

    pub async fn resolve_service_name_for_agent(
        &self,
        agent_id: &str,
        service_name: &str,
    ) -> Result<String> {
        self.refresh_from_db_if_needed().await?;
        let mut allowed = self.list_agent_service_names(agent_id).await?;
        allowed.sort();

        if allowed.iter().any(|name| name == service_name) {
            return Ok(service_name.to_string());
        }

        for global_service_name in allowed {
            let Some(service) = self.registry.find_service(&global_service_name).await else {
                continue;
            };
            if service.original_name == service_name {
                return Ok(global_service_name);
            }
        }

        Err(StoreError::ServiceNotFound(service_name.to_string()))
    }
}
