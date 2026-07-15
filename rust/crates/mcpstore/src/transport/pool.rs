use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::RwLock;

use crate::cache::CacheLayerManager;

use crate::auth::AuthCoordinator;
use crate::config::ServerConfig;
use crate::events::EventBus;
use crate::identity::InstanceId;
use crate::registry::ServiceRegistry;
use crate::transport::client::McpConnection;
use crate::transport::{
    DiscoveredPrompt, DiscoveredResource, DiscoveredResourceTemplate, DiscoveredTool,
    McpCompletion, McpCompletionRequest, McpExecutionOptions, McpLoggingLevel, McpServerMetadata,
    McpTask, McpTaskRecord, McpToolExecution, McpToolExecutionHandle, Result, TaskStateStore,
    ToolCallResult, TransportError,
};

pub struct ConnectionPool {
    connections: Arc<RwLock<HashMap<InstanceId, McpConnection>>>,
    auth_coordinator: AuthCoordinator,
    registry: ServiceRegistry,
    event_bus: EventBus,
    task_state: TaskStateStore,
    task_worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl ConnectionPool {
    pub fn new(
        auth_coordinator: AuthCoordinator,
        registry: ServiceRegistry,
        event_bus: EventBus,
        cache: Arc<CacheLayerManager>,
    ) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            auth_coordinator,
            registry,
            event_bus,
            task_state: TaskStateStore::new(cache),
            task_worker: Mutex::new(None),
        }
    }

    pub async fn add(&self, instance_id: InstanceId, config: ServerConfig) {
        let conn = McpConnection::new(
            instance_id,
            instance_id.to_string(),
            config,
            self.auth_coordinator.clone(),
            self.registry.clone(),
            self.event_bus.clone(),
        );
        self.connections.write().await.insert(instance_id, conn);
        self.start_task_worker();
    }

    pub async fn connect(&self, instance_id: InstanceId) -> Result<()> {
        let connected = {
            let mut conns = self.connections.write().await;
            let conn = conns.get_mut(&instance_id).ok_or_else(|| {
                TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
            })?;
            if conn.is_connected() {
                true
            } else {
                conn.connect().await?;
                false
            }
        };
        if !connected {
            self.recover_tasks(instance_id).await;
        }
        Ok(())
    }

    pub async fn disconnect(&self, instance_id: InstanceId) -> Result<()> {
        let mut conns = self.connections.write().await;
        if let Some(conn) = conns.get_mut(&instance_id) {
            conn.disconnect().await?;
        }
        drop(conns);
        self.mark_tasks_disconnected(instance_id, "connection closed")
            .await
    }

    pub async fn remove(&self, instance_id: InstanceId) -> Result<()> {
        let mut conns = self.connections.write().await;
        if let Some(mut conn) = conns.remove(&instance_id) {
            conn.disconnect().await.ok();
        }
        drop(conns);
        self.mark_tasks_disconnected(instance_id, "connection removed")
            .await
    }

    pub async fn clear(&self) {
        let instance_ids: Vec<InstanceId> = self.connections.read().await.keys().copied().collect();
        for instance_id in instance_ids {
            self.remove(instance_id).await.ok();
        }
    }

    pub async fn start_task_tool_execution(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
        ttl: Option<u64>,
        options: McpExecutionOptions,
    ) -> Result<McpToolExecutionHandle> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.start_tool_task(tool_name, args, ttl, options).await
    }

    pub async fn call_tool_task(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
        ttl: Option<u64>,
    ) -> Result<McpToolExecution> {
        let execution = self
            .start_task_tool_execution(
                instance_id,
                tool_name,
                args,
                ttl,
                McpExecutionOptions::default(),
            )
            .await?
            .wait()
            .await?;
        if let McpToolExecution::Task { task } = &execution {
            self.observe_task(instance_id, task.clone(), Some(tool_name))
                .await?;
        }
        Ok(execution)
    }

    pub async fn list_tasks(&self, instance_id: InstanceId) -> Result<Vec<McpTask>> {
        let tasks = {
            let conns = self.connections.read().await;
            let conn = conns.get(&instance_id).ok_or_else(|| {
                TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
            })?;
            conn.list_tasks().await?
        };
        for task in &tasks {
            self.observe_task(instance_id, task.clone(), None).await?;
        }
        Ok(tasks)
    }

    pub async fn get_task(&self, instance_id: InstanceId, task_id: &str) -> Result<McpTask> {
        let task = {
            let conns = self.connections.read().await;
            let conn = conns.get(&instance_id).ok_or_else(|| {
                TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
            })?;
            conn.get_task(task_id).await?
        };
        self.observe_task(instance_id, task.clone(), None).await?;
        Ok(task)
    }

    pub async fn get_task_result(
        &self,
        instance_id: InstanceId,
        task_id: &str,
    ) -> Result<serde_json::Value> {
        let result = {
            let conns = self.connections.read().await;
            let conn = conns.get(&instance_id).ok_or_else(|| {
                TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
            })?;
            conn.get_task_result(task_id).await
        };
        if let Err(error) = &result {
            self.record_task_error(instance_id, task_id, error).await;
        }
        result
    }

    pub async fn cancel_task(&self, instance_id: InstanceId, task_id: &str) -> Result<McpTask> {
        let task = {
            let conns = self.connections.read().await;
            let conn = conns.get(&instance_id).ok_or_else(|| {
                TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
            })?;
            conn.cancel_task(task_id).await?
        };
        self.observe_task(instance_id, task.clone(), None).await?;
        Ok(task)
    }

    pub async fn list_task_records(&self, instance_id: InstanceId) -> Result<Vec<McpTaskRecord>> {
        self.task_state
            .list(instance_id)
            .await
            .map_err(|error| TransportError::TaskState(error.to_string()))
    }

    pub async fn get_task_record(
        &self,
        instance_id: InstanceId,
        task_id: &str,
    ) -> Result<Option<McpTaskRecord>> {
        self.task_state
            .get(instance_id, task_id)
            .await
            .map_err(|error| TransportError::TaskState(error.to_string()))
    }

    pub async fn list_tools(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredTool>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.list_tools().await
    }

    pub async fn open_elicitation_session(
        &self,
        instance_id: InstanceId,
        options: crate::transport::McpElicitationSessionOptions,
    ) -> Result<Option<crate::transport::McpElicitationSession>> {
        let conns = self.connections.read().await;
        let Some(conn) = conns.get(&instance_id) else {
            return Ok(None);
        };
        conn.open_elicitation_session(options).map(Some)
    }

    pub async fn start_tool_execution(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
        options: McpExecutionOptions,
    ) -> Result<McpToolExecutionHandle> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.start_tool_call(tool_name, args, options).await
    }

    pub async fn call_tool(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<ToolCallResult> {
        match self
            .start_tool_execution(instance_id, tool_name, args, McpExecutionOptions::default())
            .await?
            .wait()
            .await?
        {
            McpToolExecution::Immediate { result } => Ok(result),
            McpToolExecution::Task { .. } => Err(TransportError::Protocol(
                "tool call unexpectedly returned a task".to_string(),
            )),
        }
    }

    pub async fn list_resources(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredResource>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.list_resources().await
    }

    pub async fn list_resource_templates(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<DiscoveredResourceTemplate>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.list_resource_templates().await
    }

    pub async fn read_resource(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<serde_json::Value> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.read_resource(uri).await
    }

    pub async fn list_prompts(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredPrompt>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.list_prompts().await
    }

    pub async fn get_prompt(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.get_prompt(prompt_name, arguments).await
    }

    pub async fn server_metadata(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<McpServerMetadata>> {
        let conns = self.connections.read().await;
        let Some(conn) = conns.get(&instance_id) else {
            return Ok(None);
        };
        if !conn.is_connected() {
            return Ok(None);
        }
        conn.server_metadata().map(Some)
    }

    pub async fn complete(
        &self,
        instance_id: InstanceId,
        request: McpCompletionRequest,
    ) -> Result<McpCompletion> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.complete(request).await
    }

    pub async fn subscribe_resource(&self, instance_id: InstanceId, uri: &str) -> Result<()> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.subscribe_resource(uri).await
    }

    pub async fn unsubscribe_resource(&self, instance_id: InstanceId, uri: &str) -> Result<()> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.unsubscribe_resource(uri).await
    }

    pub async fn set_logging_level(
        &self,
        instance_id: InstanceId,
        level: McpLoggingLevel,
    ) -> Result<()> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            TransportError::NotConnected(format!("Service instance not found: {instance_id}"))
        })?;
        conn.set_logging_level(level).await
    }

    pub async fn is_connected(&self, instance_id: InstanceId) -> bool {
        let conns = self.connections.read().await;
        conns
            .get(&instance_id)
            .map(McpConnection::is_connected)
            .unwrap_or(false)
    }

    fn start_task_worker(&self) {
        let mut worker = self.task_worker.lock().expect("task worker lock poisoned");
        if worker.is_some() {
            return;
        }
        let task_state = self.task_state.clone();
        *worker = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                if let Err(error) = task_state.expire_due_tasks(chrono::Utc::now()).await {
                    tracing::warn!("[TASKS] failed to expire task records: {error}");
                }
            }
        }));
    }

    pub(crate) async fn observe_tool_task(
        &self,
        instance_id: InstanceId,
        task: McpTask,
        tool_name: Option<&str>,
    ) -> Result<McpTaskRecord> {
        self.observe_task(instance_id, task, tool_name).await
    }

    async fn observe_task(
        &self,
        instance_id: InstanceId,
        task: McpTask,
        tool_name: Option<&str>,
    ) -> Result<McpTaskRecord> {
        self.task_state
            .observe(instance_id, task, tool_name)
            .await
            .map_err(|error| TransportError::TaskState(error.to_string()))
    }

    async fn record_task_error(
        &self,
        instance_id: InstanceId,
        task_id: &str,
        error: &TransportError,
    ) {
        if let Err(state_error) = self
            .task_state
            .record_error(instance_id, task_id, error.to_string())
            .await
        {
            tracing::warn!("[TASKS] failed to persist task result error: {state_error}");
        }
    }

    async fn mark_tasks_disconnected(&self, instance_id: InstanceId, reason: &str) -> Result<()> {
        self.task_state
            .mark_disconnected(instance_id, reason)
            .await
            .map_err(|error| TransportError::TaskState(error.to_string()))
    }

    async fn recover_tasks(&self, instance_id: InstanceId) {
        let records = match self.task_state.list(instance_id).await {
            Ok(records) => records,
            Err(error) => {
                tracing::warn!("[TASKS] failed to load task records for recovery: {error}");
                return;
            }
        };
        for record in records {
            if record.task.status.is_terminal() {
                continue;
            }
            if let Err(error) = self.get_task(instance_id, &record.task_id).await {
                self.record_task_error(instance_id, &record.task_id, &error)
                    .await;
            }
        }
    }
}

impl Drop for ConnectionPool {
    fn drop(&mut self) {
        if let Ok(mut worker) = self.task_worker.lock() {
            if let Some(handle) = worker.take() {
                handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    use crate::cache::{memory_cache_store, CacheLayerManager};
    use crate::identity::{ScopeRef, ServiceInstanceKey};
    use crate::transport::{McpTask, McpTaskStatus};

    #[tokio::test]
    async fn task_worker_expires_due_records() {
        let cache = Arc::new(CacheLayerManager::new(memory_cache_store(), "task-worker"));
        let pool = ConnectionPool::new(
            AuthCoordinator::new().unwrap(),
            ServiceRegistry::new(),
            EventBus::with_history(1),
            cache,
        );
        let instance_id = ServiceInstanceKey::new("worker", ScopeRef::Store).instance_id();
        pool.add(instance_id, ServerConfig::default()).await;

        let timestamp = (Utc::now() - Duration::milliseconds(100)).to_rfc3339();
        pool.task_state
            .observe(
                instance_id,
                McpTask {
                    task_id: "task-1".to_string(),
                    status: McpTaskStatus::Working,
                    status_message: None,
                    created_at: timestamp.clone(),
                    last_updated_at: timestamp,
                    ttl: Some(10),
                    poll_interval: None,
                },
                None,
            )
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;

        let record = pool
            .task_state
            .get(instance_id, "task-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.task.status, McpTaskStatus::Expired);
    }
}
