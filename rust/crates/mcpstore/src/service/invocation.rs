use crate::store::prelude::*;

impl MCPStore {
    pub async fn call_tool(
        &self,
        service_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<crate::transport::ToolCallResult> {
        self.refresh_from_db_if_needed().await?;
        let (service_name, tool_name, args) = self
            .resolve_transformed_tool_call(service_name, tool_name, args)
            .await?;
        let is_openapi_virtual = self.is_openapi_virtual_service(&service_name).await?;
        if is_openapi_virtual {
            self.connect_service_internal(&service_name, true).await?;
        } else if !self.pool.is_connected(&service_name).await {
            self.connect_service_internal(&service_name, true).await?;
        }
        let event_args = args.clone();
        let started_at = std::time::Instant::now();
        if is_openapi_virtual {
            let result = self
                .call_openapi_virtual_tool(&service_name, &tool_name, args)
                .await;
            return match result {
                Ok(result) => {
                    let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                    self.record_health_check_result(&service_name, true, Some(latency_ms), None)
                        .await?;
                    self.event_bus
                        .publish(
                            Event::new(
                                "TOOL_CALL_COMPLETED",
                                serde_json::json!({
                                    "service_name": service_name,
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
                        .update_status(&service_name, ConnectionStatus::Error)
                        .await;
                    self.mark_service_retryable_failure(&service_name, message)
                        .await?;
                    self.event_bus
                        .publish(
                            Event::new(
                                "TOOL_CALL_FAILED",
                                serde_json::json!({
                                    "service_name": service_name,
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
        match self.pool.call_tool(&service_name, &tool_name, args).await {
            Ok(result) => {
                let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                self.record_health_check_result(&service_name, true, Some(latency_ms), None)
                    .await?;
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_COMPLETED",
                            serde_json::json!({
                                "service_name": service_name,
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
                self.pool.disconnect(&service_name).await.ok();
                self.registry
                    .update_status(&service_name, ConnectionStatus::Error)
                    .await;
                self.mark_service_retryable_failure(&service_name, message)
                    .await?;
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_FAILED",
                            serde_json::json!({
                                "service_name": service_name,
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
        service_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<crate::transport::ToolCallResult> {
        let import = self
            .get_openapi_import(service_name)
            .await?
            .ok_or_else(|| {
                StoreError::Other(format!("OpenAPI import not found: {service_name}"))
            })?;
        let options = self.openapi_runtime_options(service_name).await?;
        crate::openapi_runtime::call_openapi_tool(&import, tool_name, args, &options).await
    }
}
