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

pub struct ServiceStateManager {
    cache: Arc<CacheLayerManager>,
    event_bus: EventBus,
    locks: Mutex<HashMap<InstanceId, Arc<Mutex<()>>>>,
}

impl ServiceStateManager {
    pub fn new(cache: Arc<CacheLayerManager>, event_bus: EventBus) -> Self {
        Self {
            cache,
            event_bus,
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub async fn create(
        &self,
        state: ServiceState,
    ) -> Result<ServiceState, ServiceStateManagerError> {
        let lock = self.instance_lock(state.instance_id).await;
        let _guard = lock.lock().await;
        self.cache
            .compare_and_put_state(
                SERVICE_STATE_TYPE,
                &state.instance_id.to_string(),
                None,
                serde_json::to_value(&state)?,
            )
            .await?;
        Ok(state)
    }

    pub async fn get(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<ServiceState>, ServiceStateManagerError> {
        self.cache
            .get_state(SERVICE_STATE_TYPE, &instance_id.to_string())
            .await?
            .map(serde_json::from_value)
            .transpose()
            .map_err(Into::into)
    }

    pub async fn dispatch(
        &self,
        instance_id: InstanceId,
        event: ServiceStateEvent,
        now: i64,
    ) -> Result<ServiceState, ServiceStateManagerError> {
        let lock = self.instance_lock(instance_id).await;
        let _guard = lock.lock().await;
        let previous = self
            .get(instance_id)
            .await?
            .ok_or(ServiceStateManagerError::NotFound(instance_id))?;
        let mut current = previous.clone();
        current.apply(event.clone(), now)?;
        self.cache
            .compare_and_put_state(
                SERVICE_STATE_TYPE,
                &instance_id.to_string(),
                Some(previous.version),
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

    pub async fn delete(&self, instance_id: InstanceId) -> Result<(), ServiceStateManagerError> {
        let lock = self.instance_lock(instance_id).await;
        let _guard = lock.lock().await;
        self.cache
            .delete_state(SERVICE_STATE_TYPE, &instance_id.to_string())
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
        let event_bus = EventBus::with_history(10);
        let cache = Arc::new(CacheLayerManager::new(memory_cache_store(), "state-test"));
        (
            Arc::new(ServiceStateManager::new(cache, event_bus.clone())),
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
