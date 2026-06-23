use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn cache_agent_scope(&self, agent_id: &str) -> Result<()> {
        let service_names = self.registry.list_agent_services(agent_id).await;
        self.cache_agent_scope_names(agent_id, service_names).await
    }

    pub(crate) async fn cache_agent_scope_names(
        &self,
        agent_id: &str,
        service_names: Vec<String>,
    ) -> Result<()> {
        let mut services = Vec::with_capacity(service_names.len());
        let now = chrono::Utc::now().timestamp();
        for service_name in service_names {
            let parsed = parse_agent_scoped(&service_name)?;
            services.push(ServiceRelationItem {
                service_original_name: parsed.local_name,
                service_global_name: service_name.clone(),
                client_id: service_name,
                established_time: now,
                last_access: Some(now),
            });
        }
        self.cache
            .put_relation(
                "agent_services",
                agent_id,
                serde_json::to_value(AgentServiceRelation { services }).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    pub(in crate::cache) async fn upsert_agent_service_relation(
        &self,
        agent_id: &str,
        service_name: &str,
        now: i64,
    ) -> Result<()> {
        let mut relation = match self.cache.get_relation("agent_services", agent_id).await? {
            Some(value) => serde_json::from_value(value).map_err(|e| {
                StoreError::Other(format!("Agent relation deserialization failed: {e}"))
            })?,
            None => AgentServiceRelation {
                services: Vec::new(),
            },
        };

        if !relation
            .services
            .iter()
            .any(|item| item.service_global_name == service_name)
        {
            let parsed = parse_agent_scoped(service_name)?;
            relation.services.push(ServiceRelationItem {
                service_original_name: parsed.local_name,
                service_global_name: service_name.to_string(),
                client_id: service_name.to_string(),
                established_time: now,
                last_access: Some(now),
            });
        }

        self.cache
            .put_relation(
                "agent_services",
                agent_id,
                serde_json::to_value(relation).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn cached_agent_scope(&self, agent_id: &str) -> Result<Vec<String>> {
        let value = self.cache.get_relation("agent_services", agent_id).await?;
        match value {
            Some(value) => {
                let relation: AgentServiceRelation =
                    serde_json::from_value(value).map_err(|e| {
                        StoreError::Other(format!("Agent scope deserialization failed: {e}"))
                    })?;
                Ok(relation
                    .services
                    .into_iter()
                    .map(|item| item.service_global_name)
                    .collect())
            }
            None => Ok(Vec::new()),
        }
    }

    pub(in crate::cache) async fn remove_service_from_agent_relations(
        &self,
        name: &str,
    ) -> Result<()> {
        let relations = self.cache.get_all_relations_async("agent_services").await?;
        for (agent_id, value) in relations {
            let mut relation: AgentServiceRelation =
                serde_json::from_value(value).map_err(|e| {
                    StoreError::Other(format!("Agent relation deserialization failed: {e}"))
                })?;
            let original_len = relation.services.len();
            relation.services.retain(|item| {
                item.service_global_name != name && item.service_original_name != name
            });

            if relation.services.is_empty() {
                self.cache
                    .delete_relation("agent_services", &agent_id)
                    .await?;
            } else if relation.services.len() != original_len {
                self.cache
                    .put_relation(
                        "agent_services",
                        &agent_id,
                        serde_json::to_value(relation).unwrap_or_default(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}
