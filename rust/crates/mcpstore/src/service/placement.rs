//! The data panel executes locally per placement.
//!
//! Placement semantics: keys of `_mcpstore.placement` are panel_ids, values override
//! base_config. Services absent from placement are executed by the control panel by
//! default. The data panel only executes services placement assigns to it (lazy connect on call,
//! residency is expressed via the lifecycle config keep_alive / OnStoreStart).

use serde_json::{Map, Value};

use crate::store::prelude::*;
use crate::store::MCPStore;

impl MCPStore {
    /// Locally override the instance's effective_config (this process's registry only, never written back to shared KV),
    /// then connect locally. Data-panel processes have no supervisor mounted, so they skip the startup
    /// probe / health self-healing — this is how "fetch validated config and execute directly" lands.
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

    /// The data panel's single connection entry (ensure_instance_connected routes here):
    /// placement hits this panel → hydrate + merge overrides and connect; otherwise error — non-tool
    /// execution surfaces have no remote RPC yet; fail loudly instead of connecting with base config.
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
        // The pool / state machine need in-process structures; hydrate once, then connect with placement overrides merged in.
        self.load_from_db().await?;
        let merged = crate::config::merge_config(&definition.base_config, diff_object);
        self.connect_with_local_config(instance_id, merged).await
    }

    /// Data-panel call routing: placement hits this panel → local execution (connection goes through
    /// the ensure entry); empty placement → remote proxy; pointing at another panel →
    /// case 3, explicitly unsupported.
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
            // Connect before entering the engine: override resolution validates tool names against synced tools;
            // the other order misreports ToolNotFound.
            self.ensure_data_panel_instance_connected(panel_id, instance_id)
                .await?;
            return self
                .kernel
                .execution
                .call_tool(self, instance_id, tool_name, args)
                .await;
        }

        if definition.placement.is_empty() {
            // Case 1: control-panel proxy execution (kvstore RPC).
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
