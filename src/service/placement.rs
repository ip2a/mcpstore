//! Data-panel routing: the panel's own config.toml `[placement]` decides local vs delegate.
//!
//! Keys are `"{scope}.{service}"` (`"agentA.github"`, `"store.context7"`); the only meaningful
//! value is `"delegate"` — a missing entry, an absent section or any other value means local
//! execution: pull the instance's kv effective_config (base + scope overrides) and run it here.
//! The control panel never reads this section.

use crate::identity::InstanceId;
use crate::store::prelude::*;
use crate::store::MCPStore;

const DELEGATE: &str = "delegate";

/// `"{scope}.{service}"` placement key (`"store.context7"`, `"agentA.github"`).
fn placement_key(service_name: &str, scope: &crate::identity::ScopeRef) -> String {
    match scope {
        crate::identity::ScopeRef::Store => format!("store.{service_name}"),
        crate::identity::ScopeRef::Agent { agent_id } => format!("{agent_id}.{service_name}"),
    }
}

impl MCPStore {
    /// Placement decision for one instance on this data panel: false = delegate the call
    /// to the control panel through the shared store.
    pub async fn panel_executes_locally(&self, instance_id: InstanceId) -> Result<bool> {
        let instance = self
            .instance_from_kv(instance_id)
            .await?
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        let key = placement_key(&instance.service_name, &instance.scope);
        Ok(self
            .kernel
            .runtime
            .panel_placement
            .get(&key)
            .map(String::as_str)
            != Some(DELEGATE))
    }

    /// The data panel's single connection entry for local execution (ensure_instance_connected
    /// routes here): connect with the instance's kv effective_config. Data-panel
    /// processes have no supervisor mounted, so there is no startup probe / health self-healing —
    /// pull the validated config and run it.
    pub(crate) async fn ensure_data_panel_instance_connected(
        &self,
        instance_id: InstanceId,
    ) -> Result<()> {
        if self.kernel.execution.pool.is_connected(instance_id).await {
            return Ok(());
        }
        self.connect_service_internal(instance_id, false).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_key_pairs_scope_and_service() {
        assert_eq!(
            placement_key("github", &crate::identity::ScopeRef::Store),
            "store.github"
        );
        assert_eq!(
            placement_key(
                "github",
                &crate::identity::ScopeRef::Agent {
                    agent_id: "agentA".to_string()
                }
            ),
            "agentA.github"
        );
    }

    #[test]
    fn only_explicit_delegate_delegates() {
        let mut placement = std::collections::HashMap::new();
        placement.insert("store.github".to_string(), "delegate".to_string());
        placement.insert("agentA.context7".to_string(), "local".to_string());
        placement.insert("agentB.filesystem".to_string(), "nonsense".to_string());

        let mode = |key: &str| placement.get(key).map(String::as_str) != Some(DELEGATE);
        assert!(!mode("store.github")); // explicit delegate
        assert!(mode("agentA.context7")); // explicit local
        assert!(mode("agentB.filesystem")); // unknown value → local, no error
        assert!(mode("agentC.gitodo")); // missing → local
    }
}
