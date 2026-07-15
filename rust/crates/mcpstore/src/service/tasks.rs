use crate::service::McpStoreToolExecutionHandle;
use crate::store::prelude::*;
use crate::transport::{
    McpExecutionOptions, McpTask, McpTaskRecord, McpToolExecution, TransportError,
};

impl MCPStore {
    pub async fn start_task_execution(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
        ttl: Option<u64>,
        options: McpExecutionOptions,
    ) -> Result<McpStoreToolExecutionHandle<'_>> {
        self.start_task_tool_execution(instance_id, tool_name, args, ttl, options)
            .await
    }

    pub async fn call_task_tool(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
        ttl: Option<u64>,
    ) -> Result<McpToolExecution> {
        self.start_task_execution(
            instance_id,
            tool_name,
            args,
            ttl,
            McpExecutionOptions::default(),
        )
        .await?
        .wait()
        .await
    }

    pub async fn list_tasks(&self, instance_id: InstanceId) -> Result<Vec<McpTask>> {
        self.ensure_task_instance_connected(instance_id).await?;
        self.pool
            .list_tasks(instance_id)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn get_task(&self, instance_id: InstanceId, task_id: &str) -> Result<McpTask> {
        self.ensure_task_instance_connected(instance_id).await?;
        self.pool
            .get_task(instance_id, task_id)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn get_task_result(
        &self,
        instance_id: InstanceId,
        task_id: &str,
    ) -> Result<serde_json::Value> {
        self.ensure_task_instance_connected(instance_id).await?;
        self.pool
            .get_task_result(instance_id, task_id)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn cancel_task(&self, instance_id: InstanceId, task_id: &str) -> Result<McpTask> {
        self.ensure_task_instance_connected(instance_id).await?;
        self.pool
            .cancel_task(instance_id, task_id)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn list_task_records(&self, instance_id: InstanceId) -> Result<Vec<McpTaskRecord>> {
        self.require_task_instance(instance_id).await?;
        self.pool
            .list_task_records(instance_id)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn get_task_record(
        &self,
        instance_id: InstanceId,
        task_id: &str,
    ) -> Result<Option<McpTaskRecord>> {
        self.require_task_instance(instance_id).await?;
        self.pool
            .get_task_record(instance_id, task_id)
            .await
            .map_err(StoreError::Transport)
    }

    async fn ensure_task_instance_connected(&self, instance_id: InstanceId) -> Result<()> {
        self.require_task_instance(instance_id).await?;
        self.ensure_instance_connected(instance_id).await
    }

    async fn require_task_instance(&self, instance_id: InstanceId) -> Result<()> {
        self.require_instance(instance_id).await?;
        if self.is_openapi_virtual_instance(instance_id).await? {
            return Err(StoreError::Transport(
                TransportError::CapabilityUnsupported {
                    instance_id,
                    capability: "tasks",
                },
            ));
        }
        Ok(())
    }
}
