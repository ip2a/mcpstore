use crate::control::request;
use crate::store::prelude::*;

impl MCPStore {
    pub(in crate::control) async fn apply_control_request(
        &self,
        event: &serde_json::Value,
    ) -> Result<()> {
        let request_type = event
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| StoreError::Other("Control request missing type".to_string()))?;
        let payload = event
            .get("payload")
            .ok_or_else(|| StoreError::Other("Control request missing payload".to_string()))?;

        match request_type {
            "ServiceAddRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                let config = request::required_config(payload)?;
                self.add_service(&service_name, config).await?;
            }
            "ServiceUpdateRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                self.update_service(&service_name, request::required_config(payload)?)
                    .await?;
            }
            "ServicePatchRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                let updates = payload.get("updates").cloned().ok_or_else(|| {
                    StoreError::Other("Control request missing updates".to_string())
                })?;
                self.patch_service(&service_name, updates).await?;
            }
            "ServiceRemoveRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                self.remove_service(&service_name).await?;
            }
            "ServiceScopeDeclareRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                let scope = payload
                    .get("scope")
                    .cloned()
                    .ok_or_else(|| StoreError::Other("Control request missing scope".to_string()))
                    .and_then(|value| {
                        serde_json::from_value::<ScopeRef>(value)
                            .map_err(|error| StoreError::Other(error.to_string()))
                    })?;
                let descriptor = payload
                    .get("descriptor")
                    .cloned()
                    .ok_or_else(|| {
                        StoreError::Other("Control request missing descriptor".to_string())
                    })
                    .and_then(|value| {
                        serde_json::from_value(value)
                            .map_err(|error| StoreError::Other(error.to_string()))
                    })?;
                self.declare_service_scope(&service_name, &scope, descriptor)
                    .await?;
            }
            "ServiceScopeRemoveRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                let scope = payload
                    .get("scope")
                    .cloned()
                    .ok_or_else(|| StoreError::Other("Control request missing scope".to_string()))
                    .and_then(|value| {
                        serde_json::from_value::<ScopeRef>(value)
                            .map_err(|error| StoreError::Other(error.to_string()))
                    })?;
                self.remove_service_scope(&service_name, &scope).await?;
            }
            "ServiceConnectRequested" => {
                let instance_id = request::required_string(payload, "instance_id")?
                    .parse::<InstanceId>()
                    .map_err(|error| StoreError::Other(format!("Invalid instance_id: {error}")))?;
                self.connect_service_internal(instance_id, false).await?;
            }
            "ServiceRefreshToolsRequested" => {
                let instance_id = request::required_string(payload, "instance_id")?
                    .parse::<InstanceId>()
                    .map_err(|error| StoreError::Other(format!("Invalid instance_id: {error}")))?;
                let force_refresh = payload
                    .get("force_refresh")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                self.refresh_service_tools_with_diff(instance_id, force_refresh)
                    .await?;
            }
            "ServiceDisconnectRequested" => {
                let instance_id = request::required_string(payload, "instance_id")?
                    .parse::<InstanceId>()
                    .map_err(|error| StoreError::Other(format!("Invalid instance_id: {error}")))?;
                self.disconnect_service(instance_id).await?;
            }
            "ServiceRestartRequested" => {
                let instance_id = request::required_string(payload, "instance_id")?
                    .parse::<InstanceId>()
                    .map_err(|error| StoreError::Other(format!("Invalid instance_id: {error}")))?;
                self.restart_service(instance_id).await?;
            }
            "StoreResetRequested" => {
                self.reset_config().await?;
            }
            "ScopeResetRequested" => {
                let scope = payload
                    .get("scope")
                    .cloned()
                    .ok_or_else(|| StoreError::Other("Control request missing scope".to_string()))
                    .and_then(|value| {
                        serde_json::from_value::<ScopeRef>(value)
                            .map_err(|error| StoreError::Other(error.to_string()))
                    })?;
                self.reset_scope(&scope).await?;
            }
            other => {
                return Err(StoreError::Other(format!(
                    "Unsupported control request type: {other}"
                )));
            }
        }
        Ok(())
    }
}
