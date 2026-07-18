use crate::state::{HealthMetrics, HealthState, RecoveryState, RuntimePhase, ServiceStateEvent};
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
            let state = self
                .state_manager
                .get(instance_id)
                .await?
                .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
            if automatic_retry && state.phase == RuntimePhase::Running {
                return Ok(());
            }
            if automatic_retry {
                self.state_manager
                    .dispatch(
                        instance_id,
                        ServiceStateEvent::RecoveryProbeStarted {
                            attempt: match state.recovery {
                                RecoveryState::Waiting { attempt, .. } => attempt,
                                _ => {
                                    return Err(StoreError::Other(format!(
                                        "Service instance has no scheduled recovery: {instance_id}"
                                    )));
                                }
                            },
                        },
                        Self::now_timestamp(),
                    )
                    .await?;
            } else {
                self.state_manager
                    .dispatch(
                        instance_id,
                        ServiceStateEvent::StartRequested,
                        Self::now_timestamp(),
                    )
                    .await?;
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
            self.state_manager
                .dispatch(
                    instance_id,
                    ServiceStateEvent::TransportConnected,
                    Self::now_timestamp(),
                )
                .await?;
            self.state_manager
                .dispatch(
                    instance_id,
                    ServiceStateEvent::ToolSyncStarted,
                    Self::now_timestamp(),
                )
                .await?;
            self.state_manager
                .dispatch(
                    instance_id,
                    ServiceStateEvent::ToolSyncSucceeded {
                        tools: tools.iter().map(|tool| tool.name.clone()).collect(),
                    },
                    Self::now_timestamp(),
                )
                .await?;
            let mut updated = instance.clone();
            updated.tools = tools;
            self.applied_openapi_configs
                .write()
                .await
                .insert(instance_id, instance.effective_config.clone());
            self.registry.register_instance(updated).await;
            self.mark_instance_applied(instance_id).await?;
            let tools = self.registry.list_instance_tools(instance_id).await;
            self.cache_instance_connected(instance_id, &tools).await?;
            self.record_openapi_availability(instance_id, true, None, None)
                .await?;
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
            return Ok(());
        }
        let service_state = self
            .state_manager
            .get(instance_id)
            .await?
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let now = Self::now_timestamp_f64();
        if automatic_retry && self.pool.is_connected(instance_id).await {
            if service_state.phase != RuntimePhase::Running {
                self.state_manager
                    .dispatch(
                        instance_id,
                        ServiceStateEvent::TransportConnected,
                        Self::now_timestamp(),
                    )
                    .await?;
            }
            return Ok(());
        }
        if automatic_retry {
            if Self::retry_exhausted(&service_state, now) {
                return Err(StoreError::Other(format!(
                    "Service instance automatic retry exhausted: {instance_id}"
                )));
            }
            if let Some(retry_in_secs) = Self::retry_wait_seconds(&service_state, now) {
                return Err(StoreError::Other(format!(
                    "Service instance reconnect backoff active: {instance_id}, retry_in={retry_in_secs}s"
                )));
            }
        }
        if automatic_retry {
            match service_state.recovery {
                RecoveryState::Waiting { attempt, .. } => {
                    self.state_manager
                        .dispatch(
                            instance_id,
                            ServiceStateEvent::RecoveryProbeStarted { attempt },
                            Self::now_timestamp(),
                        )
                        .await?;
                }
                RecoveryState::Idle
                    if service_state.desired == crate::state::DesiredState::Stopped =>
                {
                    self.state_manager
                        .dispatch(
                            instance_id,
                            ServiceStateEvent::StartRequested,
                            Self::now_timestamp(),
                        )
                        .await?;
                }
                RecoveryState::Idle | RecoveryState::Probing { .. } => {}
                RecoveryState::Exhausted { .. } => {
                    unreachable!("exhausted recovery rejected above")
                }
            }
        } else {
            self.state_manager
                .dispatch(
                    instance_id,
                    ServiceStateEvent::StartRequested,
                    Self::now_timestamp(),
                )
                .await?;
        }

        let probe_runner = std::sync::Arc::new(self.pool.clone());
        let ping_timeout_secs = match instance.transport.as_str() {
            "stdio" => self.runtime_config.ping_timeout_stdio_secs,
            _ => self.runtime_config.ping_timeout_http_secs,
        };
        if let Some(supervisor) = &self.supervisor {
            supervisor.reset(instance_id).await;
            supervisor.register(instance_id).await;
            supervisor
                .start_health_worker(
                    probe_runner.clone(),
                    instance_id,
                    std::time::Duration::from_secs_f64(self.runtime_config.liveness_interval_secs),
                    std::time::Duration::from_secs_f64(ping_timeout_secs),
                )
                .await;
        }
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
                    self.record_instance_failure(
                        instance_id,
                        format!("Connection failed: {error}"),
                    )
                    .await?;
                }
            }
            return Err(error);
        }
        self.state_manager
            .dispatch(
                instance_id,
                ServiceStateEvent::TransportConnected,
                Self::now_timestamp(),
            )
            .await?;

        // Run startup probe before declaring the service connected. For OpenAPI virtual
        // instances this is skipped; availability is determined by HTTP requests.
        if !self.is_openapi_virtual_instance(instance_id).await? {
            if let Some(supervisor) = &self.supervisor {
                match supervisor
                    .run_startup_probe(probe_runner, instance_id)
                    .await
                {
                    super::super::health::supervisor::StartupOutcome::Healthy => {}
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

        self.state_manager
            .dispatch(
                instance_id,
                ServiceStateEvent::HealthObserved {
                    health: HealthState::Healthy,
                    metrics: HealthMetrics::default(),
                    failure: None,
                },
                Self::now_timestamp(),
            )
            .await?;
        self.state_manager
            .dispatch(
                instance_id,
                ServiceStateEvent::ToolSyncStarted,
                Self::now_timestamp(),
            )
            .await?;

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
        self.state_manager
            .dispatch(
                instance_id,
                ServiceStateEvent::ToolSyncSucceeded {
                    tools: tool_infos.iter().map(|tool| tool.name.clone()).collect(),
                },
                Self::now_timestamp(),
            )
            .await?;

        let mut updated = instance.clone();
        updated.tools = tool_infos;
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
