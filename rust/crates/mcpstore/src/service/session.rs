use crate::state::{FailureInfo, FailurePhase, ServiceStateEvent};
use crate::store::prelude::*;

impl MCPStore {
    pub async fn disconnect_service(&self, instance_id: InstanceId) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceDisconnectRequested",
                    serde_json::json!({ "instance_id": instance_id }),
                )
                .await;
        }

        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        self.state_manager
            .dispatch(
                instance_id,
                ServiceStateEvent::StopRequested,
                Self::now_timestamp(),
            )
            .await?;

        let stop_result = if self.is_openapi_virtual_instance(instance_id).await? {
            self.applied_openapi_configs
                .write()
                .await
                .remove(&instance_id);
            Ok(())
        } else {
            self.pool
                .disconnect(instance_id)
                .await
                .map_err(StoreError::from)
        };
        if let Err(error) = stop_result {
            self.state_manager
                .dispatch(
                    instance_id,
                    ServiceStateEvent::StopFailed(FailureInfo {
                        phase: FailurePhase::Transport,
                        code: "transport_stop_failed".to_string(),
                        retryable: true,
                        message: error.to_string(),
                        since: Self::now_timestamp(),
                    }),
                    Self::now_timestamp(),
                )
                .await?;
            return Err(error);
        }
        self.state_manager
            .dispatch(
                instance_id,
                ServiceStateEvent::TransportStopped,
                Self::now_timestamp(),
            )
            .await?;
        self.event_bus
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
        Ok(())
    }

    pub async fn restart_service(&self, instance_id: InstanceId) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceRestartRequested",
                    serde_json::json!({ "instance_id": instance_id }),
                )
                .await;
        }

        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }
        self.disconnect_service(instance_id).await?;
        self.connect_service_internal(instance_id, false).await
    }
}
