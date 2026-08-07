use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_services(&self) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_instances().await;
        let mut services = Vec::with_capacity(instances.len());
        for instance in instances {
            services.push(self.enrich_service(instance).await?);
        }
        Ok(services)
    }

    pub async fn list_services_scoped(&self, scope: &ScopeRef) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut services = Vec::with_capacity(instances.len());
        for instance in instances {
            services.push(self.enrich_service(instance).await?);
        }
        Ok(services)
    }

    async fn enrich_service(&self, instance: ServiceInstance) -> Result<serde_json::Value> {
        let tool_count = instance.tools.len();
        let state = self.service_state_entry(instance.instance_id).await?;
        let mut value =
            serde_json::to_value(instance).map_err(|error| StoreError::Other(error.to_string()))?;
        if let serde_json::Value::Object(object) = &mut value {
            object.insert("tool_count".to_string(), serde_json::json!(tool_count));
            object.insert(
                "state".to_string(),
                serde_json::to_value(state)
                    .map_err(|error| StoreError::Other(error.to_string()))?,
            );
        }
        Ok(value)
    }

    pub async fn list_service_entries_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<ScopedServiceEntry>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut services = Vec::with_capacity(instances.len());
        for instance in instances {
            let state = self.service_state_entry(instance.instance_id).await?;
            services.push(ScopedServiceEntry {
                tool_count: instance.tools.len(),
                instance,
                state,
            });
        }
        Ok(services)
    }

    pub async fn service_info_scoped(&self, instance_id: InstanceId) -> Result<serde_json::Value> {
        self.refresh_from_db_if_needed().await?;
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let tool_count = instance.tools.len();
        let mut value =
            serde_json::to_value(instance).map_err(|error| StoreError::Other(error.to_string()))?;
        if let serde_json::Value::Object(object) = &mut value {
            object.insert("tool_count".to_string(), serde_json::json!(tool_count));
            object.insert(
                "state".to_string(),
                serde_json::to_value(self.service_state_entry(instance_id).await?)
                    .map_err(|error| StoreError::Other(error.to_string()))?,
            );
            object.insert(
                "mcp".to_string(),
                serde_json::to_value(self.mcp_server_metadata(instance_id).await?)
                    .map_err(|error| StoreError::Other(error.to_string()))?,
            );
        }
        Ok(value)
    }

    /// 按读视图列服务：Root 聚合全部（`list_services`），Store/Agent 透传给 `list_services_scoped`。
    pub async fn list_services_viewed(&self, view: &ScopeView) -> Result<Vec<serde_json::Value>> {
        match view {
            ScopeView::Root => self.list_services().await,
            ScopeView::Store => self.list_services_scoped(&ScopeRef::Store).await,
            ScopeView::Agent { agent_id } => {
                self.list_services_scoped(&ScopeRef::Agent {
                    agent_id: agent_id.clone(),
                })
                .await
            }
        }
    }
}
