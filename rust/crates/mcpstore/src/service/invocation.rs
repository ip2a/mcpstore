use std::time::Instant;

use crate::store::prelude::*;
use crate::transport::{
    McpExecutionOptions, McpExecutionProgress, McpExecutionUpdate, McpToolExecution,
    McpToolExecutionHandle, ToolCallResult, TransportError,
};

#[derive(Debug)]
pub enum McpStoreExecutionUpdate {
    Progress(McpExecutionProgress),
    Finished(Result<McpToolExecution>),
}

enum McpStoreExecutionInner {
    Transport(McpToolExecutionHandle),
    Ready(Option<Result<McpToolExecution>>),
}

enum ToolExecutionMode {
    Immediate,
    Task { ttl: Option<u64> },
}

struct ToolExecutionContext {
    instance_id: InstanceId,
    service_name: String,
    scope: ScopeRef,
    tool_name: String,
    arguments: serde_json::Value,
    task_tool_name: Option<String>,
    is_openapi_virtual: bool,
    started_at: Instant,
}

pub struct McpStoreToolExecutionHandle<'a> {
    store: &'a MCPStore,
    instance_id: InstanceId,
    inner: McpStoreExecutionInner,
    context: Option<ToolExecutionContext>,
}

impl std::fmt::Debug for McpStoreToolExecutionHandle<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("McpStoreToolExecutionHandle");
        debug.field("instance_id", &self.instance_id);
        match &self.inner {
            McpStoreExecutionInner::Transport(handle) => {
                debug
                    .field("request_id", &handle.request_id())
                    .field("progress_token", &handle.progress_token())
                    .field("supports_cancellation", &true);
            }
            McpStoreExecutionInner::Ready(_) => {
                debug.field("supports_cancellation", &false);
            }
        }
        debug.finish_non_exhaustive()
    }
}

impl<'a> McpStoreToolExecutionHandle<'a> {
    fn transport(
        store: &'a MCPStore,
        handle: McpToolExecutionHandle,
        context: ToolExecutionContext,
    ) -> Self {
        Self {
            store,
            instance_id: context.instance_id,
            inner: McpStoreExecutionInner::Transport(handle),
            context: Some(context),
        }
    }

    fn ready(
        store: &'a MCPStore,
        instance_id: InstanceId,
        result: Result<McpToolExecution>,
    ) -> Self {
        Self {
            store,
            instance_id,
            inner: McpStoreExecutionInner::Ready(Some(result)),
            context: None,
        }
    }

    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    pub fn request_id(&self) -> Option<&serde_json::Value> {
        match &self.inner {
            McpStoreExecutionInner::Transport(handle) => Some(handle.request_id()),
            McpStoreExecutionInner::Ready(_) => None,
        }
    }

    pub fn progress_token(&self) -> Option<&serde_json::Value> {
        match &self.inner {
            McpStoreExecutionInner::Transport(handle) => Some(handle.progress_token()),
            McpStoreExecutionInner::Ready(_) => None,
        }
    }

    pub fn supports_cancellation(&self) -> bool {
        matches!(self.inner, McpStoreExecutionInner::Transport(_))
    }

    pub fn cancel(&mut self, reason: impl Into<String>) -> bool {
        match &mut self.inner {
            McpStoreExecutionInner::Transport(handle) => handle.cancel(reason),
            McpStoreExecutionInner::Ready(_) => false,
        }
    }

    pub async fn next_update(&mut self) -> Option<McpStoreExecutionUpdate> {
        let update = match &mut self.inner {
            McpStoreExecutionInner::Transport(handle) => handle.next_update().await,
            McpStoreExecutionInner::Ready(result) => {
                return result.take().map(McpStoreExecutionUpdate::Finished);
            }
        };

        match update {
            Some(McpExecutionUpdate::Progress(progress)) => {
                Some(McpStoreExecutionUpdate::Progress(progress))
            }
            Some(McpExecutionUpdate::Finished(result)) => {
                let context = self
                    .context
                    .take()
                    .expect("transport execution context must exist until completion");
                Some(McpStoreExecutionUpdate::Finished(
                    self.store
                        .finish_tool_execution(context, result.map_err(StoreError::Transport))
                        .await,
                ))
            }
            None => {
                let context = self.context.take()?;
                let error =
                    TransportError::Protocol("tool execution ended without a result".to_string());
                Some(McpStoreExecutionUpdate::Finished(
                    self.store
                        .finish_tool_execution(context, Err(StoreError::Transport(error)))
                        .await,
                ))
            }
        }
    }

    pub async fn wait(mut self) -> Result<McpToolExecution> {
        while let Some(update) = self.next_update().await {
            if let McpStoreExecutionUpdate::Finished(result) = update {
                return result;
            }
        }
        Err(StoreError::Transport(TransportError::Protocol(
            "tool execution ended without a result".to_string(),
        )))
    }
}

impl MCPStore {
    pub async fn start_tool_execution(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
        options: McpExecutionOptions,
    ) -> Result<McpStoreToolExecutionHandle<'_>> {
        self.start_tool_execution_inner(
            instance_id,
            tool_name,
            args,
            ToolExecutionMode::Immediate,
            options,
        )
        .await
    }

    pub(crate) async fn start_task_tool_execution(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
        ttl: Option<u64>,
        options: McpExecutionOptions,
    ) -> Result<McpStoreToolExecutionHandle<'_>> {
        self.start_tool_execution_inner(
            instance_id,
            tool_name,
            args,
            ToolExecutionMode::Task { ttl },
            options,
        )
        .await
    }

    async fn start_tool_execution_inner(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
        mode: ToolExecutionMode,
        options: McpExecutionOptions,
    ) -> Result<McpStoreToolExecutionHandle<'_>> {
        self.refresh_from_db_if_needed().await?;
        let requested_instance_id = instance_id;
        self.ensure_context_tool_allowed(requested_instance_id, tool_name)
            .await?;
        self.ensure_instance_connected(requested_instance_id)
            .await?;
        let (instance_id, tool_name, args) = self
            .resolve_transformed_tool_call(requested_instance_id, tool_name, args)
            .await?;
        self.ensure_context_tool_allowed(instance_id, &tool_name)
            .await?;
        if instance_id != requested_instance_id {
            self.ensure_instance_connected(instance_id).await?;
        }
        let instance = self
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(instance_id.to_string()))?;
        let is_openapi_virtual = self.is_openapi_virtual_instance(instance_id).await?;
        if matches!(mode, ToolExecutionMode::Task { .. }) && is_openapi_virtual {
            return Err(StoreError::Transport(
                TransportError::CapabilityUnsupported {
                    instance_id,
                    capability: "tasks",
                },
            ));
        }
        let context = ToolExecutionContext {
            instance_id,
            service_name: instance.service_name,
            scope: instance.scope,
            tool_name: tool_name.clone(),
            arguments: args.clone(),
            task_tool_name: matches!(mode, ToolExecutionMode::Task { .. })
                .then(|| tool_name.clone()),
            is_openapi_virtual,
            started_at: Instant::now(),
        };

        if is_openapi_virtual {
            let result = self
                .call_openapi_virtual_tool(instance_id, &tool_name, args)
                .await
                .map(|result| McpToolExecution::Immediate { result });
            let result = self.finish_tool_execution(context, result).await;
            return Ok(McpStoreToolExecutionHandle::ready(
                self,
                instance_id,
                result,
            ));
        }

        let started = match mode {
            ToolExecutionMode::Task { ttl } => {
                self.pool
                    .start_task_tool_execution(instance_id, &tool_name, args, ttl, options)
                    .await
            }
            ToolExecutionMode::Immediate => {
                self.pool
                    .start_tool_execution(instance_id, &tool_name, args, options)
                    .await
            }
        };
        match started {
            Ok(handle) => Ok(McpStoreToolExecutionHandle::transport(
                self, handle, context,
            )),
            Err(error) => {
                let result = self
                    .finish_tool_execution(context, Err(StoreError::Transport(error)))
                    .await;
                Ok(McpStoreToolExecutionHandle::ready(
                    self,
                    instance_id,
                    result,
                ))
            }
        }
    }

    pub async fn call_tool(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<ToolCallResult> {
        match self
            .start_tool_execution(instance_id, tool_name, args, McpExecutionOptions::default())
            .await?
            .wait()
            .await?
        {
            McpToolExecution::Immediate { result } => Ok(result),
            McpToolExecution::Task { .. } => Err(StoreError::Transport(TransportError::Protocol(
                "tool call unexpectedly returned a task".to_string(),
            ))),
        }
    }

    async fn finish_tool_execution(
        &self,
        context: ToolExecutionContext,
        result: Result<McpToolExecution>,
    ) -> Result<McpToolExecution> {
        let latency_ms = context.started_at.elapsed().as_secs_f64() * 1000.0;
        match result {
            Ok(execution) => {
                if let McpToolExecution::Task { task } = &execution {
                    self.pool
                        .observe_tool_task(
                            context.instance_id,
                            task.clone(),
                            context.task_tool_name.as_deref(),
                        )
                        .await?;
                }
                if context.is_openapi_virtual {
                    self.record_openapi_availability(
                        context.instance_id,
                        true,
                        Some(latency_ms),
                        None,
                    )
                    .await?;
                } else {
                    self.record_tool_observation(context.instance_id, true, Some(latency_ms), None)
                        .await?;
                }
                let (is_error, status, task_id) = match &execution {
                    McpToolExecution::Immediate { result } => (
                        result.is_error,
                        if result.is_error { "error" } else { "success" },
                        None,
                    ),
                    McpToolExecution::Task { task } => {
                        (false, "task_created", Some(task.task_id.as_str()))
                    }
                };
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_COMPLETED",
                            serde_json::json!({
                                "instance_id": context.instance_id,
                                "service_name": context.service_name,
                                "scope": context.scope,
                                "tool_name": context.tool_name,
                                "arguments": context.arguments,
                                "latency_ms": latency_ms,
                                "is_error": is_error,
                                "status": status,
                                "task_id": task_id,
                            }),
                        ),
                        true,
                    )
                    .await;
                Ok(execution)
            }
            Err(error) => {
                let status = if context.is_openapi_virtual {
                    self.record_openapi_availability(
                        context.instance_id,
                        false,
                        Some(latency_ms),
                        Some(format!("OpenAPI tool call failed: {error}")),
                    )
                    .await?;
                    "error"
                } else {
                    match &error {
                        StoreError::Transport(transport_error) => {
                            if execution_failure_impairs_connection(transport_error) {
                                self.pool.disconnect(context.instance_id).await.ok();
                                self.record_transport_failure(
                                    context.instance_id,
                                    transport_error,
                                    "Tool call failed",
                                )
                                .await?;
                            }
                            execution_failure_status(transport_error)
                        }
                        _ => "error",
                    }
                };
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_FAILED",
                            serde_json::json!({
                                "instance_id": context.instance_id,
                                "service_name": context.service_name,
                                "scope": context.scope,
                                "tool_name": context.tool_name,
                                "arguments": context.arguments,
                                "latency_ms": latency_ms,
                                "is_error": true,
                                "status": status,
                                "error": error.to_string(),
                            }),
                        ),
                        true,
                    )
                    .await;
                Err(error)
            }
        }
    }

    async fn call_openapi_virtual_tool(
        &self,
        instance_id: InstanceId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<ToolCallResult> {
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

fn execution_failure_impairs_connection(error: &TransportError) -> bool {
    !matches!(
        error,
        TransportError::CapabilityUnsupported { .. }
            | TransportError::RequestCancelled { .. }
            | TransportError::RequestTimedOut { .. }
            | TransportError::TaskNotFound { .. }
            | TransportError::TaskState(_)
    )
}

fn execution_failure_status(error: &TransportError) -> &'static str {
    match error {
        TransportError::RequestCancelled { .. } => "cancelled",
        TransportError::RequestTimedOut { .. } => "timed_out",
        TransportError::RequestDisconnected { .. } => "disconnected",
        _ => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_timeout_and_capability_errors_keep_connections_healthy() {
        let instance_id = ServiceInstanceKey::new("execution-test", ScopeRef::Store).instance_id();
        assert!(!execution_failure_impairs_connection(
            &TransportError::RequestCancelled { reason: None }
        ));
        assert!(!execution_failure_impairs_connection(
            &TransportError::RequestTimedOut {
                timeout: std::time::Duration::from_secs(1),
            }
        ));
        assert!(!execution_failure_impairs_connection(
            &TransportError::CapabilityUnsupported {
                instance_id,
                capability: "tools",
            }
        ));
        assert!(execution_failure_impairs_connection(
            &TransportError::RequestDisconnected { instance_id }
        ));
    }

    #[test]
    fn execution_failure_statuses_are_stable() {
        let instance_id = ServiceInstanceKey::new("execution-test", ScopeRef::Store).instance_id();
        assert_eq!(
            execution_failure_status(&TransportError::RequestCancelled { reason: None }),
            "cancelled"
        );
        assert_eq!(
            execution_failure_status(&TransportError::RequestTimedOut {
                timeout: std::time::Duration::from_secs(1),
            }),
            "timed_out"
        );
        assert_eq!(
            execution_failure_status(&TransportError::RequestDisconnected { instance_id }),
            "disconnected"
        );
        assert_eq!(
            execution_failure_status(&TransportError::Protocol("bad response".to_string())),
            "error"
        );
    }
}
