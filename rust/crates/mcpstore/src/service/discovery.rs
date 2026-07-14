use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn ensure_instance_connected(&self, instance_id: InstanceId) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }
        if self.is_openapi_virtual_instance(instance_id).await? {
            let instance = self
                .registry
                .find_instance(instance_id)
                .await
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
            if instance.status == ConnectionStatus::Connected {
                return Ok(());
            }
            self.ensure_service_auto_start_allowed(instance_id).await?;
            self.connect_service_internal(instance_id, true).await?;
            return Ok(());
        }
        if !self.pool.is_connected(instance_id).await {
            self.ensure_service_auto_start_allowed(instance_id).await?;
            self.connect_service_internal(instance_id, true).await?;
        }
        Ok(())
    }

    pub(crate) async fn is_openapi_virtual_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<bool> {
        let Some(instance) = self.registry.find_instance(instance_id).await else {
            return Ok(false);
        };
        Ok(instance.transport == "openapi")
    }

    pub async fn list_instances(&self) -> Vec<ServiceInstance> {
        self.refresh_from_db_if_needed().await.ok();
        let mut instances = self.registry.list_instances().await;
        if self.source_mode == SourceMode::Db {
            for instance in &mut instances {
                self.hydrate_instance_status_from_cache(instance).await.ok();
            }
        }
        instances
    }

    pub async fn find_instance(&self, instance_id: InstanceId) -> Option<ServiceInstance> {
        self.refresh_from_db_if_needed().await.ok();
        let mut instance = self.registry.find_instance(instance_id).await?;
        if self.source_mode == SourceMode::Db {
            self.hydrate_instance_status_from_cache(&mut instance)
                .await
                .ok();
        }
        Some(instance)
    }

    pub async fn list_tools(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<crate::registry::ToolInfo>> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }
        Ok(self.registry.list_instance_tools(instance_id).await)
    }

    pub async fn list_all_tools(&self) -> Vec<(InstanceId, crate::registry::ToolInfo)> {
        self.refresh_from_db_if_needed().await.ok();
        self.registry.list_all_tools().await
    }

    pub async fn list_agents(&self) -> Result<Vec<serde_json::Value>> {
        self.refresh_from_db_if_needed().await?;
        let mut agent_ids = self.registry.list_agent_ids().await;
        agent_ids.sort();

        let mut agents = Vec::with_capacity(agent_ids.len());
        for agent_id in agent_ids {
            agents.push(serde_json::json!({
                "agent_id": agent_id,
                "instance_ids": self.registry.list_agent_instance_ids(&agent_id).await,
            }));
        }
        Ok(agents)
    }
}
