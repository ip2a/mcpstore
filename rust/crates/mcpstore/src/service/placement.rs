//! 数据面板按 placement 本地执行。
//!
//! placement 语义：`_mcpstore.placement` 的键是 panel_id，值是覆盖
//! base_config 的配置 diff；未出现在 placement 里的服务默认由控制面板
//! 执行。数据面板只执行 placement 命中自己的服务（调用时 lazy 建连，
//! 常驻语义由生命周期配置 keep_alive / OnStoreStart 表达）。

use serde_json::{Map, Value};

use crate::store::prelude::*;
use crate::store::MCPStore;

impl MCPStore {
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

    /// 数据面板的唯一连接入口（ensure_instance_connected 分流到这里）：
    /// placement 命中本面板 → 注水 + merge 覆盖建连；否则报错——非工具
    /// 执行面目前没有远端 RPC，宁可明确失败也不拿 base 配置瞎连。
    pub(crate) async fn ensure_data_panel_instance_connected(
        &self,
        panel_id: &str,
        instance_id: InstanceId,
    ) -> Result<()> {
        if self.kernel.execution.pool.is_connected(instance_id).await {
            return Ok(());
        }
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
        let Some(diff) = definition.placement.get(panel_id) else {
            return Err(Error::new(
                FailureCode::ServiceUnavailable,
                format!(
                    "service '{}' does not run on this data panel; only tool calls are routed remotely, other surfaces must target the owning panel",
                    instance.service_name
                ),
            ));
        };
        let diff_object = diff.as_object().ok_or_else(|| {
            Error::new(
                FailureCode::ConfigInvalid,
                format!(
                    "placement[{panel_id}] of service '{}' must be an object",
                    definition.service_name
                ),
            )
        })?;
        // 连接池 / 状态机需要进程内结构，注水一次再用 placement 覆盖建连。
        self.load_from_db().await?;
        let merged = crate::config::merge_config(&definition.base_config, diff_object);
        self.connect_with_local_config(instance_id, merged).await
    }

    /// 数据面板的 call 路由：placement 命中本面板 → 本地执行（连接交给
    /// ensure 入口）；placement 为空 → 远端代理执行；指向其他面板 →
    /// 场景 3，明确不支持。
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

        if definition.placement.contains_key(panel_id) {
            // 先建连再进引擎：override 解析按已同步的 tools 校验工具名，
            // 顺序反了会误报 ToolNotFound。
            self.ensure_data_panel_instance_connected(panel_id, instance_id)
                .await?;
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
}
