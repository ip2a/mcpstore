use crate::cache::models::{HealthStatus, ToolAvailability};
use crate::config::ServerConfig;
use crate::core::{Result, StoreError};
use crate::identity::InstanceId;
use crate::registry::ConnectionStatus;
use crate::store::MCPStore;
use crate::transport::TransportError;

use super::{
    AuthRequired, AuthStatus, AuthStatusView, AuthorizationStart, ClientSecret, PrivateKey,
};

impl MCPStore {
    pub async fn auth_status(&self, instance_id: InstanceId) -> AuthStatus {
        self.auth_coordinator.status(instance_id).await
    }

    pub async fn auth_status_view(&self, instance_id: InstanceId) -> Result<AuthStatusView> {
        let config = self.instance_auth_transport_config(instance_id).await?;
        self.auth_coordinator
            .initialize_status(instance_id, &config.auth)
            .await;
        Ok(self
            .auth_coordinator
            .status_view(instance_id, &config.auth)
            .await)
    }

    pub async fn begin_authorization(&self, instance_id: InstanceId) -> Result<AuthorizationStart> {
        let config = self.instance_auth_transport_config(instance_id).await?;
        let base_url = required_auth_base_url(instance_id, &config)?;
        self.auth_coordinator
            .begin_authorization(instance_id, base_url, &config.auth)
            .await
            .map_err(Into::into)
    }

    pub async fn authorization_callback_uri(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<String>> {
        let config = self.instance_auth_transport_config(instance_id).await?;
        match config.auth {
            super::AuthConfig::OAuthAuthorizationCode(config) => Ok(Some(config.redirect_uri)),
            super::AuthConfig::OAuthClientCredentials(_) | super::AuthConfig::None => Ok(None),
        }
    }

    pub async fn complete_authorization(
        &self,
        instance_id: InstanceId,
        callback_url: &str,
    ) -> Result<()> {
        self.ensure_instance_exists(instance_id).await?;
        self.auth_coordinator
            .complete_authorization(instance_id, callback_url)
            .await
            .map_err(Into::into)
    }

    pub async fn complete_authorization_callback(
        &self,
        instance_id: InstanceId,
        code: &str,
        state: &str,
        issuer: Option<&str>,
    ) -> Result<()> {
        self.ensure_instance_exists(instance_id).await?;
        self.auth_coordinator
            .complete_authorization_callback(instance_id, code, state, issuer)
            .await
            .map_err(Into::into)
    }

    pub async fn refresh_authorization(&self, instance_id: InstanceId) -> Result<()> {
        let config = self.instance_auth_transport_config(instance_id).await?;
        let base_url = required_auth_base_url(instance_id, &config)?;
        self.auth_coordinator
            .refresh(instance_id, base_url, &config.auth)
            .await
            .map_err(Into::into)
    }

    pub async fn begin_scope_upgrade(
        &self,
        instance_id: InstanceId,
        required_scope: &str,
    ) -> Result<AuthorizationStart> {
        if required_scope.trim().is_empty() {
            return Err(StoreError::Auth(super::AuthError::InvalidConfig(
                "required_scope must not be empty".to_string(),
            )));
        }
        let config = self.instance_auth_transport_config(instance_id).await?;
        let base_url = required_auth_base_url(instance_id, &config)?;
        self.auth_coordinator
            .begin_scope_upgrade(instance_id, base_url, &config.auth, required_scope)
            .await
            .map_err(Into::into)
    }

    pub async fn save_oauth_client_secret(
        &self,
        instance_id: InstanceId,
        secret: String,
    ) -> Result<()> {
        if secret.is_empty() {
            return Err(StoreError::Auth(super::AuthError::InvalidConfig(
                "client secret must not be empty".to_string(),
            )));
        }
        let config = self.instance_auth_transport_config(instance_id).await?;
        let base_url = required_auth_base_url(instance_id, &config)?;
        self.auth_coordinator
            .save_client_secret(
                instance_id,
                base_url,
                &config.auth,
                ClientSecret::new(secret),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn save_oauth_private_key(
        &self,
        instance_id: InstanceId,
        private_key: Vec<u8>,
    ) -> Result<()> {
        if private_key.is_empty() {
            return Err(StoreError::Auth(super::AuthError::InvalidConfig(
                "private key must not be empty".to_string(),
            )));
        }
        let config = self.instance_auth_transport_config(instance_id).await?;
        let base_url = required_auth_base_url(instance_id, &config)?;
        self.auth_coordinator
            .save_private_key(
                instance_id,
                base_url,
                &config.auth,
                PrivateKey::new(private_key),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn logout_authorization(&self, instance_id: InstanceId) -> Result<()> {
        let config = self.instance_auth_transport_config(instance_id).await?;
        let base_url = required_auth_base_url(instance_id, &config)?;
        self.pool.disconnect(instance_id).await.ok();
        self.auth_coordinator
            .logout(instance_id, base_url, &config.auth)
            .await?;
        self.registry
            .update_status(instance_id, ConnectionStatus::Disconnected)
            .await;
        self.set_instance_status(instance_id, HealthStatus::Disconnected, None, Vec::new())
            .await?;
        Ok(())
    }

    async fn ensure_instance_exists(&self, instance_id: InstanceId) -> Result<()> {
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }
        Ok(())
    }

    async fn instance_auth_transport_config(
        &self,
        instance_id: InstanceId,
    ) -> Result<ServerConfig> {
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        serde_json::from_value(serde_json::Value::Object(instance.effective_config)).map_err(|_| {
            StoreError::Auth(super::AuthError::InvalidConfig(
                "effective service authentication config cannot be decoded".to_string(),
            ))
        })
    }

    pub(crate) async fn record_transport_failure(
        &self,
        instance_id: InstanceId,
        error: &TransportError,
        context: &str,
    ) -> Result<()> {
        match error {
            TransportError::AuthRequired(required) => {
                return self
                    .mark_instance_auth_required(instance_id, required)
                    .await;
            }
            TransportError::InsufficientScope {
                instance_id: error_instance_id,
                required_scope,
            } => {
                return self
                    .mark_instance_scope_upgrade_required(
                        instance_id,
                        *error_instance_id,
                        required_scope.as_deref(),
                    )
                    .await;
            }
            _ => {}
        }

        self.registry
            .update_status(instance_id, ConnectionStatus::Error)
            .await;
        self.record_instance_failure(instance_id, format!("{context}: {error}"))
            .await?;
        Ok(())
    }

    async fn mark_instance_scope_upgrade_required(
        &self,
        instance_id: InstanceId,
        error_instance_id: InstanceId,
        required_scope: Option<&str>,
    ) -> Result<()> {
        if error_instance_id != instance_id {
            return Err(StoreError::Other(format!(
                "Scope requirement instance mismatch: expected {instance_id}, received {error_instance_id}"
            )));
        }
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;

        self.auth_coordinator
            .mark_scope_upgrade_required(instance_id, required_scope)
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
                    "AUTH_SCOPE_UPGRADE_REQUIRED",
                    serde_json::json!({
                        "instance_id": instance_id,
                        "service_name": instance.service_name,
                        "scope": instance.scope,
                        "required_scope": required_scope,
                    }),
                ),
                true,
            )
            .await;
        Ok(())
    }

    async fn mark_instance_auth_required(
        &self,
        instance_id: InstanceId,
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

fn required_auth_base_url(instance_id: InstanceId, config: &ServerConfig) -> Result<&str> {
    config.url.as_deref().ok_or_else(|| {
        StoreError::Auth(super::AuthError::InvalidConfig(format!(
            "OAuth service instance {instance_id} requires a URL"
        )))
    })
}
