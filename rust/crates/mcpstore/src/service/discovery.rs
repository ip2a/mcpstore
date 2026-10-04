use crate::state::{RecoveryState, RuntimePhase};
use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn ensure_instance_connected(&self, instance_id: InstanceId) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        if self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .is_none()
        {
            return Err(Error::new(
                FailureCode::ServiceNotFound,
                instance_id.to_string(),
            ));
        }
        let state = self
            .kernel
            .control
            .state
            .get(instance_id)
            .await?
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        let transport_connected = if self.is_openapi_virtual_instance(instance_id).await? {
            state.phase == RuntimePhase::Running
        } else {
            self.kernel.execution.pool.is_connected(instance_id).await
                && state.phase == RuntimePhase::Running
        };
        if transport_connected {
            return Ok(());
        }
        self.ensure_service_auto_start_allowed(instance_id).await?;
        self.connect_service_internal(
            instance_id,
            matches!(state.recovery, RecoveryState::Waiting { .. }),
        )
        .await
    }

    pub(crate) async fn is_openapi_virtual_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<bool> {
        let Some(instance) = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
        else {
            return Ok(false);
        };
        Ok(instance.transport == "openapi")
    }

    /// 查询面直读共享库，不注水内存注册表（`load_from_db` 只留给执行路径）。
    pub async fn list_instances(&self) -> Vec<ServiceInstance> {
        self.instances_from_kv().await.unwrap_or_default()
    }

    pub async fn find_instance(&self, instance_id: InstanceId) -> Option<ServiceInstance> {
        self.instance_from_kv(instance_id).await.ok().flatten()
    }

    pub async fn find_definition(&self, service_name: &str) -> Option<ServiceDefinition> {
        self.definition_from_kv(service_name).await.ok().flatten()
    }

    pub async fn list_tools(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<crate::registry::ToolInfo>> {
        if self.instance_from_kv(instance_id).await?.is_none() {
            return Err(Error::new(
                FailureCode::ServiceNotFound,
                instance_id.to_string(),
            ));
        }
        self.tools_from_kv(instance_id).await
    }

    pub async fn list_all_tools(&self) -> Vec<(InstanceId, crate::registry::ToolInfo)> {
        self.instances_from_kv()
            .await
            .unwrap_or_default()
            .into_iter()
            .flat_map(|instance| {
                instance
                    .tools
                    .into_iter()
                    .map(move |tool| (instance.instance_id, tool))
            })
            .collect()
    }

    pub async fn list_agents(&self) -> Result<Vec<serde_json::Value>> {
        let mut by_agent = std::collections::BTreeMap::new();
        for instance in self.instances_from_kv().await? {
            if let ScopeRef::Agent { agent_id } = instance.scope {
                by_agent
                    .entry(agent_id)
                    .or_insert_with(Vec::new)
                    .push(instance.instance_id);
            }
        }
        Ok(by_agent
            .into_iter()
            .map(|(agent_id, instance_ids)| {
                serde_json::json!({ "agent_id": agent_id, "instance_ids": instance_ids })
            })
            .collect())
    }
}
