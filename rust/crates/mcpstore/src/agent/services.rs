use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_services_scoped(&self, scope: &ScopeRef) -> Result<Vec<serde_json::Value>> {
        self.list_scope_instances(scope)
            .await?
            .into_iter()
            .map(|instance| {
                let tool_count = instance.tools.len();
                let mut value = serde_json::to_value(instance)
                    .map_err(|error| StoreError::Other(error.to_string()))?;
                if let serde_json::Value::Object(object) = &mut value {
                    object.insert("tool_count".to_string(), serde_json::json!(tool_count));
                }
                Ok(value)
            })
            .collect()
    }

    pub async fn list_service_entries_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<ScopedServiceEntry>> {
        Ok(self
            .list_scope_instances(scope)
            .await?
            .into_iter()
            .map(|instance| ScopedServiceEntry {
                tool_count: instance.tools.len(),
                instance,
            })
            .collect())
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
                "mcp".to_string(),
                serde_json::to_value(self.mcp_server_metadata(instance_id).await?)
                    .map_err(|error| StoreError::Other(error.to_string()))?,
            );
        }
        Ok(value)
    }
}
