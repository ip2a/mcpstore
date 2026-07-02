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
                let original_name = request::optional_string(payload, "service_original_name")
                    .unwrap_or_else(|| service_name.clone());
                let agent_id = request::optional_string(payload, "agent_id")
                    .unwrap_or_else(|| GLOBAL_AGENT_STORE.to_string());
                let config = request::required_config(payload)?;
                self.add_service_with_identity(&service_name, &original_name, &agent_id, config)
                    .await?;
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
            "ServiceAssignRequested" => {
                let agent_id = request::required_string(payload, "agent_id")?;
                let service_name = request::required_string(payload, "service_name")?;
                self.assign_service_to_agent(&agent_id, &service_name)
                    .await?;
            }
            "ServiceUnassignRequested" => {
                let agent_id = request::required_string(payload, "agent_id")?;
                let service_name = request::required_string(payload, "service_name")?;
                self.unassign_service_from_agent(&agent_id, &service_name)
                    .await?;
            }
            "ServiceConnectRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                self.connect_service_internal(&service_name, false).await?;
            }
            "ServiceRefreshToolsRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                let force_refresh = payload
                    .get("force_refresh")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                self.refresh_service_tools_with_diff(&service_name, force_refresh)
                    .await?;
            }
            "ServiceDisconnectRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                self.disconnect_service(&service_name).await?;
            }
            "ServiceRestartRequested" => {
                let service_name = request::required_string(payload, "service_name")?;
                self.restart_service(&service_name).await?;
            }
            "StoreResetRequested" => {
                self.reset_config().await?;
            }
            "AgentResetRequested" => {
                let agent_id = request::required_string(payload, "agent_id")?;
                self.reset_agent_config(&agent_id).await?;
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
