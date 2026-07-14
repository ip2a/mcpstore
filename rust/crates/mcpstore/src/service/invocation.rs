use crate::store::prelude::*;

impl MCPStore {
    pub async fn call_tool(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<crate::transport::ToolCallResult> {
        self.refresh_from_db_if_needed().await?;
        let (instance_id, tool_name, args) = self
            .resolve_transformed_tool_call(instance_id, tool_name, args)
            .await?;
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let is_openapi_virtual = self.is_openapi_virtual_instance(instance_id).await?;
        self.ensure_instance_connected(instance_id).await?;

        let event_args = args.clone();
        let started_at = std::time::Instant::now();
        if is_openapi_virtual {
            let result = self
                .call_openapi_virtual_tool(instance_id, &tool_name, args)
                .await;
            return match result {
                Ok(result) => {
                    let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                    self.record_health_check_result(instance_id, true, Some(latency_ms), None)
                        .await?;
                    self.event_bus
                        .publish(
                            Event::new(
                                "TOOL_CALL_COMPLETED",
                                serde_json::json!({
                                    "instance_id": instance_id,
                                    "service_name": instance.service_name,
                                    "scope": instance.scope,
                                    "tool_name": tool_name,
                                    "arguments": event_args,
                                    "latency_ms": latency_ms,
                                    "is_error": result.is_error,
                                    "status": if result.is_error { "error" } else { "success" },
                                }),
                            ),
                            true,
                        )
                        .await;
                    Ok(result)
                }
                Err(error) => {
                    let message = format!("OpenAPI tool call failed: {error}");
                    let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                    self.registry
                        .update_status(instance_id, ConnectionStatus::Error)
                        .await;
                    self.mark_instance_retryable_failure(instance_id, message)
                        .await?;
                    self.event_bus
                        .publish(
                            Event::new(
                                "TOOL_CALL_FAILED",
                                serde_json::json!({
                                    "instance_id": instance_id,
                                    "service_name": instance.service_name,
                                    "scope": instance.scope,
                                    "tool_name": tool_name,
                                    "arguments": event_args,
                                    "latency_ms": latency_ms,
                                    "is_error": true,
                                    "status": "error",
                                    "error": error.to_string(),
                                }),
                            ),
                            true,
                        )
                        .await;
                    Err(error)
                }
            };
        }

        match self.pool.call_tool(instance_id, &tool_name, args).await {
            Ok(result) => {
                let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                self.record_health_check_result(instance_id, true, Some(latency_ms), None)
                    .await?;
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_COMPLETED",
                            serde_json::json!({
                                "instance_id": instance_id,
                                "service_name": instance.service_name,
                                "scope": instance.scope,
                                "tool_name": tool_name,
                                "arguments": event_args,
                                "latency_ms": latency_ms,
                                "is_error": result.is_error,
                                "status": if result.is_error { "error" } else { "success" },
                            }),
                        ),
                        true,
                    )
                    .await;
                Ok(result)
            }
            Err(error) => {
                let message = format!("Tool call failed: {error}");
                let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                self.pool.disconnect(instance_id).await.ok();
                self.registry
                    .update_status(instance_id, ConnectionStatus::Error)
                    .await;
                self.mark_instance_retryable_failure(instance_id, message)
                    .await?;
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_FAILED",
                            serde_json::json!({
                                "instance_id": instance_id,
                                "service_name": instance.service_name,
                                "scope": instance.scope,
                                "tool_name": tool_name,
                                "arguments": event_args,
                                "latency_ms": latency_ms,
                                "is_error": true,
                                "status": "error",
                                "error": error.to_string(),
                            }),
                        ),
                        true,
                    )
                    .await;
                Err(StoreError::Transport(error))
            }
        }
    }

    async fn call_openapi_virtual_tool(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<crate::transport::ToolCallResult> {
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let import = self
            .get_openapi_import(&instance.service_name)
            .await?
            .ok_or_else(|| {
                StoreError::Other(format!(
                    "OpenAPI import not found for instance {instance_id}"
                ))
            })?;
        let options = self
            .openapi_runtime_options_for_instance(instance_id)
            .await?;
        crate::openapi_runtime::call_openapi_tool(&import, tool_name, args, &options).await
    }
}
