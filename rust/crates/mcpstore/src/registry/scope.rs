//! Agent scope resolution.
//!
//! Store-wide vs agent-specific service visibility.

use super::ServiceRegistry;

impl ServiceRegistry {
    /// Associate a service with an agent scope.
    pub async fn add_to_agent_scope(&self, agent_id: &str, service_name: &str) {
        let mut scopes = self.agent_scopes.write().await;
        let services = scopes.entry(agent_id.to_string()).or_default();
        if !services.iter().any(|name| name == service_name) {
            services.push(service_name.to_string());
        }
    }

    /// List services visible to an agent.
    pub async fn list_agent_services(&self, agent_id: &str) -> Vec<String> {
        let scopes = self.agent_scopes.read().await;
        scopes.get(agent_id).cloned().unwrap_or_default()
    }

    /// Clear all services from an agent scope.
    pub async fn clear_agent_scope(&self, agent_id: &str) {
        let mut scopes = self.agent_scopes.write().await;
        scopes.remove(agent_id);
    }
}
