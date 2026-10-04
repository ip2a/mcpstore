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
        // 缺则补：只在实例实际缺席时注水一次，循环调用方不再 N+1 全量重建。
        self.load_from_db().await?;
        self.kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))
    }
}
