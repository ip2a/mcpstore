use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::cache::{CacheError, CacheLayerManager};
use crate::events::types::EventKind;
use crate::events::{Event, EventBus};
use crate::identity::InstanceId;

use super::{ServiceState, ServiceStateError, ServiceStateEvent};

const SERVICE_STATE_TYPE: &str = "service_state";

#[derive(Debug, thiserror::Error)]
pub enum ServiceStateManagerError {
    #[error("service state not found: {0}")]
    NotFound(InstanceId),
    #[error(transparent)]
    InvalidTransition(#[from] ServiceStateError),
    #[error(transparent)]
    Cache(#[from] CacheError),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

/// 控制面节点在状态分栏中的固定标识：`instance@control` 是权威栏。
pub const CONTROL_NODE_ID: &str = "control";

/// 每个节点一行心跳/能力自报的 state 类型。
pub const NODE_STATUS_TYPE: &str = "node_status";

pub struct ServiceStateManager {
    cache: Arc<CacheLayerManager>,
    event_bus: EventBus,
    node_id: String,
    locks: Mutex<HashMap<InstanceId, Arc<Mutex<()>>>>,
}

impl ServiceStateManager {
    pub fn new(cache: Arc<CacheLayerManager>, event_bus: EventBus, node_id: String) -> Self {
        Self {
            cache,
            event_bus,
            node_id,
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    fn own_key(&self, instance_id: InstanceId) -> String {
        format!("{instance_id}@{}", self.node_id)
    }

    fn control_key(instance_id: InstanceId) -> String {
        format!("{instance_id}@{CONTROL_NODE_ID}")
    }

    pub async fn create(
        &self,
        state: ServiceState,
    ) -> Result<ServiceState, ServiceStateManagerError> {
        let lock = self.instance_lock(state.instance_id).await;
        let _guard = lock.lock().await;
        let mut state = state;
        state.node = self.node_id.clone();
        self.cache
            .compare_and_put_state(
                SERVICE_STATE_TYPE,
                &self.own_key(state.instance_id),
                None,
                serde_json::to_value(&state)?,
            )
            .await?;
        Ok(state)
    }

    /// 本节点自己的栏：连接本地真相（auth/recovery/facade 上下文）。
    pub async fn get(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<ServiceState>, ServiceStateManagerError> {
        self.cache
            .get_state(SERVICE_STATE_TYPE, &self.own_key(instance_id))
            .await?
            .map(serde_json::from_value)
            .transpose()
            .map_err(Into::into)
    }

    /// 展示视角：control 权威栏优先，无则回退本节点栏。
    pub async fn get_display(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<ServiceState>, ServiceStateManagerError> {
        if let Some(state) = self
            .cache
            .get_state(SERVICE_STATE_TYPE, &Self::control_key(instance_id))
            .await?
            .map(serde_json::from_value::<ServiceState>)
            .transpose()?
        {
            return Ok(Some(state));
        }
        self.get(instance_id).await
    }

    pub async fn dispatch(
        &self,
        instance_id: InstanceId,
        event: ServiceStateEvent,
        now: i64,
    ) -> Result<ServiceState, ServiceStateManagerError> {
        let lock = self.instance_lock(instance_id).await;
        let _guard = lock.lock().await;
        let (previous, seeded_from_control) = match self.get(instance_id).await? {
            Some(previous) => (previous, false),
            // 本节点栏不存在时以 control 栏为底稿建栏：本节点观测从此有自己的家，
            // 也不再串写权威栏。
            None => match self
                .cache
                .get_state(SERVICE_STATE_TYPE, &Self::control_key(instance_id))
                .await?
                .map(serde_json::from_value::<ServiceState>)
                .transpose()?
            {
                Some(mut seeded) => {
                    seeded.node = self.node_id.clone();
                    seeded.version = 0;
                    (seeded, true)
                }
                None => return Err(ServiceStateManagerError::NotFound(instance_id)),
            },
        };
        let mut current = previous.clone();
        current.apply(event.clone(), now)?;
        // 建栏用 create-if-absent；更新沿用 CAS。
        let expected = (!seeded_from_control).then_some(previous.version);
        self.cache
            .compare_and_put_state(
                SERVICE_STATE_TYPE,
                &self.own_key(instance_id),
                expected,
                serde_json::to_value(&current)?,
            )
            .await?;
        self.event_bus
            .publish(
                Event::new(
                    EventKind::ServiceStateChanged.as_str(),
                    serde_json::json!({
                        "instance_id": instance_id,
                        "event": event,
                        "previous": previous,
                        "current": current,
                    }),
                ),
                true,
            )
            .await;
        Ok(current)
    }

    /// 只删本节点自己的栏；其他节点的观测栏不属于本节点。
    // ponytail: remove 只清 control 栏，数据节点的观测栏会残留，等出现真实清理需求再做按 instance 的全栏清理
    pub async fn delete(&self, instance_id: InstanceId) -> Result<(), ServiceStateManagerError> {
        let lock = self.instance_lock(instance_id).await;
        let _guard = lock.lock().await;
        self.cache
            .delete_state(SERVICE_STATE_TYPE, &self.own_key(instance_id))
            .await?;
        self.locks.lock().await.remove(&instance_id);
        Ok(())
    }

    /// data 面板心跳/能力自报：一行 per-node 记录，updated_at 即存活信号。
    pub async fn write_node_status(
        &self,
        payload: serde_json::Value,
    ) -> Result<(), ServiceStateManagerError> {
        let doc = serde_json::json!({
            "node": self.node_id,
            "updated_at": chrono::Utc::now().timestamp(),
            "payload": payload,
        });
        self.cache
            .put_state(NODE_STATUS_TYPE, &Self::node_status_key(&self.node_id), doc)
            .await?;
        Ok(())
    }

    pub async fn read_node_status(
        &self,
        node_id: &str,
    ) -> Result<Option<serde_json::Value>, ServiceStateManagerError> {
        Ok(self
            .cache
            .get_state(NODE_STATUS_TYPE, &Self::node_status_key(node_id))
            .await?)
    }

    pub async fn list_node_statuses(
        &self,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>, ServiceStateManagerError>
    {
        Ok(self.cache.get_all_states_async(NODE_STATUS_TYPE).await?)
    }

    /// 返回所有节点的最新心跳，并按时间窗给出可读的 liveness 状态。
    pub async fn node_liveness(
        &self,
        stale_after_secs: i64,
        now: i64,
    ) -> Result<serde_json::Value, ServiceStateManagerError> {
        let statuses = self.list_node_statuses().await?;
        let nodes = statuses
            .into_iter()
            .map(|(key, mut status)| {
                let updated_at = status["updated_at"].as_i64().unwrap_or(0);
                let state = if now.saturating_sub(updated_at) > stale_after_secs {
                    "unknown"
                } else {
                    "alive"
                };
                status["liveness"] = serde_json::json!(state);
                (key, status)
            })
            .collect::<serde_json::Map<_, _>>();
        Ok(serde_json::Value::Object(nodes))
    }

    fn node_status_key(node_id: &str) -> String {
        format!("node:{node_id}")
    }

    async fn instance_lock(&self, instance_id: InstanceId) -> Arc<Mutex<()>> {
        self.locks
            .lock()
            .await
            .entry(instance_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::storage::memory_cache_store;
    use crate::identity::{ScopeRef, ServiceInstanceKey};
    use crate::state::{AuthState, DesiredState, RuntimePhase};

    fn manager() -> (Arc<ServiceStateManager>, EventBus) {
        manager_as(CONTROL_NODE_ID)
    }

    fn manager_as(node_id: &str) -> (Arc<ServiceStateManager>, EventBus) {
        let event_bus = EventBus::with_history(10);
        let cache = Arc::new(CacheLayerManager::new(memory_cache_store(), "state-test"));
        (
            Arc::new(ServiceStateManager::new(
                cache,
                event_bus.clone(),
                node_id.to_string(),
            )),
            event_bus,
        )
    }

    fn initial() -> ServiceState {
        let scope = ScopeRef::Store;
        ServiceState::new(
            ServiceInstanceKey::new("test", scope.clone()).instance_id(),
            "test".to_string(),
            scope,
            DesiredState::Stopped,
            AuthState::NotRequired,
            1,
        )
    }

    #[tokio::test]
    async fn node_columns_do_not_clobber_each_other() {
        // A(control) 与 B(worker) 共享同一存储：B 的写只落自己的栏，
        // control 权威栏不受串写（坑 3 修法），展示读 control 优先。
        let cache = Arc::new(CacheLayerManager::new(memory_cache_store(), "state-cols"));
        let bus = EventBus::with_history(10);
        let control = Arc::new(ServiceStateManager::new(
            cache.clone(),
            bus.clone(),
            CONTROL_NODE_ID.to_string(),
        ));
        let worker = Arc::new(ServiceStateManager::new(cache, bus, "worker".to_string()));

        let created = control.create(initial()).await.unwrap();
        let instance_id = created.instance_id;
        control
            .dispatch(instance_id, ServiceStateEvent::StartRequested, 2)
            .await
            .unwrap();
        control
            .dispatch(instance_id, ServiceStateEvent::TransportConnected, 3)
            .await
            .unwrap();

        // worker 观测到自己侧连接停止：以 control 栏为底稿建自己的栏后落事件
        worker
            .dispatch(instance_id, ServiceStateEvent::TransportStopped, 4)
            .await
            .unwrap();

        let control_state = control.get(instance_id).await.unwrap().unwrap();
        assert_eq!(control_state.phase, RuntimePhase::Running);
        assert_eq!(control_state.node, CONTROL_NODE_ID);

        let worker_state = worker.get(instance_id).await.unwrap().unwrap();
        assert_eq!(worker_state.phase, RuntimePhase::Stopped);
        assert_eq!(worker_state.node, "worker");

        // 展示视角：worker 也看到 control 的权威 Running，而不是自己的观测
        let display = worker.get_display(instance_id).await.unwrap().unwrap();
        assert_eq!(display.phase, RuntimePhase::Running);
    }

    #[tokio::test]
    async fn node_status_round_trips() {
        let (manager, _bus) = manager_as("worker");
        manager
            .write_node_status(serde_json::json!({"capabilities": ["browser"]}))
            .await
            .unwrap();
        let status = manager.read_node_status("worker").await.unwrap().unwrap();
        assert_eq!(status["node"], "worker");
        assert_eq!(status["payload"]["capabilities"][0], "browser");
        assert!(status["updated_at"].as_i64().is_some());
    }

    #[tokio::test]
    async fn node_liveness_marks_stale_nodes_unknown() {
        let (manager, _bus) = manager_as("worker");
        let write_status = |updated_at: i64| {
            let manager = manager.clone();
            async move {
                manager
                    .cache
                    .put_state(
                        NODE_STATUS_TYPE,
                        &ServiceStateManager::node_status_key("worker"),
                        serde_json::json!({
                            "node": "worker",
                            "updated_at": updated_at,
                            "payload": {"capabilities": []}
                        }),
                    )
                    .await
                    .unwrap();
            }
        };

        let now = chrono::Utc::now().timestamp();
        write_status(now).await;
        let fresh = manager.node_liveness(45, now).await.unwrap();
        assert_eq!(fresh["node:worker"]["liveness"], "alive");

        write_status(now - 60).await;
        let stale = manager.node_liveness(45, now).await.unwrap();
        assert_eq!(stale["node:worker"]["liveness"], "unknown");
    }

    #[tokio::test]
    async fn dispatch_commits_before_publishing_state_changed() {
        let (manager, event_bus) = manager();
        let state = manager.create(initial()).await.unwrap();
        let current = manager
            .dispatch(state.instance_id, ServiceStateEvent::StartRequested, 2)
            .await
            .unwrap();
        assert_eq!(current.phase, RuntimePhase::Starting);
        assert_eq!(manager.get(state.instance_id).await.unwrap(), Some(current));
        let history = event_bus.get_history(1).await;
        assert_eq!(
            history[0].event_type,
            EventKind::ServiceStateChanged.as_str()
        );
        assert_eq!(history[0].payload["current"]["version"], 1);
    }

    #[tokio::test]
    async fn invalid_transition_does_not_change_persisted_state() {
        let (manager, _) = manager();
        let state = manager.create(initial()).await.unwrap();
        let result = manager
            .dispatch(state.instance_id, ServiceStateEvent::TransportConnected, 2)
            .await;
        assert!(matches!(
            result,
            Err(ServiceStateManagerError::InvalidTransition(_))
        ));
        assert_eq!(manager.get(state.instance_id).await.unwrap(), Some(state));
    }

    #[tokio::test]
    async fn concurrent_dispatches_are_serialized_per_instance() {
        let (manager, _) = manager();
        let state = manager.create(initial()).await.unwrap();
        let first = tokio::spawn({
            let manager = manager.clone();
            async move {
                manager
                    .dispatch(state.instance_id, ServiceStateEvent::StartRequested, 2)
                    .await
            }
        });
        let second = tokio::spawn({
            let manager = manager.clone();
            async move {
                manager
                    .dispatch(state.instance_id, ServiceStateEvent::ToolSyncStarted, 3)
                    .await
            }
        });
        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();
        let current = manager.get(state.instance_id).await.unwrap().unwrap();
        assert_eq!(current.version, 2);
    }

    #[tokio::test]
    async fn duplicate_create_is_rejected() {
        let (manager, _) = manager();
        let state = manager.create(initial()).await.unwrap();
        let result = manager.create(state).await;
        assert!(matches!(
            result,
            Err(ServiceStateManagerError::Cache(CacheError::Conflict(_)))
        ));
    }
}
