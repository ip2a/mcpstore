use crate::identity::{InstanceId, ScopeRef, ServiceInstanceKey};
use crate::registry::{ConnectionStatus, ServiceDefinition, ServiceInstance, ServiceRegistry};

impl ServiceRegistry {
    pub async fn register_definition(&self, definition: ServiceDefinition) {
        self.definitions
            .write()
            .await
            .insert(definition.service_name.clone(), definition);
    }

    pub async fn unregister_definition(&self, service_name: &str) -> Vec<InstanceId> {
        self.definitions.write().await.remove(service_name);
        let instance_ids = self
            .instances
            .read()
            .await
            .values()
            .filter(|instance| instance.service_name == service_name)
            .map(|instance| instance.instance_id)
            .collect::<Vec<_>>();
        for instance_id in &instance_ids {
            self.unregister_instance(*instance_id).await;
        }
        instance_ids
    }

    pub async fn find_definition(&self, service_name: &str) -> Option<ServiceDefinition> {
        self.definitions.read().await.get(service_name).cloned()
    }

    pub async fn list_definitions(&self) -> Vec<ServiceDefinition> {
        self.definitions.read().await.values().cloned().collect()
    }

    pub async fn register_instance(&self, instance: ServiceInstance) {
        let instance_id = instance.instance_id;
        let key = instance.key();
        let scope = instance.scope.clone();

        self.instances.write().await.insert(instance_id, instance);
        self.instance_index.write().await.insert(key, instance_id);

        if let ScopeRef::Agent { agent_id } = scope {
            let mut index = self.agent_index.write().await;
            let instances = index.entry(agent_id).or_default();
            if !instances.contains(&instance_id) {
                instances.push(instance_id);
            }
        }
    }

    pub async fn unregister_instance(&self, instance_id: InstanceId) {
        let Some(instance) = self.instances.write().await.remove(&instance_id) else {
            return;
        };
        self.instance_index.write().await.remove(&instance.key());
        if let ScopeRef::Agent { agent_id } = instance.scope {
            let mut index = self.agent_index.write().await;
            if let Some(instances) = index.get_mut(&agent_id) {
                instances.retain(|candidate| *candidate != instance_id);
                if instances.is_empty() {
                    index.remove(&agent_id);
                }
            }
        }
    }

    pub async fn find_instance(&self, instance_id: InstanceId) -> Option<ServiceInstance> {
        self.instances.read().await.get(&instance_id).cloned()
    }

    pub async fn find_instance_by_key(
        &self,
        service_name: &str,
        scope: &ScopeRef,
    ) -> Option<ServiceInstance> {
        let key = ServiceInstanceKey::new(service_name, scope.clone());
        let instance_id = self.instance_index.read().await.get(&key).copied()?;
        self.find_instance(instance_id).await
    }

    pub async fn instance_id(&self, service_name: &str, scope: &ScopeRef) -> Option<InstanceId> {
        let key = ServiceInstanceKey::new(service_name, scope.clone());
        self.instance_index.read().await.get(&key).copied()
    }

    pub async fn list_instances(&self) -> Vec<ServiceInstance> {
        self.instances.read().await.values().cloned().collect()
    }

    pub async fn update_status(&self, instance_id: InstanceId, status: ConnectionStatus) {
        if let Some(instance) = self.instances.write().await.get_mut(&instance_id) {
            instance.status = status;
        }
    }

    pub async fn mark_applied(&self, instance_id: InstanceId) {
        if let Some(instance) = self.instances.write().await.get_mut(&instance_id) {
            instance.applied_config_revision = Some(instance.config_revision);
        }
    }
}
