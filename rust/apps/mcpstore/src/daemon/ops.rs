//! 业务 op 分发：daemon socket 与 CLI embedded 进程共用同一份实现
//! （单业务协议、双执行位置）。daemon 管理面（status/config/stop）在 server.rs。
use std::collections::HashSet;

use mcpstore::config::{
    AppConfig, ExecutionPolicy, ExecutionTarget, McpStoreExtension, ScopeDeclarations,
    ScopeDescriptor, ServerConfig,
};
use mcpstore::error::{Error, FailureCode};
use mcpstore::{AuthFlow, InstanceId, MCPStore, McpCompletionRequest, ScopeRef};
use serde_json::{json, Value};

use crate::daemon::protocol::KernelOperation;

/// Resolve and validate execution target at the daemon trust boundary.
/// Missing field keeps older clients on the daemon target.
pub(crate) async fn resolve_daemon_execution_target(
    store: &MCPStore,
    instance_id: InstanceId,
    payload: &Value,
) -> Result<ExecutionTarget, Error> {
    let requested = match payload.get("execute_on") {
        None | Some(Value::Null) => ExecutionTarget::Daemon,
        Some(Value::String(target)) => target
            .parse()
            .map_err(|error: String| Error::new(FailureCode::InvalidInput, error))?,
        Some(_) => {
            return Err(Error::new(
                FailureCode::InvalidInput,
                "execute_on must be a string",
            ))
        }
    };
    if requested != ExecutionTarget::Daemon {
        return Err(Error::new(
            FailureCode::CapabilityUnsupported,
            "daemon execution accepts execute_on=daemon only",
        ));
    }
    if let Some(instance) = store.find_instance(instance_id).await {
        if let Some(definition) = store.find_definition(&instance.service_name).await {
            if let Some(policy) = definition.execution_policy {
                if let Some(missing) = missing_local_capabilities(&policy, &requested) {
                    return Err(capabilities_unsupported(missing));
                }
                if !policy.allows(&requested) {
                    return Err(target_not_allowed(
                        &requested,
                        &instance.service_name,
                        &policy,
                    ));
                }
            }
        }
    }
    Ok(requested)
}

fn target_not_allowed(
    target: &ExecutionTarget,
    service_name: &str,
    policy: &ExecutionPolicy,
) -> Error {
    Error::new(
        FailureCode::InvalidInput,
        format!(
            "execution target '{target}' not allowed for service '{service_name}'; allowed: {}",
            policy
                .allowed_targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
}

/// 执行一个业务 op。请求/响应 op 全部经此；流式 op（StreamToolExecution/
/// SubscribeEvents）与管理 op 不在此列。
fn mutation_status(store: &MCPStore) -> &'static str {
    if store.is_control_mutation_queued() {
        "queued"
    } else {
        "applied"
    }
}

pub(crate) async fn execute(
    store: &MCPStore,
    operation: KernelOperation,
    payload: Value,
) -> Result<Value, Error> {
    match operation {
        KernelOperation::CallTool => {
            let instance_id = instance_id(&payload)?;
            resolve_daemon_execution_target(store, instance_id, &payload).await?;
            let tool_name = required_str(&payload, "tool_name")?;
            let args = payload.get("args").cloned().unwrap_or_else(|| json!({}));
            let result = store.call_tool(instance_id, &tool_name, args).await?;
            Ok(json!({
                "content": result.content,
                "is_error": result.is_error,
            }))
        }
        KernelOperation::ListTools => {
            let instance_id = instance_id(&payload)?;
            let tools = store
                .list_tool_entries_for_instance_with_filter(
                    instance_id,
                    mcpstore::ToolVisibilityFilter::Available,
                )
                .await?;
            let tools: Vec<Value> = tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "schema": tool.input_schema,
                    })
                })
                .collect();
            Ok(json!({"tools": tools, "total": tools.len()}))
        }
        KernelOperation::ListServices => {
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            let services = store.list_scope_instances(&scope).await?;
            let mut data = Vec::with_capacity(services.len());
            for service in services {
                let state = store.service_state_entry(service.instance_id).await?;
                let metadata = store.mcp_server_metadata(service.instance_id).await?;
                data.push(json!({
                    "instance_id": service.instance_id,
                    "service_name": service.service_name,
                    "scope": service.scope,
                    "transport": service.transport,
                    "state": state,
                    "tools_count": service.tools.len(),
                    "mcp": metadata,
                }));
            }
            Ok(json!({"services": data, "total": data.len()}))
        }
        KernelOperation::GetService => {
            let instance_id = instance_id(&payload)?;
            let service = store
                .find_instance(instance_id)
                .await
                .ok_or_else(|| service_not_found(instance_id))?;
            let state = store.service_state_entry(instance_id).await?;
            Ok(json!({
                "instance_id": service.instance_id,
                "service_name": service.service_name,
                "scope": service.scope,
                "transport": service.transport,
                "state": state,
                "tools": service.tools.iter().map(|tool| json!({
                    "name": tool.name,
                    "description": tool.description,
                })).collect::<Vec<_>>(),
            }))
        }
        KernelOperation::GetServiceInfo => {
            let instance_id = instance_id(&payload)?;
            Ok(store.service_info_scoped(instance_id).await?)
        }
        KernelOperation::ConnectService => {
            let instance_id = instance_id(&payload)?;
            let request_id = store.connect_service(instance_id).await?;
            if store.is_control_mutation_queued() {
                return Ok(json!({"request_id": request_id, "status": "queued"}));
            }
            let tools = store
                .list_tool_entries_for_instance_with_filter(
                    instance_id,
                    mcpstore::ToolVisibilityFilter::Available,
                )
                .await
                .unwrap_or_default();
            let metadata = store.mcp_server_metadata(instance_id).await?;
            Ok(json!({
                "instance_id": instance_id,
                "tools_count": tools.len(),
                "tools": tools.iter().map(|tool| json!({
                    "name": tool.name,
                    "description": tool.description,
                })).collect::<Vec<_>>(),
                "mcp": metadata,
            }))
        }
        KernelOperation::DisconnectService => {
            let instance_id = instance_id(&payload)?;
            let request_id = store.disconnect_service(instance_id).await?;
            Ok(
                json!({"instance_id": instance_id, "request_id": request_id, "status": mutation_status(store)}),
            )
        }
        KernelOperation::RestartService => {
            let instance_id = instance_id(&payload)?;
            let request_id = store.restart_service(instance_id).await?;
            Ok(
                json!({"instance_id": instance_id, "request_id": request_id, "status": mutation_status(store)}),
            )
        }
        KernelOperation::CheckService => {
            let instance_id = instance_id(&payload)?;
            let state = store.service_state_entry(instance_id).await?;
            Ok(json!({"instance_id": instance_id, "state": state}))
        }
        KernelOperation::WaitService => {
            let instance_id = instance_id(&payload)?;
            let timeout = payload.get("timeout").and_then(Value::as_u64).unwrap_or(30);
            let state = store
                .wait_instance_ready(instance_id, std::time::Duration::from_secs(timeout))
                .await?;
            Ok(json!({"instance_id": instance_id, "state": state}))
        }
        KernelOperation::AddService => add_service(store, payload).await,
        KernelOperation::UpdateService => {
            let name = required_str(&payload, "name")?;
            let config = payload_field::<ServerConfig>(&payload, "config")?;
            let execution_policy =
                payload_field::<Option<ExecutionPolicy>>(&payload, "execution_policy")?;
            let request_id = store
                .update_service(&name, config, execution_policy)
                .await?;
            Ok(
                json!({"service_name": name, "request_id": request_id, "status": mutation_status(store)}),
            )
        }
        KernelOperation::DeclareServiceScope => {
            let service_name = required_str(&payload, "service_name")?;
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            let descriptor = payload_field::<ScopeDescriptor>(&payload, "descriptor")?;
            let request_id = store
                .declare_service_scope(&service_name, &scope, descriptor)
                .await?;
            let instance_id =
                mcpstore::ServiceInstanceKey::new(service_name.clone(), scope.clone())
                    .instance_id();
            Ok(json!({
                "instance_id": instance_id,
                "service_name": service_name,
                "scope": scope,
                "request_id": request_id,
                "status": mutation_status(store),
            }))
        }
        KernelOperation::RemoveServiceScope => {
            let service_name = required_str(&payload, "service_name")?;
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            let request_id = store.remove_service_scope(&service_name, &scope).await?;
            Ok(
                json!({"service_name": service_name, "scope": scope, "request_id": request_id, "status": mutation_status(store)}),
            )
        }
        KernelOperation::ListAgents => {
            let agents = store.list_agents().await?;
            Ok(json!({"agents": agents, "total": agents.len()}))
        }
        KernelOperation::ShowConfig => store.show_config().await,
        KernelOperation::ResetConfig => {
            let request_id = store.reset_config().await?;
            Ok(json!({"request_id": request_id, "status": mutation_status(store)}))
        }
        KernelOperation::AuthStatus => {
            let instance_id = instance_id(&payload)?;
            Ok(json!({"auth": store.auth_status_view(instance_id).await?}))
        }
        KernelOperation::AuthCallbackUri => {
            let instance_id = instance_id(&payload)?;
            let callback_uri = store.authorization_callback_uri(instance_id).await?;
            Ok(json!({"callback_uri": callback_uri}))
        }
        KernelOperation::AuthBegin => auth_begin(store, payload).await,
        KernelOperation::AuthCallback => {
            let instance_id = instance_id(&payload)?;
            let code = required_str(&payload, "code")?;
            let state = required_str(&payload, "state")?;
            let issuer = payload.get("issuer").and_then(Value::as_str);
            store
                .complete_authorization_callback(instance_id, &code, &state, issuer)
                .await?;
            reconnect_authorized_service(store, instance_id).await?;
            Ok(json!({"auth": store.auth_status_view(instance_id).await?}))
        }
        KernelOperation::AuthRefresh => {
            let instance_id = instance_id(&payload)?;
            store.refresh_authorization(instance_id).await?;
            reconnect_authorized_service(store, instance_id).await?;
            Ok(json!({"auth": store.auth_status_view(instance_id).await?}))
        }
        KernelOperation::AuthLogout => {
            let instance_id = instance_id(&payload)?;
            store.logout_authorization(instance_id).await?;
            Ok(json!({"auth": store.auth_status_view(instance_id).await?}))
        }
        KernelOperation::AuthScopeUpgrade => {
            let instance_id = instance_id(&payload)?;
            let required_scope = required_str(&payload, "required_scope")?;
            let authorization = store
                .begin_scope_upgrade(instance_id, required_scope.trim())
                .await?;
            let auth = store.auth_status_view(instance_id).await?;
            Ok(json!({"auth": auth, "authorization": authorization}))
        }
        KernelOperation::AuthSaveClientSecret => {
            let instance_id = instance_id(&payload)?;
            let secret = required_str(&payload, "client_secret")?;
            store.save_oauth_client_secret(instance_id, secret).await?;
            Ok(json!({"stored": true}))
        }
        KernelOperation::AuthSavePrivateKey => {
            let instance_id = instance_id(&payload)?;
            let private_key = required_str(&payload, "private_key_pem")?;
            store
                .save_oauth_private_key(instance_id, private_key.into_bytes())
                .await?;
            Ok(json!({"stored": true}))
        }
        KernelOperation::ResourcesList => {
            let instance_id = instance_id(&payload)?;
            resolve_daemon_execution_target(store, instance_id, &payload).await?;
            let resources = store.list_resources(instance_id).await?;
            Ok(json!({"resources": resources, "total": resources.len()}))
        }
        KernelOperation::ResourcesTemplates => {
            let instance_id = instance_id(&payload)?;
            resolve_daemon_execution_target(store, instance_id, &payload).await?;
            let templates = store.list_resource_templates(instance_id).await?;
            Ok(json!({"templates": templates, "total": templates.len()}))
        }
        KernelOperation::ResourcesRead => {
            let instance_id = instance_id(&payload)?;
            resolve_daemon_execution_target(store, instance_id, &payload).await?;
            let uri = required_str(&payload, "uri")?;
            Ok(json!({"resource": store.read_resource(instance_id, &uri).await?}))
        }
        KernelOperation::PromptsList => {
            let instance_id = instance_id(&payload)?;
            resolve_daemon_execution_target(store, instance_id, &payload).await?;
            let prompts = store.list_prompts(instance_id).await?;
            Ok(json!({"prompts": prompts, "total": prompts.len()}))
        }
        KernelOperation::PromptGet => {
            let instance_id = instance_id(&payload)?;
            resolve_daemon_execution_target(store, instance_id, &payload).await?;
            let prompt_name = required_str(&payload, "prompt_name")?;
            let arguments = payload
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let prompt = store
                .get_prompt(instance_id, &prompt_name, arguments)
                .await?;
            Ok(json!({"prompt": prompt}))
        }
        KernelOperation::CompleteArgument => {
            let instance_id = instance_id(&payload)?;
            resolve_daemon_execution_target(store, instance_id, &payload).await?;
            let request = payload_field::<McpCompletionRequest>(&payload, "request")?;
            let completion = store.complete_mcp_argument(instance_id, request).await?;
            Ok(json!({"completion": completion}))
        }
        KernelOperation::TaskList => {
            let instance_id = instance_id(&payload)?;
            let records = store.list_task_records(instance_id).await?;
            Ok(json!({"records": records, "total": records.len()}))
        }
        KernelOperation::TaskGet => {
            let instance_id = instance_id(&payload)?;
            let task_id = required_str(&payload, "task_id")?;
            Ok(json!({"record": store.get_task_record(instance_id, &task_id).await?}))
        }
        KernelOperation::TaskLive => {
            let instance_id = instance_id(&payload)?;
            let task_id = required_str(&payload, "task_id")?;
            Ok(json!({"task": store.get_task(instance_id, &task_id).await?}))
        }
        KernelOperation::TaskResult => {
            let instance_id = instance_id(&payload)?;
            let task_id = required_str(&payload, "task_id")?;
            Ok(json!({"result": store.get_task_result(instance_id, &task_id).await?}))
        }
        KernelOperation::TaskCancel => {
            let instance_id = instance_id(&payload)?;
            let task_id = required_str(&payload, "task_id")?;
            store.cancel_task(instance_id, &task_id).await?;
            Ok(json!({"cancelled": true, "task_id": task_id}))
        }
        KernelOperation::SwapStore => {
            let store_name = required_str(&payload, "store")?;
            let config = payload.get("config").cloned().unwrap_or_else(|| json!({}));
            let target = mcpstore::JsonStoreConfig::new(&store_name, config);
            let result = store.swap_store(&target).await?;
            Ok(json!({"target_store": result.target_store, "copied": result.copied}))
        }
        KernelOperation::ListInstances => {
            let instances = store.list_instances().await;
            let mut data = Vec::with_capacity(instances.len());
            for instance in instances {
                let state = store.service_state_entry(instance.instance_id).await?;
                data.push(json!({"instance": instance, "state": state}));
            }
            Ok(json!({"instances": data, "total": data.len()}))
        }
        KernelOperation::ListAllTools => {
            let tools: Vec<Value> = store
                .list_all_tools()
                .await
                .into_iter()
                .map(|(instance_id, tool)| json!({"instance_id": instance_id, "tool": tool}))
                .collect();
            Ok(json!({"tools": tools, "total": tools.len()}))
        }
        KernelOperation::EventHistory => {
            let count = payload.get("count").and_then(Value::as_u64).unwrap_or(100) as usize;
            Ok(json!({"events": store.event_history(count).await}))
        }
        KernelOperation::EventCapabilityReport => Ok(store.event_capability_report().await),
        KernelOperation::CacheHealth => store.cache_health_check().await,
        KernelOperation::ControlRequestGet => {
            let request_id = required_str(&payload, "request_id")?;
            let request = store.control_request(&request_id).await?;
            Ok(serde_json::to_value(request)
                .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?)
        }
        KernelOperation::ControlRequestList => {
            let requests = store.control_requests().await?;
            Ok(json!({"requests": requests, "total": requests.len()}))
        }
        KernelOperation::HealthCheck => {
            let instance_id = instance_id(&payload)?;
            Ok(json!({"state": store.health_check(instance_id).await?}))
        }
        KernelOperation::GetDefinitionConfig => {
            let name = required_str(&payload, "name")?;
            Ok(json!({"config": store.get_definition_config(&name).await?}))
        }
        KernelOperation::LoadFromSource => {
            store.load_from_source().await?;
            Ok(json!({"status": "ok"}))
        }
        KernelOperation::GetAppConfig => {
            let manager = store.config_manager();
            let config = manager.load_app_config_or_default().map_err(config_error)?;
            Ok(json!({
                "config": config,
                "mcp_path": manager.mcp_path().display().to_string(),
                "current_store_name": store.current_store_name().await,
                "namespace": store.namespace(),
            }))
        }
        KernelOperation::SaveAppConfig => {
            // server/mcp_aggregate 是 daemon 热应用面，必须走 config --<key>；其余段整体保存
            let new_config = payload_field::<AppConfig>(&payload, "config")?;
            let manager = store.config_manager();
            let current = manager.load_app_config_or_default().map_err(config_error)?;
            if serde_json::to_value(&new_config.server).unwrap_or(Value::Null)
                != serde_json::to_value(&current.server).unwrap_or(Value::Null)
                || serde_json::to_value(&new_config.mcp_aggregate).unwrap_or(Value::Null)
                    != serde_json::to_value(&current.mcp_aggregate).unwrap_or(Value::Null)
            {
                return Err(Error::new(
                    FailureCode::InvalidInput,
                    "server/mcp_aggregate 段是 daemon 热应用配置，请用 mcpstore config --<key> <value> 修改",
                ));
            }
            manager.save_app_config(&new_config).map_err(config_error)?;
            Ok(json!({"saved": true}))
        }
        KernelOperation::ListScopeTools => {
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            Ok(json!({"tools": store.list_tools_scoped(&scope).await?}))
        }
        KernelOperation::ListScopeResources => {
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            Ok(json!({"resources": store.list_resources_scoped(&scope).await?}))
        }
        KernelOperation::ListScopeResourceTemplates => {
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            Ok(json!({"templates": store.list_resource_templates_scoped(&scope).await?}))
        }
        KernelOperation::ListScopePrompts => {
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            Ok(json!({"prompts": store.list_prompts_scoped(&scope).await?}))
        }
        KernelOperation::SubscribeEvents
        | KernelOperation::StreamToolExecution
        | KernelOperation::StopHost
        | KernelOperation::StatusHost
        | KernelOperation::GetDaemonConfig
        | KernelOperation::SetDaemonConfig => {
            unreachable!("stream/admin operations are handled by the daemon server")
        }
    }
}

pub(crate) fn config_error(error: mcpstore::config::ConfigError) -> Error {
    Error::new(
        FailureCode::InvalidInput,
        format!("config.toml Operation failed: {error}"),
    )
}

pub(crate) async fn add_service(store: &MCPStore, payload: Value) -> Result<Value, Error> {
    let name = required_str(&payload, "name")?;
    let mut config = payload_field::<ServerConfig>(&payload, "config")?;
    let scope = payload_field::<ScopeRef>(&payload, "scope")?;
    if let ScopeRef::Agent { agent_id } = &scope {
        let previous = config.mcpstore.take();
        let mut scopes = ScopeDeclarations::default();
        scopes
            .agents
            .insert(agent_id.clone(), ScopeDescriptor::default());
        config.mcpstore = Some(McpStoreExtension {
            scopes,
            lifecycle: previous
                .as_ref()
                .and_then(|extension| extension.lifecycle.clone()),
            handshake_mode: previous
                .as_ref()
                .and_then(|extension| extension.handshake_mode),
            execution_policy: previous
                .as_ref()
                .and_then(|extension| extension.execution_policy.clone()),
            revision: previous
                .as_ref()
                .map(|extension| extension.revision)
                .unwrap_or(1)
                .max(1),
            extra: previous
                .map(|extension| extension.extra)
                .unwrap_or_default(),
        });
    }
    let definition_exists = store.get_definition_config(&name).await?.is_some();
    let request_id;
    if definition_exists {
        let lifecycle = config
            .mcpstore
            .as_ref()
            .and_then(|extension| extension.lifecycle.clone());
        request_id = store
            .declare_service_scope(
                &name,
                &scope,
                ScopeDescriptor {
                    config: config.base_config(),
                    lifecycle,
                    revision: 0,
                    ..Default::default()
                },
            )
            .await?
            .to_string();
    } else {
        request_id = store.add_service(&name, config).await?;
    }
    let instance_id = mcpstore::ServiceInstanceKey::new(name.clone(), scope.clone()).instance_id();
    Ok(json!({
        "service_name": name,
        "scope": scope,
        "instance_id": instance_id,
        "request_id": request_id,
        "status": mutation_status(store),
    }))
}

async fn auth_begin(store: &MCPStore, payload: Value) -> Result<Value, Error> {
    let instance_id = instance_id(&payload)?;
    let auth = store.auth_status_view(instance_id).await?;
    match auth.flow {
        Some(AuthFlow::AuthorizationCode) => {
            let authorization = store.begin_authorization(instance_id).await?;
            let auth = store.auth_status_view(instance_id).await?;
            Ok(json!({"auth": auth, "authorization": authorization}))
        }
        Some(AuthFlow::ClientCredentials) => {
            store.refresh_authorization(instance_id).await?;
            reconnect_authorized_service(store, instance_id).await?;
            let auth = store.auth_status_view(instance_id).await?;
            Ok(json!({"auth": auth, "authorization": null}))
        }
        None => Err(Error::new(
            FailureCode::ConnectionAuthRequired,
            "authentication is not configured for this instance",
        )),
    }
}

pub(crate) async fn reconnect_authorized_service(
    store: &MCPStore,
    instance_id: InstanceId,
) -> mcpstore::Result<()> {
    store.disconnect_service(instance_id).await.ok();
    store.connect_service(instance_id).await.map(|_| ())
}

pub(crate) fn service_not_found(instance_id: InstanceId) -> Error {
    Error::new(
        FailureCode::ServiceNotFound,
        format!("service instance not found: {instance_id}"),
    )
}

pub(crate) fn payload_field<T>(payload: &Value, key: &str) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(payload.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|error| Error::new(FailureCode::InvalidInput, format!("invalid {key}: {error}")))
}

pub(crate) fn required_str(payload: &Value, key: &str) -> Result<String, Error> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| Error::new(FailureCode::InvalidInput, format!("{key} is required")))
}

pub(crate) fn instance_id(payload: &Value) -> Result<InstanceId, Error> {
    required_str(payload, "instance_id")?
        .parse()
        .map_err(|error| {
            Error::new(
                FailureCode::InvalidInput,
                format!("invalid instance_id: {error}"),
            )
        })
}

/// 配置 key 表（设计文档 §7）：把单 key 修改应用到 AppConfig，
/// 返回受影响面的目标端口（Some=起/重绑，None=停；空=仅改配置无面变更）。
pub(crate) fn plan_config_change(
    config: &mut AppConfig,
    key: &str,
    value: &Value,
) -> Result<Vec<(crate::daemon::listeners::ListenerKey, Option<u16>)>, Error> {
    use crate::daemon::listeners::ListenerKey;

    fn aggregate_target(config: &AppConfig) -> Option<u16> {
        (config.mcp_aggregate.enabled && config.mcp_aggregate.transport == "streamable-http")
            .then_some(config.mcp_aggregate.port)
    }

    let server = &mut config.server;
    match key {
        "host" => {
            let host = required_str_value(value, "host")?;
            if host.trim().is_empty() {
                return Err(Error::new(FailureCode::InvalidInput, "host 不能为空"));
            }
            let mut planned = Vec::new();
            if server.core_enabled {
                planned.push((ListenerKey::Core, Some(server.port)));
            }
            if server.app_enabled {
                planned.push((ListenerKey::App, Some(server.app_port)));
            }
            if server.web_enabled {
                planned.push((ListenerKey::Web, Some(server.web_port)));
            }
            server.host = host;
            let aggregate = aggregate_target(config);
            if aggregate.is_some() {
                planned.push((ListenerKey::Aggregate, aggregate));
            }
            Ok(planned)
        }
        "core" => {
            let on = parse_switch(value, "core")?;
            server.core_enabled = on;
            Ok(vec![(ListenerKey::Core, on.then_some(server.port))])
        }
        "core-port" => {
            let port = parse_port(value, "core-port")?;
            server.port = port;
            Ok(server
                .core_enabled
                .then_some(vec![(ListenerKey::Core, Some(port))])
                .unwrap_or_default())
        }
        "app" => {
            let on = parse_switch(value, "app")?;
            server.app_enabled = on;
            Ok(vec![(ListenerKey::App, on.then_some(server.app_port))])
        }
        "app-port" => {
            let port = parse_port(value, "app-port")?;
            server.app_port = port;
            Ok(server
                .app_enabled
                .then_some(vec![(ListenerKey::App, Some(port))])
                .unwrap_or_default())
        }
        "web" => {
            let on = parse_switch(value, "web")?;
            server.web_enabled = on;
            Ok(vec![(ListenerKey::Web, on.then_some(server.web_port))])
        }
        "web-port" => {
            let port = parse_port(value, "web-port")?;
            server.web_port = port;
            Ok(server
                .web_enabled
                .then_some(vec![(ListenerKey::Web, Some(port))])
                .unwrap_or_default())
        }
        "mcp" => {
            let on = parse_switch(value, "mcp")?;
            config.mcp_aggregate.enabled = on;
            let target = aggregate_target(config);
            if on && target.is_none() {
                return Err(Error::new(
                    FailureCode::InvalidInput,
                    "mcp 聚合监听要求 transport=streamable-http；先 --mcp-transport streamable-http",
                ));
            }
            Ok(vec![(ListenerKey::Aggregate, target)])
        }
        "mcp-port" => {
            let port = parse_port(value, "mcp-port")?;
            config.mcp_aggregate.port = port;
            Ok(aggregate_target(config)
                .map(|_| vec![(ListenerKey::Aggregate, Some(port))])
                .unwrap_or_default())
        }
        "mcp-transport" => {
            let transport = required_str_value(value, "mcp-transport")?;
            if !matches!(transport.as_str(), "stdio" | "streamable-http") {
                return Err(Error::new(
                    FailureCode::InvalidInput,
                    format!("mcp-transport must be stdio or streamable-http, got {transport}"),
                ));
            }
            config.mcp_aggregate.transport = transport;
            Ok(config
                .mcp_aggregate
                .enabled
                .then_some(vec![(ListenerKey::Aggregate, aggregate_target(config))])
                .unwrap_or_default())
        }
        other => Err(Error::new(
            FailureCode::InvalidInput,
            format!(
                "未知配置 key {other:?}；可用：host, core, core-port, app, app-port, web, web-port, mcp, mcp-port, mcp-transport"
            ),
        )),
    }
}

pub(crate) fn required_str_value(value: &Value, field: &str) -> Result<String, Error> {
    value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            Error::new(
                FailureCode::InvalidInput,
                format!("{field} must be a non-empty string"),
            )
        })
}

fn host_capabilities() -> HashSet<&'static str> {
    let mut capabilities = HashSet::from(["browser"]);
    if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        capabilities.insert("display");
    }
    capabilities
}

fn missing_local_capabilities(
    policy: &ExecutionPolicy,
    target: &ExecutionTarget,
) -> Option<String> {
    if *target != ExecutionTarget::Local {
        return None;
    }
    let capabilities = host_capabilities();
    let missing: Vec<_> = policy
        .required_capabilities
        .iter()
        .filter(|capability| !capabilities.contains(capability.as_str()))
        .cloned()
        .collect();
    (!missing.is_empty()).then(|| missing.join(", "))
}

fn capabilities_unsupported(missing: String) -> Error {
    Error::new(
        FailureCode::CapabilityUnsupported,
        format!("local host lacks required capabilities: {missing}"),
    )
}

pub(crate) fn parse_switch(value: &Value, field: &str) -> Result<bool, Error> {
    match value.as_str() {
        Some("on") | Some("true") => Ok(true),
        Some("off") | Some("false") => Ok(false),
        other => Err(Error::new(
            FailureCode::InvalidInput,
            format!("{field} must be on/off, got {other:?}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpstore::ServiceInstanceKey;

    #[test]
    fn local_execution_rejects_missing_host_capability() {
        let policy = ExecutionPolicy {
            default_target: ExecutionTarget::Local,
            allowed_targets: Vec::new(),
            required_capabilities: vec!["definitely-missing-capability".into()],
        };
        let error = missing_local_capabilities(&policy, &ExecutionTarget::Local).unwrap();
        assert_eq!(error, "definitely-missing-capability");
    }

    #[tokio::test]
    async fn daemon_rejects_execution_target_disallowed_by_service() {
        let path =
            std::env::temp_dir().join(format!("mcpstore-daemon-policy-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let config_path = path.join("mcp.json");
        let store = MCPStore::setup(Some(config_path.to_str().unwrap())).unwrap();
        let mut config = ServerConfig {
            command: Some("echo".into()),
            args: vec!["fixture".into()],
            transport: Some("stdio".into()),
            ..ServerConfig::default()
        };
        config.mcpstore = Some(McpStoreExtension {
            execution_policy: Some(ExecutionPolicy {
                default_target: ExecutionTarget::Local,
                allowed_targets: vec![ExecutionTarget::Local],
                required_capabilities: Vec::new(),
            }),
            scopes: ScopeDeclarations::store_only(),
            ..McpStoreExtension::default()
        });
        store.add_service("svc", config).await.unwrap();
        let instance_id = ServiceInstanceKey::new("svc", ScopeRef::Store).instance_id();

        let error =
            resolve_daemon_execution_target(&store, instance_id, &json!({"execute_on": "daemon"}))
                .await
                .unwrap_err();

        assert!(error.to_string().contains("not allowed"), "{error}");
        std::fs::remove_dir_all(path).ok();
    }
}

pub(crate) fn parse_port(value: &Value, field: &str) -> Result<u16, Error> {
    value
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
        .ok_or_else(|| {
            Error::new(
                FailureCode::InvalidInput,
                format!("{field} must be a port number from 1 to 65535"),
            )
        })
}
