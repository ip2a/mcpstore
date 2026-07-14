use crate::store::prelude::*;
use crate::transport::{McpCompletion, McpCompletionRequest, McpLoggingLevel, McpServerMetadata};

impl MCPStore {
    pub async fn mcp_server_metadata(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<McpServerMetadata>> {
        self.require_instance(instance_id).await?;
        self.pool
            .server_metadata(instance_id)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn complete_mcp_argument(
        &self,
        instance_id: InstanceId,
        request: McpCompletionRequest,
    ) -> Result<McpCompletion> {
        self.ensure_instance_connected(instance_id).await?;
        self.pool
            .complete(instance_id, request)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn subscribe_resource_updates(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<()> {
        self.ensure_instance_connected(instance_id).await?;
        self.pool
            .subscribe_resource(instance_id, uri)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn unsubscribe_resource_updates(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<()> {
        self.ensure_instance_connected(instance_id).await?;
        self.pool
            .unsubscribe_resource(instance_id, uri)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn set_mcp_logging_level(
        &self,
        instance_id: InstanceId,
        level: McpLoggingLevel,
    ) -> Result<()> {
        self.ensure_instance_connected(instance_id).await?;
        self.pool
            .set_logging_level(instance_id, level)
            .await
            .map_err(StoreError::Transport)
    }
}
