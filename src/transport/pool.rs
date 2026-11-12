use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio::sync::RwLock;

use crate::health::supervisor::InstanceSupervisor;
use crate::kv::KvLayerManager;

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

/// Concurrent connect critical sections allowed to drain at once. Sized for
/// a 26-service batch boot: wide enough to keep handshakes overlapping,
/// narrow enough that child-process spawns and remote handshakes never
/// stampede the machine (R3: 26 parallel connects starved 10 services into
/// a silent, unlogged hang).
pub(crate) const CONNECT_MAX_PARALLEL: usize = 4;
/// Hard ceiling on one connect attempt end to end — window queue, gate-held
/// span (OAuth discovery, transport connect, startup probe, tool sync), and
/// the terminal-failure cleanup. Anything still inside when it elapses is
/// recorded as a terminal failure — no eternal `starting`, panel-visible.
pub(crate) const CONNECT_HARD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

pub struct ConnectionPool {
    /// One mutex per connection: transport-level awaits (connect, disconnect,
    /// ping) must never serialize unrelated instances. The map lock is only
    /// held to clone or swap entries, never across an await on a connection.
    connections: Arc<RwLock<HashMap<InstanceId, Arc<tokio::sync::Mutex<McpConnection>>>>>,
    /// Per-instance in-flight connect gates. connect_service_internal holds
    /// the gate across its remove/add/connect span, so concurrent "connect
    /// the same instance" requests (parallel gateway mount builds, panel
    /// retries, OAuth callbacks) coalesce into one transport instead of
    /// trampling each other's pool entries. The map lock is std and never
    /// held across an await.
    connect_guards: Arc<std::sync::Mutex<HashMap<InstanceId, Arc<tokio::sync::Mutex<()>>>>>,
    /// Bounds connect critical sections draining at once. A 26-service
    /// parallel batch boot must drain as a bounded queue, not stampede the
    /// machine with concurrent child-process spawns and handshakes.
    connect_window: Arc<tokio::sync::Semaphore>,
    resource_subscriptions: Arc<RwLock<HashMap<InstanceId, HashSet<String>>>>,
    auth_coordinator: AuthCoordinator,
    registry: ServiceRegistry,
    event_bus: EventBus,
    task_state: TaskStateStore,
    supervisor: Arc<Mutex<Option<Arc<InstanceSupervisor>>>>,
    /// Frame-telemetry sink. None until the host attaches one; read lazily at
    /// connect time so late attachment still covers the next connection.
    frame_telemetry: Arc<std::sync::OnceLock<Option<Arc<dyn crate::telemetry::FrameTelemetry>>>>,
}

impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            connections: Arc::clone(&self.connections),
            connect_guards: Arc::clone(&self.connect_guards),
            connect_window: Arc::clone(&self.connect_window),
            resource_subscriptions: Arc::clone(&self.resource_subscriptions),
            auth_coordinator: self.auth_coordinator.clone(),
            registry: self.registry.clone(),
            event_bus: self.event_bus.clone(),
            task_state: self.task_state.clone(),
            supervisor: Arc::clone(&self.supervisor),
            frame_telemetry: Arc::clone(&self.frame_telemetry),
        }
    }
}

impl ConnectionPool {
    pub fn new(
        auth_coordinator: AuthCoordinator,
        registry: ServiceRegistry,
        event_bus: EventBus,
        kv: Arc<KvLayerManager>,
    ) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connect_guards: Arc::new(std::sync::Mutex::new(HashMap::new())),
            connect_window: Arc::new(tokio::sync::Semaphore::const_new(CONNECT_MAX_PARALLEL)),
            resource_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            auth_coordinator,
            registry,
            event_bus,
            task_state: TaskStateStore::new(kv),
            supervisor: Arc::new(Mutex::new(None)),
            frame_telemetry: Arc::new(std::sync::OnceLock::new()),
        }
    }

    /// The storm-control window: bounds how many connect critical sections
    /// drain at once (see CONNECT_MAX_PARALLEL).
    pub(crate) fn connect_window(&self) -> Arc<tokio::sync::Semaphore> {
        Arc::clone(&self.connect_window)
    }

    /// Hard ceiling on one connect attempt end to end (see
    /// CONNECT_HARD_TIMEOUT).
    pub(crate) fn connect_hard_timeout() -> std::time::Duration {
        CONNECT_HARD_TIMEOUT
    }

    pub(crate) fn attach_supervisor(&self, supervisor: Arc<InstanceSupervisor>) {
        *self.supervisor.lock().unwrap() = Some(supervisor);
    }

    /// The connect gate for one instance (created on first use). The returned
    /// mutex is the serialization point for same-instance connect attempts.
    pub(crate) async fn connect_guard(
        &self,
        instance_id: InstanceId,
    ) -> Arc<tokio::sync::Mutex<()>> {
        self.connect_guards
            .lock()
            .unwrap()
            .entry(instance_id)
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// Attach the frame-telemetry sink (host opts in; SDK default = off).
    /// Idempotent: the first attach wins.
    pub(crate) fn attach_frame_telemetry(
        &self,
        sink: Option<Arc<dyn crate::telemetry::FrameTelemetry>>,
    ) {
        let _ = self.frame_telemetry.set(sink);
    }

    pub(crate) fn frame_telemetry(&self) -> Option<Arc<dyn crate::telemetry::FrameTelemetry>> {
        self.frame_telemetry.get().and_then(|slot| slot.clone())
    }

    pub async fn add(&self, instance_id: InstanceId, config: ServerConfig) {
        let conn = McpConnection::new(
            instance_id,
            instance_id.to_string(),
            config,
            self.auth_coordinator.clone(),
            self.registry.clone(),
            self.event_bus.clone(),
            Arc::clone(&self.frame_telemetry),
        );
        self.connections
            .write()
            .await
            .insert(instance_id, Arc::new(tokio::sync::Mutex::new(conn)));
    }

    /// Clone the connection entry without holding the map lock across any
    /// subsequent await on the connection itself. The map lock is only ever
    /// held to clone or swap entries, so awaiting it is bounded.
    async fn connection(
        &self,
        instance_id: InstanceId,
    ) -> Result<Arc<tokio::sync::Mutex<McpConnection>>> {
        self.connection_opt(instance_id).await.ok_or_else(|| {
            Error::new(
                FailureCode::NotConnected,
                format!("Service instance not found: {instance_id}"),
            )
        })
    }

    async fn connection_opt(
        &self,
        instance_id: InstanceId,
    ) -> Option<Arc<tokio::sync::Mutex<McpConnection>>> {
        self.connections.read().await.get(&instance_id).cloned()
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
            let conn = self.connection(instance_id).await?;
            let mut conn = conn.lock().await;
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
        if let Some(conn) = self.connection_opt(instance_id).await {
            conn.lock().await.disconnect().await?;
        }
        self.mark_tasks_disconnected(instance_id, "connection closed")
            .await
    }

    pub async fn remove(&self, instance_id: InstanceId) -> Result<()> {
        let conn = self.connections.write().await.remove(&instance_id);
        if let Some(conn) = conn {
            conn.lock().await.disconnect().await.ok();
        }
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
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
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
            let conn = self.connection(instance_id).await?;
            let conn = conn.lock().await;
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
            let conn = self.connection(instance_id).await?;
            let conn = conn.lock().await;
            conn.get_task_result(task_id).await
        };
        if let Err(error) = &result {
            self.record_task_error(instance_id, task_id, error).await;
        }
        result
    }

    pub async fn cancel_task(&self, instance_id: InstanceId, task_id: &str) -> Result<()> {
        let result = {
            let conn = self.connection(instance_id).await?;
            let conn = conn.lock().await;
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
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
        conn.list_tools().await
    }

    pub async fn open_elicitation_session(
        &self,
        instance_id: InstanceId,
        options: crate::transport::McpElicitationSessionOptions,
    ) -> Result<Option<crate::transport::McpElicitationSession>> {
        let Some(conn) = self.connection_opt(instance_id).await else {
            return Ok(None);
        };
        let conn = conn.lock().await;
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
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
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
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
        conn.list_resources().await
    }

    pub async fn list_resource_templates(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<DiscoveredResourceTemplate>> {
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
        conn.list_resource_templates().await
    }

    pub async fn read_resource(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<serde_json::Value> {
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
        conn.read_resource(uri).await
    }

    pub async fn list_prompts(&self, instance_id: InstanceId) -> Result<Vec<DiscoveredPrompt>> {
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
        conn.list_prompts().await
    }

    pub async fn get_prompt(
        &self,
        instance_id: InstanceId,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
        conn.get_prompt(prompt_name, arguments).await
    }

    pub async fn server_metadata(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<McpServerMetadata>> {
        let Some(conn) = self.connection_opt(instance_id).await else {
            return Ok(None);
        };
        let conn = conn.lock().await;
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
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
        conn.complete(request).await
    }

    pub async fn subscribe_resource(&self, instance_id: InstanceId, uri: &str) -> Result<()> {
        let subscriptions = {
            let mut subscriptions = self.resource_subscriptions.write().await;
            let uris = subscriptions.entry(instance_id).or_default();
            uris.insert(uri.to_string());
            uris.clone()
        };
        let conn = self.connection(instance_id).await?;
        let mut conn = conn.lock().await;
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
        let conn = self.connection(instance_id).await?;
        let mut conn = conn.lock().await;
        conn.refresh_subscription(&subscriptions).await
    }

    pub async fn ping(&self, instance_id: InstanceId, timeout: std::time::Duration) -> Result<()> {
        self.ping_elapsed(instance_id, timeout).await.map(|_| ())
    }

    /// Ping with the latency measured at the transport request itself:
    /// waiting for the connection mutex (e.g. behind a slow tool call) is
    /// queueing, not server latency, and must not pollute health windows.
    pub async fn ping_elapsed(
        &self,
        instance_id: InstanceId,
        timeout: std::time::Duration,
    ) -> Result<std::time::Duration> {
        let conn = self.connection(instance_id).await?;
        let conn = conn.lock().await;
        let started = std::time::Instant::now();
        conn.ping(timeout).await?;
        Ok(started.elapsed())
    }

    pub async fn is_connected(&self, instance_id: InstanceId) -> bool {
        match self.connection_opt(instance_id).await {
            Some(conn) => conn.lock().await.is_connected(),
            None => false,
        }
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
