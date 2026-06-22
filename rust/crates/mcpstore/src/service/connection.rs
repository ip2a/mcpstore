use crate::store::prelude::*;

impl MCPStore {
    pub async fn connect_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceConnectRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }
        self.connect_service_internal(name, false).await
    }

    pub(crate) async fn connect_service_internal(
        &self,
        name: &str,
        automatic_retry: bool,
    ) -> Result<()> {
        if self.registry.find_service(name).await.is_none() {
            return Err(StoreError::ServiceNotFound(name.to_string()));
        }
        if self.pool.is_connected(name).await {
            self.registry
                .update_status(name, ConnectionStatus::Connected)
                .await;
            return Ok(());
        }

        let retry_state = self.sync_retry_window(name).await?;
        let now = Self::now_timestamp_f64();
        if automatic_retry {
            if let Some(status) = retry_state.as_ref() {
                if Self::retry_exhausted(status, now) {
                    return Err(StoreError::Other(format!(
                        "Service automatic retry exhausted: {name}"
                    )));
                }
                if let Some(retry_in_secs) = Self::retry_wait_seconds(status, now) {
                    return Err(StoreError::Other(format!(
                        "Service reconnect backoff active: {name}, retry_in={retry_in_secs}s"
                    )));
                }
            }
        }

        self.registry
            .update_status(name, ConnectionStatus::Connecting)
            .await;
        let previous_status = retry_state
            .as_ref()
            .map(|status| status.health_status.clone());
        let mut startup = retry_state.unwrap_or_else(|| {
            self.service_status_payload(name, HealthStatus::Startup, None, Vec::new())
        });
        startup.health_status = HealthStatus::Startup;
        startup.last_health_check = Self::now_timestamp();
        startup.next_retry_time = None;
        startup.lease_deadline = if matches!(previous_status, Some(HealthStatus::HalfOpen)) {
            Some(now + self.runtime_config.half_open_lease_secs as f64)
        } else {
            None
        };
        self.put_service_status_payload(&startup).await?;
        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_CONNECTION_REQUESTED",
                    serde_json::json!({ "name": name }),
                ),
                true,
            )
            .await;

        if let Err(error) = self.pool.connect(name).await {
            let message = format!("Connection failed: {error}");
            self.pool.disconnect(name).await.ok();
            self.registry
                .update_status(name, ConnectionStatus::Error)
                .await;
            self.mark_service_retryable_failure(name, message.clone())
                .await?;
            return Err(error.into());
        }
        self.registry
            .update_status(name, ConnectionStatus::Connected)
            .await;

        let tools = match self.pool.list_tools(name).await {
            Ok(tools) => tools,
            Err(error) => {
                let message = format!("Tool discovery failed: {error}");
                self.pool.disconnect(name).await.ok();
                self.registry
                    .update_status(name, ConnectionStatus::Error)
                    .await;
                self.mark_service_retryable_failure(name, message.clone())
                    .await?;
                return Err(error.into());
            }
        };
        let tool_infos: Vec<crate::registry::ToolInfo> = tools
            .into_iter()
            .map(|tool| crate::registry::ToolInfo {
                name: tool.name,
                description: tool.description,
                schema: tool.input_schema,
            })
            .collect();

        let tool_count = tool_infos.len();
        if let Some(mut entry) = self.registry.find_service(name).await {
            entry.tools = tool_infos;
            entry.status = ConnectionStatus::Connected;
            self.registry.register(entry).await;
        }

        let tools = self.registry.list_service_tools(name).await;
        self.cache_service_connected(name, &tools).await?;

        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_CONNECTED",
                    serde_json::json!({
                        "name": name, "tools_count": tool_count
                    }),
                ),
                true,
            )
            .await;

        tracing::info!("[STORE] Service connected: {} (tools={})", name, tool_count);
        Ok(())
    }

    pub async fn disconnect_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceDisconnectRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.pool.disconnect(name).await?;
        self.registry
            .update_status(name, ConnectionStatus::Disconnected)
            .await;
        self.set_service_status(name, HealthStatus::Disconnected, None, Vec::new())
            .await?;
        self.event_bus
            .publish(
                Event::new("SERVICE_DISCONNECTED", serde_json::json!({ "name": name })),
                true,
            )
            .await;
        tracing::info!("[STORE] Service disconnected: {}", name);
        Ok(())
    }

    pub async fn restart_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceRestartRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.disconnect_service(name).await.ok();
        self.connect_service(name).await
    }
}
