//! 数据面板按 placement 拉取服务定义并本地执行。
//!
//! placement 语义（对账文档 D5）：`_mcpstore.placement` 的键是 panel_id，
//! 值是覆盖 base_config 的配置 diff；未出现在 placement 里的服务默认由
//! 控制面板执行。数据面板只执行 placement 命中自己的服务。

use serde_json::{Map, Value};

use crate::store::prelude::*;
use crate::store::MCPStore;

/// 一个命中本面板的 placement 服务及其合并后的执行配置。
pub struct PlacementService {
    pub service_name: String,
    pub instance_id: InstanceId,
    /// merge_config(base_config, placement[panel_id]) 的结果（store scope）。
    pub merged_config: Map<String, Value>,
}

impl MCPStore {
    /// 列出 placement 命中本面板的服务（store scope 实例化）。
    pub async fn panel_services(&self, panel_id: &str) -> Result<Vec<PlacementService>> {
        self.refresh_from_db_if_needed().await?;
        let mut entries = Vec::new();
        for definition in self.kernel.control.registry.list_definitions().await {
            let service_name = definition.service_name.clone();
            let Some(diff) = definition.placement.get(panel_id) else {
                continue;
            };
            let Some(diff_object) = diff.as_object() else {
                return Err(Error::new(
                    FailureCode::ConfigInvalid,
                    format!("placement[{panel_id}] of service '{service_name}' must be an object"),
                ));
            };
            let merged = crate::config::merge_config(&definition.base_config, diff_object);
            entries.push(PlacementService {
                service_name: service_name.clone(),
                instance_id: ServiceInstanceKey::new(service_name, ScopeRef::Store).instance_id(),
                merged_config: merged,
            });
        }
        Ok(entries)
    }

    /// 本地覆盖实例的 effective_config（只动本进程 registry，不写回共享 KV），
    /// 然后建立本地连接。数据面板进程未挂 supervisor，天然跳过 startup
    /// probe / 健康自愈——这就是"拉取已验证配置直接执行"的落地。
    pub(crate) async fn connect_with_local_config(
        &self,
        instance_id: InstanceId,
        merged_config: Map<String, Value>,
    ) -> Result<()> {
        let mut instance = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        instance.effective_config = merged_config;
        self.kernel
            .control
            .registry
            .register_instance(instance)
            .await;
        self.connect_service_internal(instance_id, false).await
    }

    /// 数据面板的 call 路由：placement 命中本面板 → 本地建连执行；
    /// placement 为空 → 远端代理执行；指向其他面板 → 场景 3，明确不支持。
    pub(crate) async fn route_data_panel_tool_call(
        &self,
        panel_id: &str,
        instance_id: InstanceId,
        tool_name: &str,
        args: Value,
    ) -> Result<crate::transport::ToolCallResult> {
        let instance = self
            .instance_from_kv(instance_id)
            .await?
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        let definition = self
            .definition_from_kv(&instance.service_name)
            .await?
            .ok_or_else(|| {
                Error::new(FailureCode::ServiceNotFound, instance.service_name.clone())
            })?;

        if let Some(diff) = definition.placement.get(panel_id) {
            let diff_object = diff.as_object().ok_or_else(|| {
                Error::new(
                    FailureCode::ConfigInvalid,
                    format!(
                        "placement[{panel_id}] of service '{}' must be an object",
                        definition.service_name
                    ),
                )
            })?;
            if !self.kernel.execution.pool.is_connected(instance_id).await {
                // 首次调用才注水一次（连接池 / 状态机需要进程内结构），再用 placement 覆盖建连。
                self.load_from_db().await?;
                let merged = crate::config::merge_config(&definition.base_config, diff_object);
                self.connect_with_local_config(instance_id, merged).await?;
            }
            return self
                .kernel
                .execution
                .call_tool(self, instance_id, tool_name, args)
                .await;
        }

        if definition.placement.is_empty() {
            // 场景 1：控制面板代理执行（kvstore RPC）。
            return self.call_tool_remote(instance_id, tool_name, args).await;
        }

        Err(Error::new(
            FailureCode::ServiceUnavailable,
            format!(
                "service '{}' is placed on another panel; this data panel only executes placement['{panel_id}']",
                instance.service_name
            ),
        ))
    }

    /// 数据面板角色在 load 完成后调用：拉取 placement 命中的服务本地建连。
    /// 单个服务失败不阻断其余，返回成功建立的数量。
    pub(crate) async fn serve_placement(&self, panel_id: &str) -> Result<usize> {
        let services = self.panel_services(panel_id).await?;
        let mut connected = 0;
        for service in services {
            let instance_id = service.instance_id;
            match self
                .connect_with_local_config(instance_id, service.merged_config)
                .await
            {
                Ok(()) => connected += 1,
                Err(error) => tracing::warn!(
                    panel_id = %panel_id,
                    service = %service.service_name,
                    %error,
                    "placement 服务本地执行失败"
                ),
            }
        }
        Ok(connected)
    }
}
