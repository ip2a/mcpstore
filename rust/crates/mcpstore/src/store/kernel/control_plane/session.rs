use crate::state::{FailureInfo, FailurePhase, ServiceStateEvent};
use crate::store::prelude::*;
use crate::store::{ControlPlane, MCPStore};

impl ControlPlane {
    pub async fn disconnect_service(
        &self,
        store: &MCPStore,
        instance_id: InstanceId,
    ) -> Result<String> {
        if store.is_data_plane() {
            return store
                .queue_control_request(
                    "ServiceDisconnectRequested",
                    serde_json::json!({ "instance_id": instance_id }),
                )
                .await;
        }

        let instance = store
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        store
            .kernel
            .control
            .state
            .dispatch(
                instance_id,
                ServiceStateEvent::StopRequested,
                MCPStore::now_timestamp(),
            )
            .await?;

        let stop_result = if store.is_openapi_virtual_instance(instance_id).await? {
            store
                .kernel
                .runtime
                .applied_openapi_configs
                .write()
                .await
                .remove(&instance_id);
            Ok(String::new())
        } else {
            store
                .kernel
                .execution
                .pool
                .disconnect(instance_id)
                .await
                .map(|_| String::new())
                .map_err(Error::from)
        };
        if let Err(error) = stop_result {
            store
                .kernel
                .control
                .state
                .dispatch(
                    instance_id,
                    ServiceStateEvent::StopFailed(FailureInfo {
                        phase: FailurePhase::Transport,
                        code: crate::error::FailureCode::StopFailed,
                        retryable: true,
                        message: error.to_string(),
                        since: MCPStore::now_timestamp(),
                    }),
                    MCPStore::now_timestamp(),
                )
                .await?;
            return Err(error);
        }
        store
            .kernel
            .control
            .state
            .dispatch(
                instance_id,
                ServiceStateEvent::TransportStopped,
                MCPStore::now_timestamp(),
            )
            .await?;
        store
            .kernel
            .execution
            .event_bus
            .publish(
                Event::new(
                    "SERVICE_DISCONNECTED",
                    serde_json::json!({
                        "instance_id": instance_id,
                        "service_name": instance.service_name,
                        "scope": instance.scope,
                    }),
                ),
                true,
            )
            .await;
        tracing::info!(
            "[STORE] Service instance disconnected: {} (service={})",
            instance_id,
            instance.service_name
        );
        Ok(String::new())
    }

    pub async fn restart_service(
        &self,
        store: &MCPStore,
        instance_id: InstanceId,
    ) -> Result<String> {
        if store.is_data_plane() {
            return store
                .queue_control_request(
                    "ServiceRestartRequested",
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
        self.disconnect_service(store, instance_id).await?;
        store
            .connect_service_internal(instance_id, false)
            .await
            .map(|_| String::new())
    }
}
