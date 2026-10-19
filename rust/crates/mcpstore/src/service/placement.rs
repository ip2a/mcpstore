//! Data-panel routing: the panel's own config.toml `[placement]` decides local vs delegate.
//!
//! Entries are nested by scope then service (`[placement.agentA] github = "delegate"`); the
//! only meaningful value is `"delegate"` — a missing entry, an absent section or any other
//! value means local execution: pull the instance's kv effective_config (base + scope
//! overrides) and run it here. The control panel never reads this section. `store` is the
//! reserved scope section for store-scope instances.

use std::collections::HashMap;

use serde_json::Value;

use crate::identity::{InstanceId, ScopeRef};
use crate::store::prelude::*;
use crate::store::MCPStore;

const DELEGATE: &str = "delegate";

/// Config section name for the store scope (`[placement.store]`); reserved, an agent must
/// not be named `store`.
fn scope_token(scope: &ScopeRef) -> &str {
    match scope {
        ScopeRef::Store => "store",
        ScopeRef::Agent { agent_id } => agent_id,
    }
}

impl MCPStore {
    /// Placement decision for one instance on this data panel: false = delegate the call
    /// to the control panel through the shared store.
    pub(crate) async fn panel_executes_locally(&self, instance_id: InstanceId) -> Result<bool> {
        let instance = self
            .instance_from_kv(instance_id)
            .await?
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        let delegates = self.kernel.runtime.panel_placement
            .get(scope_token(&instance.scope))
            .and_then(|services| services.get(&instance.service_name))
            .map(String::as_str)
            == Some(DELEGATE);
        Ok(!delegates)
    }

    /// The data panel's single connection entry for local execution (ensure_instance_connected
    /// routes here): hydrate once, then connect with the instance's kv effective_config. Data-panel
    /// processes have no supervisor mounted, so there is no startup probe / health self-healing —
    /// pull the validated config and run it.
    pub(crate) async fn ensure_data_panel_instance_connected(
        &self,
        instance_id: InstanceId,
    ) -> Result<()> {
        if self.kernel.execution.pool.is_connected(instance_id).await {
            return Ok(());
        }
        if self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .is_none()
        {
            self.load_from_db().await?;
        }
        self.connect_service_internal(instance_id, false).await
    }

    /// Data-panel call routing: the local placement config decides — connect lazily and execute
    /// here, or delegate (request into the shared store, control panel claims and executes).
    pub(crate) async fn route_data_panel_tool_call(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: Value,
    ) -> Result<crate::transport::ToolCallResult> {
        if self.panel_executes_locally(instance_id).await? {
            self.ensure_data_panel_instance_connected(instance_id).await?;
            return self
                .kernel
                .execution
                .call_tool(self, instance_id, tool_name, args)
                .await;
        }
        self.call_tool_remote(instance_id, tool_name, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_delegate_entry_delegates() {
        let mut placement: HashMap<String, HashMap<String, String>> = HashMap::new();
        placement
            .entry("agentA".to_string())
            .or_default()
            .insert("github".to_string(), "delegate".to_string());
        placement
            .entry("store".to_string())
            .or_default()
            .insert("context7".to_string(), "local".to_string());
        placement
            .entry("agentB".to_string())
            .or_default()
            .insert("filesystem".to_string(), "nonsense".to_string());

        let delegates = |scope: &str, service: &str| {
            placement
                .get(scope)
                .and_then(|services| services.get(service))
                .map(String::as_str)
                == Some(DELEGATE)
        };
        assert!(delegates("agentA", "github")); // explicit delegate
        assert!(!delegates("store", "context7")); // explicit local
        assert!(!delegates("agentB", "filesystem")); // unknown value → local, no error
        assert!(!delegates("agentA", "context7")); // unknown service → local
        assert!(!delegates("agentC", "gitodo")); // unknown scope → local
    }
}
