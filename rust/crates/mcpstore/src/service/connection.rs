use crate::store::prelude::*;

impl MCPStore {
    pub async fn connect_service(&self, instance_id: InstanceId) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_control_request(
                    "ServiceConnectRequested",
                    serde_json::json!({ "instance_id": instance_id }),
                )
                .await;
        }
        if self.registry.find_instance(instance_id).await.is_none() {
            return Err(StoreError::ServiceNotFound(instance_id.to_string()));
        }
        self.clear_lifecycle_manual_stop(instance_id).await?;
        self.connect_service_internal(instance_id, false).await
    }

    pub(crate) async fn connect_service_internal(
        &self,
        instance_id: InstanceId,
        automatic_retry: bool,
    ) -> Result<()> {
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        if self.is_openapi_virtual_instance(instance_id).await? {
            if automatic_retry && instance.status == ConnectionStatus::Connected {
                return Ok(());
            }
            let import = self
                .get_openapi_import(&instance.service_name)
                .await?
                .ok_or_else(|| {
                    StoreError::Other(format!(
                        "OpenAPI import not found for instance {instance_id}"
                    ))
                })?;
            let tools = crate::openapi_runtime::openapi_tool_infos(&import);
            let tool_count = tools.len();
            let mut updated = instance.clone();
            updated.tools = tools;
            updated.status = ConnectionStatus::Connected;
            self.applied_openapi_configs
                .write()
                .await
                .insert(instance_id, instance.effective_config.clone());
            self.registry.register_instance(updated).await;
            self.mark_instance_applied(instance_id).await?;
            let tools = self.registry.list_instance_tools(instance_id).await;
            self.cache_instance_connected(instance_id, &tools).await?;
            self.event_bus
                .publish(
                    Event::new(
                        "SERVICE_CONNECTED",
                        serde_json::json!({
                            "instance_id": instance_id,
                            "service_name": instance.service_name,
                            "scope": instance.scope,
                            "tools_count": tool_count,
                            "transport": "openapi"
                        }),
                    ),
                    true,
                )
                .await;
            self.record_openapi_availability(instance_id, true, None, None)
                .await?;
            return Ok(());
        }
        if automatic_retry && self.pool.is_connected(instance_id).await {
            self.registry
                .update_status(instance_id, ConnectionStatus::Connected)
                .await;
            return Ok(());
        }

        let retry_state = self.cached_instance_status(instance_id).await?;
        let now = Self::now_timestamp_f64();
        if automatic_retry {
            if let Some(status) = retry_state.as_ref() {
                let lifecycle = self.resolved_instance_lifecycle(instance_id).await?;
                if status.current_error.is_some()
                    && !lifecycle.restart_policy.should_restart_after_failure(
                        status.lifecycle_state.restart_attempts.max(1),
                    )
                {
                    return Err(StoreError::Other(format!(
                        "Service instance automatic restart disabled by restart_policy: {instance_id}"
                    )));
                }
                if Self::retry_exhausted(status, now) {
                    return Err(StoreError::Other(format!(
                        "Service instance automatic retry exhausted: {instance_id}"
                    )));
                }
                if let Some(retry_in_secs) = Self::retry_wait_seconds(status, now) {
                    return Err(StoreError::Other(format!(
                        "Service instance reconnect backoff active: {instance_id}, retry_in={retry_in_secs}s"
                    )));
                }
            }
        }

        self.registry
            .update_status(instance_id, ConnectionStatus::Connecting)
            .await;
        let _previous_status = retry_state
            .as_ref()
            .map(|status| status.health_status.clone());
        let mut startup = match retry_state {
            Some(status) => status,
            None => {
                self.new_instance_status(instance_id, HealthStatus::Startup, None, Vec::new())
                    .await?
            }
        };
        let probe_runner = std::sync::Arc::new(self.pool.clone());
        let ping_timeout_secs = match instance.transport.as_str() {
            "stdio" => self.runtime_config.ping_timeout_stdio_secs,
            _ => self.runtime_config.ping_timeout_http_secs,
        };
        if let Some(supervisor) = &self.supervisor {
            supervisor
                .reset_machine(instance_id, startup.health_status.clone())
                .await;
            supervisor
                .register(instance_id, startup.health_status.clone())
                .await;
            supervisor
                .start_health_worker(
                    probe_runner.clone(),
                    instance_id,
                    std::time::Duration::from_secs_f64(self.runtime_config.liveness_interval_secs),
                    std::time::Duration::from_secs_f64(ping_timeout_secs),
                )
                .await;
        }
        startup.health_status = HealthStatus::Startup;
        startup.last_health_check = Self::now_timestamp();
        startup.next_retry_time = None;
        startup.lease_deadline = None;
        self.put_instance_status(&startup).await?;
        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_CONNECTION_REQUESTED",
                    serde_json::json!({
                        "instance_id": instance_id,
                        "service_name": instance.service_name,
                        "scope": instance.scope,
                    }),
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
        let transport_config: ServerConfig =
            serde_json::from_value(serde_json::Value::Object(instance.effective_config.clone()))
                .map_err(|error| {
                    StoreError::Other(format!(
                        "Effective config for instance {instance_id} cannot be decoded: {error}"
                    ))
                })?;
        self.pool.remove(instance_id).await.ok();
        self.pool.add(instance_id, transport_config).await;
        let connect_result: Result<()> =
            match tokio::time::timeout(connect_timeout, self.pool.connect(instance_id)).await {
                Ok(result) => result.map_err(Into::into),
                Err(_) => Err(StoreError::Other(format!(
                    "Service instance connection timed out: {instance_id}, timeout={}s",
                    self.runtime_config.connect_timeout_secs
                ))),
            };
        if let Err(error) = connect_result {
            self.pool.disconnect(instance_id).await.ok();
            match &error {
                StoreError::Transport(transport_error) => {
                    self.record_transport_failure(
                        instance_id,
                        transport_error,
                        "Connection failed",
                    )
                    .await?;
                }
                _ => {
                    self.registry
                        .update_status(instance_id, ConnectionStatus::Error)
                        .await;
                    self.record_instance_failure(
                        instance_id,
                        format!("Connection failed: {error}"),
                    )
                    .await?;
                }
            }
            return Err(error);
        }
        self.registry
            .update_status(instance_id, ConnectionStatus::Connected)
            .await;

        // Run startup probe before declaring the service connected. For OpenAPI virtual
        // instances this is skipped; availability is determined by HTTP requests.
        if !self.is_openapi_virtual_instance(instance_id).await? {
            if let Some(supervisor) = &self.supervisor {
                match supervisor
                    .run_startup_probe(probe_runner, instance_id)
                    .await
                {
                    super::super::health::supervisor::StartupOutcome::Healthy => {}
                    super::super::health::supervisor::StartupOutcome::Failed(transition) => {
                        let error = format!("Startup probe failed: {}", transition.reason);
                        self.record_instance_failure(instance_id, error).await?;
                        return Err(StoreError::Other(format!(
                            "Service instance startup probe failed: {instance_id}"
                        )));
                    }
                    super::super::health::supervisor::StartupOutcome::TimedOut => {
                        let error = "Startup probe timed out".to_string();
                        self.record_instance_failure(instance_id, error).await?;
                        return Err(StoreError::Other(format!(
                            "Service instance startup probe timed out: {instance_id}"
                        )));
                    }
                }
            }
        }

        let tool_discovery = match self.pool.server_metadata(instance_id).await {
            Ok(Some(metadata)) if metadata.capabilities.tools => {
                self.pool.list_tools(instance_id).await
            }
            Ok(Some(_)) => Ok(Vec::new()),
            Ok(None) => Err(crate::transport::TransportError::NotConnected(
                instance_id.to_string(),
            )),
            Err(error) => Err(error),
        };
        let tools = match tool_discovery {
            Ok(tools) => tools,
            Err(error) => {
                self.pool.disconnect(instance_id).await.ok();
                self.record_transport_failure(instance_id, &error, "Tool discovery failed")
                    .await?;
                return Err(error.into());
            }
        };
        let tool_infos: Vec<crate::registry::ToolInfo> =
            tools.into_iter().map(Into::into).collect();
        let tool_count = tool_infos.len();

        let mut updated = instance.clone();
        updated.tools = tool_infos;
        updated.status = ConnectionStatus::Connected;
        self.registry.register_instance(updated).await;
        self.mark_instance_applied(instance_id).await?;

        let tools = self.registry.list_instance_tools(instance_id).await;
        self.cache_instance_connected(instance_id, &tools).await?;

        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_CONNECTED",
                    serde_json::json!({
                        "instance_id": instance_id,
                        "service_name": instance.service_name,
                        "scope": instance.scope,
                        "tools_count": tool_count
                    }),
                ),
                true,
            )
            .await;

        tracing::info!(
            "[STORE] Service instance connected: {} (service={}, tools={})",
            instance_id,
            instance.service_name,
            tool_count
        );
        Ok(())
    }
}
