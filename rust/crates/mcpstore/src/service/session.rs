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
        if self.is_openapi_virtual_instance(instance_id).await? {
            self.applied_openapi_configs
                .write()
                .await
                .remove(&instance_id);
        } else {
            self.pool.disconnect(instance_id).await?;
        }
        self.registry
            .update_status(instance_id, ConnectionStatus::Disconnected)
            .await;
        self.set_instance_status(instance_id, HealthStatus::Disconnected, None, Vec::new())
            .await?;
        let lifecycle = self.resolved_instance_lifecycle(instance_id).await?;
        self.update_lifecycle_state(instance_id, |state| {
            state.manually_stopped = true;
            state.manually_stopped_at = Some(Self::now_timestamp());
            state.manual_stop_persistent = lifecycle.restart_policy.is_unless_stopped();
        })
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
        if self.is_openapi_virtual_instance(instance_id).await? {
            self.applied_openapi_configs
                .write()
                .await
                .remove(&instance_id);
        } else {
            self.pool.disconnect(instance_id).await.ok();
        }
        self.registry
            .update_status(instance_id, ConnectionStatus::Disconnected)
            .await;
        self.set_instance_status(instance_id, HealthStatus::Disconnected, None, Vec::new())
            .await?;
        self.clear_lifecycle_manual_stop(instance_id).await?;
        self.connect_service(instance_id).await
    }
}
