use crate::identity::InstanceId;
use crate::registry::{ServiceInstance, ServiceRegistry};

impl ServiceRegistry {
    pub async fn list_agent_instance_ids(&self, agent_id: &str) -> Vec<InstanceId> {
        self.agent_index
            .read()
            .await
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn list_agent_instances(&self, agent_id: &str) -> Vec<ServiceInstance> {
        let instance_ids = self.list_agent_instance_ids(agent_id).await;
        let instances = self.instances.read().await;
        instance_ids
            .into_iter()
            .filter_map(|instance_id| instances.get(&instance_id).cloned())
            .collect()
    }

    pub async fn list_agent_ids(&self) -> Vec<String> {
        self.agent_index.read().await.keys().cloned().collect()
    }
}
