use std::collections::HashMap;

use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_scope_instances(&self, scope: &ScopeRef) -> Result<Vec<ServiceInstance>> {
        self.refresh_from_db_if_needed().await?;
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
        self.refresh_from_db_if_needed().await?;
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

    /// 作用域注册表：root + store + 各 agent，每项带运行时服务数（来自 registry）。
    pub async fn list_scopes(&self) -> Result<Vec<ScopeSummary>> {
        self.refresh_from_db_if_needed().await?;
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
        let mut summaries = Vec::with_capacity(2 + agent_counts.len());
        summaries.push(ScopeSummary {
            scope: ScopeView::Root,
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

    /// 单个作用域摘要（root = 全部服务的聚合视图）；未声明返回 None。
    pub async fn scope_info(&self, view: &ScopeView) -> Result<Option<ScopeSummary>> {
        let scopes = self.list_scopes().await?;
        Ok(scopes.into_iter().find(|summary| &summary.scope == view))
    }

    /// 单个 agent 实体（agent_id + 其下实例 id）；不存在返回 None。
    pub async fn find_agent(&self, agent_id: &str) -> Result<Option<AgentInfo>> {
        self.refresh_from_db_if_needed().await?;
        let instance_ids = self
            .kernel
            .control
            .registry
            .list_agent_instance_ids(agent_id)
            .await;
        if instance_ids.is_empty() {
            return Ok(None);
        }
        Ok(Some(AgentInfo {
            agent_id: agent_id.to_string(),
            instance_ids,
        }))
    }
}
