use crate::state::{HealthMetrics, HealthState, RecoveryState, RuntimePhase, ServiceStateEvent};
use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn connect_service_internal(
        &self,
        instance_id: InstanceId,
        automatic_retry: bool,
    ) -> Result<()> {
        let instance = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        if self.is_openapi_virtual_instance(instance_id).await? {
            let state = self
                .kernel
                .control
                .state
                .get(instance_id)
                .await?
                .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
            if automatic_retry && state.phase == RuntimePhase::Running {
                return Ok(());
            }
            if automatic_retry {
                self.kernel
                    .control
                    .state
                    .dispatch(
                        instance_id,
                        ServiceStateEvent::RecoveryProbeStarted {
                            attempt: match state.recovery {
                                RecoveryState::Waiting { attempt, .. } => attempt,
                                _ => {
                                    return Err(Error::new(
                                        FailureCode::Internal,
                                        format!(
                                        "Service instance has no scheduled recovery: {instance_id}"
                                    ),
                                    ));
                                }
                            },
                        },
                        Self::now_timestamp(),
                    )
                    .await?;
            } else {
                self.kernel
                    .control
                    .state
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
                    Error::new(
                        FailureCode::Internal,
                        format!("OpenAPI import not found for instance {instance_id}"),
                    )
                })?;
            let tools = crate::openapi_runtime::openapi_tool_infos(&import);
            let tool_count = tools.len();
            self.kernel
                .control
                .state
                .dispatch(
                    instance_id,
                    ServiceStateEvent::TransportConnected,
                    Self::now_timestamp(),
                )
                .await?;
            self.kernel
                .control
                .state
                .dispatch(
                    instance_id,
                    ServiceStateEvent::ToolSyncStarted,
                    Self::now_timestamp(),
                )
                .await?;
            self.kernel
                .control
                .state
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
            self.kernel
                .runtime
                .applied_openapi_configs
                .write()
                .await
                .insert(instance_id, instance.effective_config.clone());
            self.kernel
                .control
                .registry
                .register_instance(updated)
                .await;
            self.mark_instance_applied(instance_id).await?;
            self.kernel
                .runtime
                .local_connections
                .write()
                .await
                .insert(instance_id);
            let tools = self
                .kernel
                .control
                .registry
                .list_instance_tools(instance_id)
                .await;
            self.upsert_instance_tools_projection(instance_id, &tools)
                .await?;

            // scopes.toml tool/policy entries skip never-connected instances at
            // boot ("no synced tools yet"), which made override persistence
            // require a manual /config/reload after every restart. Tools are
            // synced now — replay the scopes config so file-declared overrides
            // land. Idempotent (set_* upserts + journal reconciliation); the
            // gateway's boot connect runs through this same path, so its private
            // KV is seeded too. Never fail the connect over a reload hiccup.
            if let Err(error) = self.load_scopes_config().await {
                tracing::warn!("scopes.toml replay after connect failed: {error}");
            }
            self.kernel
                .execution
                .event_bus
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
        // Coalesce concurrent connects of the same instance (parallel gateway
        // mount builds, panel retries, OAuth callbacks): losers wait here,
        // then take the connected fast-path below instead of removing and
        // rebuilding the winner's just-created pool entry.
        //
        // Storm control: the hard timeout bounds the WHOLE attempt — window
        // queue, gate-held span (OAuth discovery, transport connect, startup
        // probe, tool sync) — so a batch boot always drains to terminal
        // states. The cleanup after a timeout is bounded too: the failure
        // paths below park forever when a quit streamable-http worker swallows
        // the disconnect ack (live wedge: linear stayed `starting` for hours
        // because `record_failure` never ran after `connect failed`).
        let result = tokio::time::timeout(
            crate::transport::client::ConnectionPool::connect_hard_timeout(),
            async {
                tracing::info!(%instance_id, "[CONNECT] queued for the connect window");
                // Underscore binding: unnamed drops immediately, plain name
                // warns — this one must live until the block ends.
                let _permit = self
                    .kernel
                    .execution
                    .pool
                    .connect_window()
                    .acquire_owned()
                    .await
                    .map_err(|_| {
                        Error::new(FailureCode::Internal, "connection pool is shut down")
                    })?;
                tracing::info!(%instance_id, "[CONNECT] window permit acquired");
                self.connect_wired_instance(instance_id, instance, automatic_retry)
                    .await
            },
        )
        .await;
        match result {
            Ok(inner) => inner,
            Err(_) => {
                let budget = crate::transport::client::ConnectionPool::connect_hard_timeout();
                tracing::error!(
                    %instance_id,
                    budget_secs = budget.as_secs(),
                    "[CONNECT] hard timeout: connect span exceeded the budget; recording terminal failure"
                );
                let error = Error::new(
                    FailureCode::ConnectionTimedOut,
                    format!(
                        "service instance connect hard timeout: {instance_id}, budget={}s",
                        budget.as_secs()
                    ),
                );
                let cleanup = tokio::time::timeout(budget, async {
                    self.kernel
                        .execution
                        .pool
                        .disconnect(instance_id)
                        .await
                        .ok();
                    if let Err(record_error) = self.record_failure(instance_id, &error).await {
                        // A rejected transition (e.g. desired stopped) must not
                        // mask the real cause: the caller still gets the timeout.
                        tracing::warn!(
                            %instance_id,
                            "[CONNECT] could not record hard-timeout failure: {record_error}"
                        );
                    }
                })
                .await;
                if cleanup.is_err() {
                    tracing::error!(
                        %instance_id,
                        "[CONNECT] hard-timeout cleanup itself exceeded the budget; terminal failure returned without state record"
                    );
                }
                Err(error)
            }
        }
    }

    /// The gate-held connect span for a wired (non-OpenAPI) instance:
    /// same-instance coalescing, state transitions, transport connect,
    /// startup probe, tool sync. Caller holds a connect-window permit.
    async fn connect_wired_instance(
        &self,
        instance_id: InstanceId,
        instance: crate::registry::ServiceInstance,
        automatic_retry: bool,
    ) -> Result<()> {
        let connect_gate = self.kernel.execution.pool.connect_guard(instance_id).await;
        let _connect_gate = connect_gate.lock().await;
        let service_state = self
            .kernel
            .control
            .state
            .get(instance_id)
            .await?
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        let now = Self::now_timestamp_f64();
        if self.kernel.execution.pool.is_connected(instance_id).await {
            if service_state.phase != RuntimePhase::Running {
                self.kernel
                    .control
                    .state
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
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("Service instance automatic retry exhausted: {instance_id}"),
                ));
            }
            if let Some(retry_in_secs) = Self::retry_wait_seconds(&service_state, now) {
                return Err(Error::new(FailureCode::Internal, format!(
                    "Service instance reconnect backoff active: {instance_id}, retry_in={retry_in_secs}s"
                )));
            }
        }
        if automatic_retry {
            match service_state.recovery {
                RecoveryState::Waiting { attempt, .. } => {
                    self.kernel
                        .control
                        .state
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
                    self.kernel
                        .control
                        .state
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
            self.kernel
                .control
                .state
                .dispatch(
                    instance_id,
                    ServiceStateEvent::StartRequested,
                    Self::now_timestamp(),
                )
                .await?;
        }

        let probe_runner = std::sync::Arc::new(self.kernel.execution.pool.clone());
        let ping_timeout_secs = self
            .kernel
            .runtime
            .runtime_config
            .ping_timeout_for_transport(instance.transport.as_str());
        if let Some(supervisor) = self.kernel.execution.supervisor.get() {
            supervisor.reset(instance_id).await;
            supervisor.register(instance_id).await;
            supervisor
                .start_health_worker(
                    probe_runner.clone(),
                    instance_id,
                    std::time::Duration::from_secs_f64(
                        self.kernel.runtime.runtime_config.liveness_interval_secs,
                    ),
                    std::time::Duration::from_secs_f64(ping_timeout_secs),
                )
                .await;
        }
        self.kernel
            .execution
            .event_bus
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

        // A pre-flight auth failure (e.g. the endpoint answers 401 with a Bearer
        // challenge) must surface as a terminal service failure. Swallowing it
        // here leaves the instance in `starting` forever with no evidence.
        if let Err(error) = self.ensure_http_oauth_config(instance_id).await {
            self.kernel
                .execution
                .pool
                .disconnect(instance_id)
                .await
                .ok();
            self.record_failure(instance_id, &error).await?;
            return Err(error);
        }
        let instance = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;

        let connect_timeout = std::time::Duration::from_secs(
            self.kernel
                .runtime
                .runtime_config
                .connect_timeout_for_transport(instance.transport.as_str())
                .try_into()
                .unwrap_or(1),
        );
        let mut transport_config: ServerConfig = match serde_json::from_value(
            serde_json::Value::Object(instance.effective_config.clone()),
        ) {
            Ok(config) => config,
            Err(error) => {
                let error = Error::new(
                    FailureCode::Internal,
                    format!(
                        "Effective config for instance {instance_id} cannot be decoded: {error}"
                    ),
                );
                self.kernel
                    .execution
                    .pool
                    .disconnect(instance_id)
                    .await
                    .ok();
                self.record_failure(instance_id, &error).await?;
                return Err(error);
            }
        };
        // effective_config strips _mcpstore, but the transport needs the
        // definition-level handshake_mode to pick the right lifecycle mode.
        if let Some(definition) = self
            .kernel
            .control
            .registry
            .find_definition(&instance.service_name)
            .await
        {
            if let Some(mode) = definition.handshake_mode {
                let extension = transport_config
                    .mcpstore
                    .get_or_insert_with(Default::default);
                extension.handshake_mode = Some(mode);
            }
        }
        self.kernel.execution.pool.remove(instance_id).await.ok();
        self.kernel
            .execution
            .pool
            .add(instance_id, transport_config)
            .await;
        let connect_result: Result<()> = match tokio::time::timeout(
            connect_timeout,
            self.kernel.execution.pool.connect(instance_id),
        )
        .await
        {
            Ok(result) => result.map_err(Into::into),
            Err(_) => Err(Error::new(
                FailureCode::ConnectionTimedOut,
                format!(
                    "service instance connection timed out: {instance_id}, timeout={}s",
                    connect_timeout.as_secs()
                ),
            )),
        };
        if let Err(error) = connect_result {
            self.kernel
                .execution
                .pool
                .disconnect(instance_id)
                .await
                .ok();
            self.record_failure(instance_id, &error).await?;
            return Err(error);
        }
        self.kernel
            .control
            .state
            .dispatch(
                instance_id,
                ServiceStateEvent::TransportConnected,
                Self::now_timestamp(),
            )
            .await?;

        // Run startup probe before declaring the service connected. For OpenAPI virtual
        // instances this is skipped; availability is determined by HTTP requests.
        if !self.is_openapi_virtual_instance(instance_id).await? {
            if let Some(supervisor) = self.kernel.execution.supervisor.get() {
                match supervisor
                    .run_startup_probe(probe_runner, instance_id)
                    .await
                {
                    super::super::health::supervisor::StartupOutcome::Healthy => {}
                    super::super::health::supervisor::StartupOutcome::TimedOut => {
                        tracing::warn!(
                            %instance_id,
                            "startup probe timed out; failing service connect"
                        );
                        let error = Error::new(
                            FailureCode::ProbeTimedOut,
                            format!("service instance startup probe timed out: {instance_id}"),
                        );
                        self.record_failure(instance_id, &error).await?;
                        return Err(error);
                    }
                }
            }
        }

        self.kernel
            .control
            .state
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
        self.kernel
            .control
            .state
            .dispatch(
                instance_id,
                ServiceStateEvent::ToolSyncStarted,
                Self::now_timestamp(),
            )
            .await?;

        let tool_discovery = match self
            .kernel
            .execution
            .pool
            .server_metadata(instance_id)
            .await
        {
            Ok(Some(metadata)) if metadata.capabilities.tools => {
                self.kernel.execution.pool.list_tools(instance_id).await
            }
            Ok(Some(_)) => Ok(Vec::new()),
            Ok(None) => Err(crate::error::Error::new(
                crate::error::FailureCode::NotConnected,
                instance_id.to_string(),
            )),
            Err(error) => Err(error),
        };
        let tools = match tool_discovery {
            Ok(tools) => tools,
            Err(error) => {
                self.kernel
                    .execution
                    .pool
                    .disconnect(instance_id)
                    .await
                    .ok();
                self.record_failure(instance_id, &error).await?;
                return Err(error);
            }
        };
        let tool_infos: Vec<crate::registry::ToolInfo> =
            tools.into_iter().map(Into::into).collect();
        let tool_count = tool_infos.len();
        self.kernel
            .control
            .state
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
        self.kernel
            .control
            .registry
            .register_instance(updated)
            .await;
        self.mark_instance_applied(instance_id).await?;
        self.kernel
            .runtime
            .local_connections
            .write()
            .await
            .insert(instance_id);

        let tools = self
            .kernel
            .control
            .registry
            .list_instance_tools(instance_id)
            .await;
        self.upsert_instance_tools_projection(instance_id, &tools)
            .await?;

        // scopes.toml tool/policy entries skip never-connected instances at
        // boot ("no synced tools yet"), which made override persistence
        // require a manual /config/reload after every restart. Tools are
        // synced now — replay the scopes config so file-declared overrides
        // land. Idempotent (set_* upserts + journal reconciliation); the
        // gateway's boot connect runs through this same path, so its private
        // KV is seeded too. Never fail the connect over a reload hiccup.
        if let Err(error) = self.load_scopes_config().await {
            tracing::warn!("scopes.toml replay after connect failed: {error}");
        }

        self.kernel
            .execution
            .event_bus
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
