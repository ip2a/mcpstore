use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio::sync::RwLock;

use crate::cache::CacheLayerManager;
use crate::health::supervisor::InstanceSupervisor;

use crate::auth::AuthCoordinator;
use crate::config::ServerConfig;
use crate::error::Result;
use crate::error::{Error, FailureCode};
use crate::events::EventBus;
use crate::identity::InstanceId;
use crate::registry::ServiceRegistry;
use crate::transport::client::McpConnection;
use crate::transport::{
    DiscoveredPrompt, DiscoveredResource, DiscoveredResourceTemplate, DiscoveredTool,
    McpCompletion, McpCompletionRequest, McpExecutionOptions, McpServerMetadata, McpTask,
    McpTaskRecord, McpToolExecution, McpToolExecutionHandle, TaskStateStore, ToolCallResult,
};

pub struct ConnectionPool {
    connections: Arc<RwLock<HashMap<InstanceId, McpConnection>>>,
    resource_subscriptions: Arc<RwLock<HashMap<InstanceId, HashSet<String>>>>,
    auth_coordinator: AuthCoordinator,
    registry: ServiceRegistry,
    event_bus: EventBus,
    task_state: TaskStateStore,
    supervisor: Arc<Mutex<Option<Arc<InstanceSupervisor>>>>,
}

impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            connections: Arc::clone(&self.connections),
            resource_subscriptions: Arc::clone(&self.resource_subscriptions),
            auth_coordinator: self.auth_coordinator.clone(),
            registry: self.registry.clone(),
            event_bus: self.event_bus.clone(),
            task_state: self.task_state.clone(),
            supervisor: Arc::clone(&self.supervisor),
        }
    }
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
            resource_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            auth_coordinator,
            registry,
            event_bus,
            task_state: TaskStateStore::new(cache),
            supervisor: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn attach_supervisor(&self, supervisor: Arc<InstanceSupervisor>) {
        *self.supervisor.lock().unwrap() = Some(supervisor);
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
    }

    pub async fn connect(&self, instance_id: InstanceId) -> Result<()> {
        let supervisor = self.supervisor.lock().unwrap().clone();
        let subscriptions = self
            .resource_subscriptions
            .read()
            .await
            .get(&instance_id)
            .cloned()
            .unwrap_or_default();
        let connected = {
            let mut conns = self.connections.write().await;
            let conn = conns.get_mut(&instance_id).ok_or_else(|| {
                Error::new(
                    FailureCode::NotConnected,
                    format!("Service instance not found: {instance_id}"),
                )
            })?;
            if conn.is_connected() {
                true
            } else {
                conn.connect(supervisor).await?;
                conn.refresh_subscription(&subscriptions).await?;
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
        self.resource_subscriptions.write().await.clear();
    }

    pub async fn instance_ids(&self) -> HashSet<InstanceId> {
        self.connections.read().await.keys().copied().collect()
    }

    pub async fn contains(&self, instance_id: InstanceId) -> bool {
        self.connections.read().await.contains_key(&instance_id)
    }

    pub async fn start_task_tool_execution(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
        meta: Option<rmcp::model::RequestMetaObject>,
        options: McpExecutionOptions,
    ) -> Result<McpToolExecutionHandle> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
        })?;
        conn.start_tool_task(tool_name, args, meta, options).await
    }

    pub async fn call_tool_task(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<McpToolExecution> {
        let execution = self
            .start_task_tool_execution(
                instance_id,
                tool_name,
                args,
                None,
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
        Ok(self
            .list_task_records(instance_id)
            .await?
            .into_iter()
            .map(|record| record.task)
            .collect())
    }

    pub async fn get_task(&self, instance_id: InstanceId, task_id: &str) -> Result<McpTask> {
        let task = {
            let conns = self.connections.read().await;
            let conn = conns.get(&instance_id).ok_or_else(|| {
                Error::new(
                    FailureCode::NotConnected,
                    format!("Service instance not found: {instance_id}"),
                )
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
                Error::new(
                    FailureCode::NotConnected,
                    format!("Service instance not found: {instance_id}"),
                )
            })?;
            conn.get_task_result(task_id).await
        };
        if let Err(error) = &result {
            self.record_task_error(instance_id, task_id, error).await;
        }
        result
    }

    pub async fn cancel_task(&self, instance_id: InstanceId, task_id: &str) -> Result<()> {
        let result = {
            let conns = self.connections.read().await;
            let conn = conns.get(&instance_id).ok_or_else(|| {
                Error::new(
                    FailureCode::NotConnected,
                    format!("Service instance not found: {instance_id}"),
                )
            })?;
            conn.cancel_task(task_id).await
        };
        if let Err(error) = &result {
            self.record_task_error(instance_id, task_id, error).await;
        }
        result
    }

    pub async fn list_task_records(&self, instance_id: InstanceId) -> Result<Vec<McpTaskRecord>> {
        self.task_state
            .list(instance_id)
            .await
            .map_err(|error| Error::new(FailureCode::TaskStateFailed, error.to_string()))
    }

    pub async fn get_task_record(
        &self,
        instance_id: InstanceId,
        task_id: &str,
    ) -> Result<Option<McpTaskRecord>> {
        self.task_state
            .get(instance_id, task_id)
            .await
            .map_err(|error| Error::new(FailureCode::TaskStateFailed, error.to_string()))
    }

    pub async fn list_tools(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredTool>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
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
        meta: Option<rmcp::model::RequestMetaObject>,
        options: McpExecutionOptions,
    ) -> Result<McpToolExecutionHandle> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
        })?;
        conn.start_tool_call(tool_name, args, meta, options).await
    }

    pub async fn call_tool(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<ToolCallResult> {
        match self
            .start_tool_execution(
                instance_id,
                tool_name,
                args,
                None,
                McpExecutionOptions::default(),
            )
            .await?
            .wait()
            .await?
        {
            McpToolExecution::Immediate { result } => Ok(result),
            McpToolExecution::Task { .. } => Err(Error::new(
                FailureCode::ToolFailed,
                "tool call unexpectedly returned a task",
            )),
        }
    }

    pub async fn list_resources(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredResource>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
        })?;
        conn.list_resources().await
    }

    pub async fn list_resource_templates(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<DiscoveredResourceTemplate>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
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
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
        })?;
        conn.read_resource(uri).await
    }

    pub async fn list_prompts(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredPrompt>> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
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
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
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
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
        })?;
        conn.complete(request).await
    }

    pub async fn subscribe_resource(&self, instance_id: InstanceId, uri: &str) -> Result<()> {
        let subscriptions = {
            let mut subscriptions = self.resource_subscriptions.write().await;
            let uris = subscriptions.entry(instance_id).or_default();
            uris.insert(uri.to_string());
            uris.clone()
        };
        let mut conns = self.connections.write().await;
        let conn = conns.get_mut(&instance_id).ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
        })?;
        conn.refresh_subscription(&subscriptions).await
    }

    pub async fn unsubscribe_resource(&self, instance_id: InstanceId, uri: &str) -> Result<()> {
        let subscriptions = {
            let mut subscriptions = self.resource_subscriptions.write().await;
            let mut remaining = HashSet::new();
            if let Some(uris) = subscriptions.get_mut(&instance_id) {
                uris.remove(uri);
                remaining.clone_from(uris);
                if uris.is_empty() {
                    subscriptions.remove(&instance_id);
                }
            }
            remaining
        };
        let mut conns = self.connections.write().await;
        let conn = conns.get_mut(&instance_id).ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
        })?;
        conn.refresh_subscription(&subscriptions).await
    }

    pub async fn ping(&self, instance_id: InstanceId, timeout: std::time::Duration) -> Result<()> {
        let conns = self.connections.read().await;
        let conn = conns.get(&instance_id).ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
        })?;
        conn.ping(timeout).await
    }

    pub async fn is_connected(&self, instance_id: InstanceId) -> bool {
        let conns = self.connections.read().await;
        conns
            .get(&instance_id)
            .map(McpConnection::is_connected)
            .unwrap_or(false)
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
            .map_err(|error| Error::new(FailureCode::TaskStateFailed, error.to_string()))
    }

    async fn record_task_error(&self, instance_id: InstanceId, task_id: &str, error: &Error) {
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
            .map_err(|error| Error::new(FailureCode::TaskStateFailed, error.to_string()))
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
