use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn require_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<ServiceInstance> {
        self.refresh_from_db_if_needed().await?;
        self.registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))
    }
}
