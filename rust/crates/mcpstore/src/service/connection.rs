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
        if self.registry.find_service(name).await.is_none() {
            return Err(StoreError::ServiceNotFound(name.to_string()));
        }
        self.clear_lifecycle_manual_stop(name).await?;
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
        if self.is_openapi_virtual_service(name).await? {
            let import = self
                .get_openapi_import(name)
                .await?
                .ok_or_else(|| StoreError::Other(format!("OpenAPI import not found: {name}")))?;
            let tools = crate::openapi_runtime::openapi_tool_infos(&import);
            let tool_count = tools.len();
            if let Some(mut entry) = self.registry.find_service(name).await {
                entry.tools = tools;
                entry.status = ConnectionStatus::Connected;
                self.registry.register(entry).await;
            }
            self.event_bus
                .publish(
                    Event::new(
                        "SERVICE_CONNECTED",
                        serde_json::json!({
                            "name": name, "tools_count": tool_count, "transport": "openapi"
                        }),
                    ),
                    true,
                )
                .await;
            self.record_health_check_result(name, true, None, None)
                .await?;
            return Ok(());
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
                let lifecycle = self.resolved_service_lifecycle(name).await?;
                if status.current_error.is_some()
                    && !lifecycle.restart_policy.should_restart_after_failure(
                        status.lifecycle_state.restart_attempts.max(1),
                    )
                {
                    return Err(StoreError::Other(format!(
                        "Service automatic restart disabled by restart_policy: {name}"
                    )));
                }
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

        let connect_timeout = std::time::Duration::from_secs(
            self.runtime_config
                .connect_timeout_secs
                .try_into()
                .unwrap_or(1),
        );
        let connect_result: Result<()> =
            match tokio::time::timeout(connect_timeout, self.pool.connect(name)).await {
                Ok(result) => result.map_err(Into::into),
                Err(_) => Err(StoreError::Other(format!(
                    "Service connection timed out: {name}, timeout={}s",
                    self.runtime_config.connect_timeout_secs
                ))),
            };
        if let Err(error) = connect_result {
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
}
