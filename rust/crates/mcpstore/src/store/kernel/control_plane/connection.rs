use crate::store::prelude::*;
use crate::store::{ControlPlane, MCPStore};

impl ControlPlane {
    pub async fn connect_service(
        &self,
        store: &MCPStore,
        instance_id: InstanceId,
    ) -> Result<String> {
        if store.is_data_plane() {
            return store
                .queue_control_request(
                    "ServiceConnectRequested",
                    serde_json::json!({ "instance_id": instance_id }),
                )
                .await;
        }
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
        store
            .connect_service_internal(instance_id, false)
            .await
            .map(|_| String::new())
    }
}
