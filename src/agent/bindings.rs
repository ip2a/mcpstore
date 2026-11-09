use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn require_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<ServiceInstance> {
        if let Some(instance) = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
        {
            return Ok(instance);
        }
        // Fill-if-missing: hydrate only when the instance is actually absent, so looping callers stop N+1 full rebuilds.
        self.load_from_db().await?;
        self.kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))
    }
}
