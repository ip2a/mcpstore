use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_scope_instances(&self, scope: &ScopeRef) -> Result<Vec<ServiceInstance>> {
        let mut instances = match scope {
            ScopeRef::Store => self
                .kernel
                .control
                .registry
                .list_instances()
                .await
                .into_iter()
                .filter(|instance| instance.scope == ScopeRef::Store)
                .collect(),
            ScopeRef::Agent { agent_id } => {
                self.kernel
                    .control
                    .registry
                    .list_agent_instances(agent_id)
                    .await
            }
        };
        instances.sort_by(|left, right| {
            left.service_name
                .cmp(&right.service_name)
                .then_with(|| left.instance_id.cmp(&right.instance_id))
        });
        Ok(instances)
    }

    pub async fn instance_id_for_scope(
        &self,
        service_name: &str,
        scope: &ScopeRef,
    ) -> Result<InstanceId> {
        self.kernel
            .control
            .registry
            .instance_id(service_name, scope)
            .await
            .ok_or_else(|| {
                Error::new(
                    FailureCode::ServiceNotFound,
                    format!("scope {scope:?} is not declared for service '{service_name}'"),
                )
            })
    }

    /// Instances of a read view: `All` = every registered instance (store ∪ all
    /// agents, it is not an instance scope), store/agent delegate to the scope
    /// listing. Sorted like `list_scope_instances`.
    pub(crate) async fn list_view_instances(
        &self,
        view: &ScopeView,
    ) -> Result<Vec<ServiceInstance>> {
        let mut instances = match view.as_scope_ref() {
            Some(scope) => self.list_scope_instances(&scope).await?,
            None => self.kernel.control.registry.list_instances().await,
        };
        instances.sort_by(|left, right| {
            left.service_name
                .cmp(&right.service_name)
                .then_with(|| left.instance_id.cmp(&right.instance_id))
        });
        Ok(instances)
    }

    fn agent_shells_registry_path(&self) -> Option<PathBuf> {
        self.config_manager()
            .app_config_path()
            .parent()
            .map(|parent| parent.join("agent-shells.json"))
    }

    fn client_agents_registry_path(&self) -> Option<PathBuf> {
        self.config_manager()
            .app_config_path()
            .parent()
            .map(|parent| parent.join("client-agents.json"))
    }

    /// Agent ids that exist as empty shells (no services required): agent-shells
    /// registry + client-agent binding targets.
    pub(crate) fn listed_shell_agent_ids(&self) -> Result<HashSet<String>> {
        let mut ids = HashSet::new();
        if let Some(path) = self.agent_shells_registry_path() {
            let registry = crate::agent_shell::load_agent_shell_registry(&path)?;
            ids.extend(registry.shells.into_keys());
        }
        if let Some(path) = self.client_agents_registry_path() {
            match crate::client_agent::load_client_agent_registry(&path) {
                Ok(registry) => {
                    for binding in registry.bindings.into_values() {
                        ids.insert(binding.agent_id);
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        "listed_shell_agent_ids: failed to load client-agents registry: {}",
                        error
                    );
                }
            }
        }
        Ok(ids)
    }

    /// Create (or keep) an empty agent scope shell. Does not declare any service.
    pub async fn create_agent_shell(&self, agent_id: &str) -> Result<()> {
        let path = self.agent_shells_registry_path().ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                "Cannot resolve agent-shells registry path",
            )
        })?;
        crate::agent_shell::ensure_agent_shell(&path, agent_id)
    }

    /// Drop the empty-shell registration. Declared service instances are untouched;
    /// the agent remains listed while it still has instances or a client binding.
    pub async fn remove_agent_shell(&self, agent_id: &str) -> Result<bool> {
        let path = self.agent_shells_registry_path().ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                "Cannot resolve agent-shells registry path",
            )
        })?;
        crate::agent_shell::remove_agent_shell(&path, agent_id)
    }

    /// Scope registry: root + store + each agent, each entry carrying its runtime service count (from the registry).
    /// Also includes empty shells (agent-shells.json + client-agent bindings).
    pub async fn list_scopes(&self) -> Result<Vec<ScopeSummary>> {
        let instances = self.kernel.control.registry.list_instances().await;
        let mut agent_counts: HashMap<String, usize> = HashMap::new();
        let mut store_count = 0usize;
        for instance in &instances {
            match &instance.scope {
                ScopeRef::Store => store_count += 1,
                ScopeRef::Agent { agent_id } => {
                    *agent_counts.entry(agent_id.clone()).or_default() += 1;
                }
            }
        }

        for agent_id in self.listed_shell_agent_ids()? {
            agent_counts.entry(agent_id).or_insert(0);
        }

        let mut summaries = Vec::with_capacity(2 + agent_counts.len());
        summaries.push(ScopeSummary {
            scope: ScopeView::All,
            service_count: instances.len(),
        });
        summaries.push(ScopeSummary {
            scope: ScopeView::Store,
            service_count: store_count,
        });
        let mut agent_entries: Vec<(String, usize)> = agent_counts.into_iter().collect();
        agent_entries.sort_by(|left, right| left.0.cmp(&right.0));
        for (agent_id, service_count) in agent_entries {
            summaries.push(ScopeSummary {
                scope: ScopeView::Agent { agent_id },
                service_count,
            });
        }
        Ok(summaries)
    }

    /// Single scope summary (root = aggregated view of all services); returns None if undeclared.
    pub async fn scope_info(&self, view: &ScopeView) -> Result<Option<ScopeSummary>> {
        let scopes = self.list_scopes().await?;
        Ok(scopes.into_iter().find(|summary| &summary.scope == view))
    }

    /// Single agent entity (agent_id + its instance ids). Empty shells return
    /// `instance_ids: []` instead of None.
    pub async fn find_agent(&self, agent_id: &str) -> Result<Option<AgentInfo>> {
        let instance_ids = self
            .kernel
            .control
            .registry
            .list_agent_instance_ids(agent_id)
            .await;
        if !instance_ids.is_empty() {
            return Ok(Some(AgentInfo {
                agent_id: agent_id.to_string(),
                instance_ids,
            }));
        }
        if self.listed_shell_agent_ids()?.contains(agent_id) {
            return Ok(Some(AgentInfo {
                agent_id: agent_id.to_string(),
                instance_ids: Vec::new(),
            }));
        }
        Ok(None)
    }
}
