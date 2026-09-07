use crate::auth::{AuthStatus, AuthStatusView, AuthorizationStart};
use crate::config::ScopeDescriptor;
use crate::store::prelude::*;
use serde_json::Value;

impl MCPStore {
    pub async fn add_service(&self, service_name: &str, config: ServerConfig) -> Result<String> {
        self.kernel
            .control
            .add_service(self, service_name, config)
            .await
    }

    pub async fn remove_service(&self, service_name: &str) -> Result<String> {
        self.kernel.control.remove_service(self, service_name).await
    }

    pub async fn update_service(&self, service_name: &str, config: ServerConfig) -> Result<String> {
        self.kernel
            .control
            .update_service(self, service_name, config)
            .await
    }

    pub async fn patch_service(&self, service_name: &str, updates: Value) -> Result<String> {
        self.kernel
            .control
            .patch_service(self, service_name, updates)
            .await
    }

    pub async fn declare_service_scope(
        &self,
        service_name: &str,
        scope: &ScopeRef,
        descriptor: ScopeDescriptor,
    ) -> Result<InstanceId> {
        self.kernel
            .control
            .declare_service_scope(self, service_name, scope, descriptor)
            .await
    }

    pub async fn remove_service_scope(
        &self,
        service_name: &str,
        scope: &ScopeRef,
    ) -> Result<String> {
        self.kernel
            .control
            .remove_service_scope(self, service_name, scope)
            .await
    }

    pub async fn connect_service(&self, instance_id: InstanceId) -> Result<String> {
        self.kernel.control.connect_service(self, instance_id).await
    }

    pub async fn disconnect_service(&self, instance_id: InstanceId) -> Result<String> {
        self.kernel
            .control
            .disconnect_service(self, instance_id)
            .await
    }

    pub async fn restart_service(&self, instance_id: InstanceId) -> Result<String> {
        self.kernel.control.restart_service(self, instance_id).await
    }

    pub async fn reset_config(&self) -> Result<String> {
        self.kernel.control.reset_config(self).await
    }

    pub async fn reset_scope(&self, scope: &ScopeRef) -> Result<String> {
        self.kernel.control.reset_scope(self, scope).await
    }

    pub(crate) async fn ensure_http_oauth_config(&self, instance_id: InstanceId) -> Result<()> {
        self.kernel
            .control
            .ensure_http_oauth_config(self, instance_id)
            .await
    }

    pub(crate) async fn record_failure(
        &self,
        instance_id: InstanceId,
        error: &Error,
    ) -> Result<()> {
        self.kernel
            .control
            .record_failure(self, instance_id, error)
            .await
    }
    pub async fn auth_status(&self, instance_id: InstanceId) -> AuthStatus {
        self.kernel.control.auth_status(self, instance_id).await
    }

    pub async fn auth_status_view(&self, instance_id: InstanceId) -> Result<AuthStatusView> {
        self.kernel
            .control
            .auth_status_view(self, instance_id)
            .await
    }

    pub async fn begin_authorization(&self, instance_id: InstanceId) -> Result<AuthorizationStart> {
        self.kernel
            .control
            .begin_authorization(self, instance_id)
            .await
    }

    pub async fn authorization_callback_uri(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<String>> {
        self.kernel
            .control
            .authorization_callback_uri(self, instance_id)
            .await
    }

    pub async fn complete_authorization(
        &self,
        instance_id: InstanceId,
        callback_url: &str,
    ) -> Result<()> {
        self.kernel
            .control
            .complete_authorization(self, instance_id, callback_url)
            .await
    }

    pub async fn complete_authorization_callback(
        &self,
        instance_id: InstanceId,
        code: &str,
        state: &str,
        issuer: Option<&str>,
    ) -> Result<()> {
        self.kernel
            .control
            .complete_authorization_callback(self, instance_id, code, state, issuer)
            .await
    }

    pub async fn refresh_authorization(&self, instance_id: InstanceId) -> Result<()> {
        self.kernel
            .control
            .refresh_authorization(self, instance_id)
            .await
    }

    pub async fn begin_scope_upgrade(
        &self,
        instance_id: InstanceId,
        required_scope: &str,
    ) -> Result<AuthorizationStart> {
        self.kernel
            .control
            .begin_scope_upgrade(self, instance_id, required_scope)
            .await
    }

    pub async fn save_oauth_client_secret(
        &self,
        instance_id: InstanceId,
        secret: String,
    ) -> Result<()> {
        self.kernel
            .control
            .save_oauth_client_secret(self, instance_id, secret)
            .await
    }

    pub async fn save_oauth_private_key(
        &self,
        instance_id: InstanceId,
        private_key: Vec<u8>,
    ) -> Result<()> {
        self.kernel
            .control
            .save_oauth_private_key(self, instance_id, private_key)
            .await
    }

    pub async fn logout_authorization(&self, instance_id: InstanceId) -> Result<()> {
        self.kernel
            .control
            .logout_authorization(self, instance_id)
            .await
    }
}
