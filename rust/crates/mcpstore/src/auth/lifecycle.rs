use crate::cache::models::{HealthStatus, ToolAvailability};
use crate::core::{Result, StoreError};
use crate::registry::ConnectionStatus;
use crate::store::MCPStore;
use crate::transport::TransportError;

use super::{AuthRequired, AuthStatus};

impl MCPStore {
    pub async fn auth_status(&self, instance_id: crate::identity::InstanceId) -> AuthStatus {
        self.auth_coordinator.status(instance_id).await
    }

    pub(crate) async fn record_transport_failure(
        &self,
        instance_id: crate::identity::InstanceId,
        error: &TransportError,
        context: &str,
    ) -> Result<()> {
        if let TransportError::AuthRequired(required) = error {
            return self
                .mark_instance_auth_required(instance_id, required)
                .await;
        }

        self.registry
            .update_status(instance_id, ConnectionStatus::Error)
            .await;
        self.mark_instance_retryable_failure(instance_id, format!("{context}: {error}"))
            .await?;
        Ok(())
    }

    async fn mark_instance_auth_required(
        &self,
        instance_id: crate::identity::InstanceId,
        required: &AuthRequired,
    ) -> Result<()> {
        if required.instance_id != instance_id {
            return Err(StoreError::Other(format!(
                "Authentication requirement instance mismatch: expected {instance_id}, received {}",
                required.instance_id
            )));
        }
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;

        self.auth_coordinator
            .set_status(instance_id, AuthStatus::Unauthenticated)
            .await;
        self.registry
            .update_status(instance_id, ConnectionStatus::Disconnected)
            .await;

        let mut status = self.load_or_default_status(instance_id).await?;
        status.health_status = HealthStatus::Disconnected;
        status.last_health_check = Self::now_timestamp();
        status.connection_attempts = 0;
        status.current_error = None;
        status.window_error_rate = None;
        status.next_retry_time = None;
        status.hard_deadline = None;
        status.lease_deadline = None;
        status.lifecycle_state.restart_attempts = 0;
        status.tools = self
            .tool_statuses_with_availability(instance_id, ToolAvailability::Unavailable)
            .await?;
        self.put_instance_status(&status).await?;

        self.event_bus
            .publish(
                crate::events::Event::new(
                    "AUTH_REQUIRED",
                    serde_json::json!({
                        "instance_id": instance_id,
                        "service_name": instance.service_name,
                        "scope": instance.scope,
                        "flow": required.flow,
                        "scopes": required.scopes,
                    }),
                ),
                true,
            )
            .await;
        Ok(())
    }
}
