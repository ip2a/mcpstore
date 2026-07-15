use crate::store::prelude::*;
use crate::transport::{McpElicitationSession, McpElicitationSessionOptions};

impl MCPStore {
    pub async fn open_elicitation_session(
        &self,
        instance_id: InstanceId,
        options: McpElicitationSessionOptions,
    ) -> Result<Option<McpElicitationSession>> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }
        if self.is_openapi_virtual_instance(instance_id).await? {
            return Ok(None);
        }
        self.ensure_instance_connected(instance_id).await?;
        self.pool
            .open_elicitation_session(instance_id, options)
            .await
            .map_err(StoreError::Transport)
    }
}
