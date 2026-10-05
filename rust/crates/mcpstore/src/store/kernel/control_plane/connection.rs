use crate::store::prelude::*;
use crate::store::{ControlPlane, MCPStore};

impl ControlPlane {
    pub async fn connect_service(&self, store: &MCPStore, instance_id: InstanceId) -> Result<()> {
        // Same entry as the execution surface: fill-if-missing hydration / data-panel placement resolution all live here.
        store.ensure_instance_connected(instance_id).await
    }
}

impl MCPStore {
    /// Connect instances with desired=Running (keep_alive / OnStoreStart semantics).
    /// Startup (tail of load) and event consumption share this one copy; semantics don't fork.
    pub(crate) async fn connect_desired_running_instances(&self, service_name: Option<&str>) {
        for instance in self.kernel.control.registry.list_instances().await {
            if service_name.is_some_and(|name| instance.service_name != name) {
                continue;
            }
            let Ok(Some(state)) = self.kernel.control.state.get(instance.instance_id).await else {
                continue;
            };
            if state.desired != crate::state::DesiredState::Running {
                continue;
            }
            if let Err(error) = self
                .connect_service_internal(instance.instance_id, false)
                .await
            {
                tracing::warn!(
                    "[STORE] desired-instance connect failed: {} ({})",
                    instance.instance_id,
                    error
                );
            }
        }
    }
}
