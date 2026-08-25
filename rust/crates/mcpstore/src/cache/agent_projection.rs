use crate::cache::models::{AgentInstanceRelation, InstanceRelationItem};
use crate::cache::CacheError;
use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn upsert_agent_instance_relation(
        &self,
        agent_id: &str,
        instance: &ServiceInstance,
        now: i64,
    ) -> Result<()> {
        for _ in 0..3 {
            let current = self.cache.get_relation("agent_instances", agent_id).await?;
            let expected_version = current.as_ref().map(|value| {
                value
                    .get("version")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
            });
            let mut relation = match current {
                Some(value) => serde_json::from_value(value).map_err(|error| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Agent relation deserialization failed: {error}"),
                    )
                })?,
                None => AgentInstanceRelation::default(),
            };

            if relation
                .instances
                .iter()
                .any(|item| item.instance_id == instance.instance_id)
            {
                return Ok(());
            }
            relation.instances.push(InstanceRelationItem {
                instance_id: instance.instance_id,
                service_name: instance.service_name.clone(),
                scope: instance.scope.clone(),
                established_time: now,
                last_access: Some(now),
            });
            relation.version += 1;
            match self
                .cache
                .compare_and_put_relation(
                    "agent_instances",
                    agent_id,
                    expected_version,
                    serde_json::to_value(relation).unwrap_or_default(),
                )
                .await
            {
                Ok(()) => return Ok(()),
                Err(CacheError::Conflict(_)) => continue,
                Err(error) => return Err(Error::from(error)),
            }
        }
        Err(Error::from(CacheError::Conflict(format!(
            "agent instance relation conflict after retries: agent_id={agent_id}"
        ))))
    }

    pub(in crate::cache) async fn remove_instance_from_agent_relations(
        &self,
        instance_id: InstanceId,
    ) -> Result<()> {
        let relations = self
            .cache
            .get_all_relations_async("agent_instances")
            .await?;
        for (agent_id, _) in relations {
            let mut complete = false;
            for _ in 0..3 {
                let Some(value) = self
                    .cache
                    .get_relation("agent_instances", &agent_id)
                    .await?
                else {
                    complete = true;
                    break;
                };
                let expected_version = value
                    .get("version")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let mut relation: AgentInstanceRelation =
                    serde_json::from_value(value).map_err(|error| {
                        Error::new(
                            FailureCode::Internal,
                            format!("Agent relation deserialization failed: {error}"),
                        )
                    })?;
                let original_len = relation.instances.len();
                relation
                    .instances
                    .retain(|item| item.instance_id != instance_id);
                if relation.instances.len() == original_len {
                    complete = true;
                    break;
                }
                relation.version += 1;
                match self
                    .cache
                    .compare_and_put_relation(
                        "agent_instances",
                        &agent_id,
                        Some(expected_version),
                        serde_json::to_value(relation).unwrap_or_default(),
                    )
                    .await
                {
                    Ok(()) => {
                        complete = true;
                        break;
                    }
                    Err(CacheError::Conflict(_)) => continue,
                    Err(error) => return Err(Error::from(error)),
                }
            }
            if !complete {
                return Err(Error::from(CacheError::Conflict(format!(
                    "agent instance relation conflict after retries: agent_id={agent_id}"
                ))));
            }
        }
        Ok(())
    }
}
