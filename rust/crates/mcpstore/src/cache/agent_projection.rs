use crate::cache::models::{AgentInstanceRelation, InstanceRelationItem};
use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn upsert_agent_instance_relation(
        &self,
        agent_id: &str,
        instance: &ServiceInstance,
        now: i64,
    ) -> Result<()> {
        let mut relation = match self.cache.get_relation("agent_instances", agent_id).await? {
            Some(value) => serde_json::from_value(value).map_err(|error| {
                StoreError::Other(format!("Agent relation deserialization failed: {error}"))
            })?,
            None => AgentInstanceRelation::default(),
        };

        if !relation
            .instances
            .iter()
            .any(|item| item.instance_id == instance.instance_id)
        {
            relation.instances.push(InstanceRelationItem {
                instance_id: instance.instance_id,
                service_name: instance.service_name.clone(),
                scope: instance.scope.clone(),
                established_time: now,
                last_access: Some(now),
            });
        }

        self.cache
            .put_relation(
                "agent_instances",
                agent_id,
                serde_json::to_value(relation).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    pub(in crate::cache) async fn remove_instance_from_agent_relations(
        &self,
        instance_id: InstanceId,
    ) -> Result<()> {
        let relations = self
            .cache
            .get_all_relations_async("agent_instances")
            .await?;
        for (agent_id, value) in relations {
            let mut relation: AgentInstanceRelation =
                serde_json::from_value(value).map_err(|error| {
                    StoreError::Other(format!("Agent relation deserialization failed: {error}"))
                })?;
            let original_len = relation.instances.len();
            relation
                .instances
                .retain(|item| item.instance_id != instance_id);

            if relation.instances.is_empty() {
                self.cache
                    .delete_relation("agent_instances", &agent_id)
                    .await?;
            } else if relation.instances.len() != original_len {
                self.cache
                    .put_relation(
                        "agent_instances",
                        &agent_id,
                        serde_json::to_value(relation).unwrap_or_default(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}
