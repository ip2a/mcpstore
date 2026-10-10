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

/// Fixed identifier of the control node in the state columns: `instance@control` is the authoritative column.
pub const CONTROL_NODE_ID: &str = "control";

pub struct ServiceStateManager {
    cache: Arc<CacheLayerManager>,
    event_bus: EventBus,
    panel_id: String,
    locks: Mutex<HashMap<InstanceId, Arc<Mutex<()>>>>,
}

impl ServiceStateManager {
    pub fn new(cache: Arc<CacheLayerManager>, event_bus: EventBus, panel_id: String) -> Self {
        Self {
            cache,
            event_bus,
            panel_id,
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn panel_id(&self) -> &str {
        &self.panel_id
    }

    fn own_key(&self, instance_id: InstanceId) -> String {
        format!("{instance_id}@{}", self.panel_id)
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
        state.node = self.panel_id.clone();
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

    /// This node's own column: local truth of the connection (auth/recovery/facade context).
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

    /// Display view: prefer the control authoritative column; fall back to this node's column.
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
            // When this node's column is absent, create it using the control column as the base:
            // local observations get their own home and no longer cross-write the authoritative column.
            None => match self
                .cache
                .get_state(SERVICE_STATE_TYPE, &Self::control_key(instance_id))
                .await?
                .map(serde_json::from_value::<ServiceState>)
                .transpose()?
            {
                Some(mut seeded) => {
                    seeded.node = self.panel_id.clone();
                    seeded.version = 0;
                    (seeded, true)
                }
                None => return Err(ServiceStateManagerError::NotFound(instance_id)),
            },
        };
        let mut current = previous.clone();
        current.apply(event.clone(), now)?;
        // Column creation uses create-if-absent; updates keep CAS.
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

    /// Deletes only this node's own column; other nodes' observation columns are not ours.
    // ponytail: remove only clears the control column; data-node observation columns linger — add per-instance full-column cleanup when a real need appears
    pub async fn delete(&self, instance_id: InstanceId) -> Result<(), ServiceStateManagerError> {
        let lock = self.instance_lock(instance_id).await;
        let _guard = lock.lock().await;
        self.cache
            .delete_state(SERVICE_STATE_TYPE, &self.own_key(instance_id))
            .await?;
        self.locks.lock().await.remove(&instance_id);
        Ok(())
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

    fn manager_as(panel_id: &str) -> (Arc<ServiceStateManager>, EventBus) {
        let event_bus = EventBus::with_history(10);
        let cache = Arc::new(CacheLayerManager::new(memory_cache_store(), "state-test"));
        (
            Arc::new(ServiceStateManager::new(
                cache,
                event_bus.clone(),
                panel_id.to_string(),
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
        // A(control) and B(worker) share one storage: B's writes only land in its own column,
        // the control authoritative column is never cross-written (pitfall 3 fix); display reads prefer control.
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

        // the worker observes its own side of the connection stopping: build its column from the control column, then record the event
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

        // Display view: the worker also sees control's authoritative Running instead of its own observation
        let display = worker.get_display(instance_id).await.unwrap().unwrap();
        assert_eq!(display.phase, RuntimePhase::Running);
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
