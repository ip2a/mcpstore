use crate::store::prelude::*;

impl MCPStore {
    pub async fn disconnect_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceDisconnectRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.pool.disconnect(name).await?;
        self.registry
            .update_status(name, ConnectionStatus::Disconnected)
            .await;
        self.set_service_status(name, HealthStatus::Disconnected, None, Vec::new())
            .await?;
        let lifecycle = self.resolved_service_lifecycle(name).await?;
        self.update_lifecycle_state(name, |state| {
            state.manually_stopped = true;
            state.manually_stopped_at = Some(Self::now_timestamp());
            state.manual_stop_persistent = lifecycle.restart_policy.is_unless_stopped();
        })
        .await?;
        self.event_bus
            .publish(
                Event::new("SERVICE_DISCONNECTED", serde_json::json!({ "name": name })),
                true,
            )
            .await;
        tracing::info!("[STORE] Service disconnected: {}", name);
        Ok(())
    }

    pub async fn restart_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceRestartRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.pool.disconnect(name).await.ok();
        self.registry
            .update_status(name, ConnectionStatus::Disconnected)
            .await;
        self.set_service_status(name, HealthStatus::Disconnected, None, Vec::new())
            .await?;
        self.clear_lifecycle_manual_stop(name).await?;
        self.connect_service(name).await
    }
}
