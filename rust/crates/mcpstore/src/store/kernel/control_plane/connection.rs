use crate::store::prelude::*;
use crate::store::{ControlPlane, MCPStore};

impl ControlPlane {
    pub async fn connect_service(&self, store: &MCPStore, instance_id: InstanceId) -> Result<()> {
        if store
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .is_none()
        {
            return Err(Error::new(
                FailureCode::ServiceNotFound,
                instance_id.to_string(),
            ));
        }
        store.connect_service_internal(instance_id, false).await
    }
}
