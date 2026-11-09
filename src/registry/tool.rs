use crate::identity::InstanceId;
use crate::registry::{ServiceRegistry, ToolInfo};

impl ServiceRegistry {
    pub async fn replace_instance_tools(
        &self,
        instance_id: InstanceId,
        tools: Vec<ToolInfo>,
    ) -> bool {
        let mut instances = self.instances.write().await;
        let Some(instance) = instances.get_mut(&instance_id) else {
            return false;
        };
        instance.tools = tools;
        true
    }

    pub async fn find_tool(&self, instance_id: InstanceId, tool_name: &str) -> Option<ToolInfo> {
        self.instances
            .read()
            .await
            .get(&instance_id)?
            .tools
            .iter()
            .find(|tool| tool.name == tool_name)
            .cloned()
    }

    pub async fn list_all_tools(&self) -> Vec<(InstanceId, ToolInfo)> {
        self.instances
            .read()
            .await
            .values()
            .flat_map(|instance| {
                instance
                    .tools
                    .iter()
                    .cloned()
                    .map(|tool| (instance.instance_id, tool))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub async fn list_instance_tools(&self, instance_id: InstanceId) -> Vec<ToolInfo> {
        self.instances
            .read()
            .await
            .get(&instance_id)
            .map(|instance| instance.tools.clone())
            .unwrap_or_default()
    }
}
