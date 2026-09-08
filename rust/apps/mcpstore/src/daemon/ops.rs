//! 业务 op 分发：daemon socket 与 CLI embedded 进程共用同一份实现
//! （单业务协议、双执行位置）。daemon 管理面（status/config/stop）在 server.rs。

use mcpstore::config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig};
use mcpstore::error::{Error, FailureCode};
use mcpstore::{AuthFlow, InstanceId, McpCompletionRequest, MCPStore, ScopeRef};
use serde_json::{json, Value};

use crate::daemon::protocol::KernelOperation;

/// 执行一个业务 op。请求/响应 op 全部经此；流式 op（StreamToolExecution/
/// SubscribeEvents）与管理 op 不在此列。
pub(crate) async fn execute(
    store: &MCPStore,
    operation: KernelOperation,
    payload: Value,
) -> Result<Value, Error> {
    match operation {
        KernelOperation::CallTool => {
            let instance_id = instance_id(&payload)?;
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
            store.connect_service(instance_id).await?;
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
            store.disconnect_service(instance_id).await?;
            Ok(json!({"instance_id": instance_id}))
        }
        KernelOperation::RestartService => {
            let instance_id = instance_id(&payload)?;
            store.restart_service(instance_id).await?;
            Ok(json!({"instance_id": instance_id}))
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
            store.update_service(&name, config).await?;
            Ok(json!({"service_name": name}))
        }
        KernelOperation::DeclareServiceScope => {
            let service_name = required_str(&payload, "service_name")?;
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            let descriptor = payload_field::<ScopeDescriptor>(&payload, "descriptor")?;
            let instance_id = store
                .declare_service_scope(&service_name, &scope, descriptor)
                .await?;
            Ok(json!({
                "instance_id": instance_id,
                "service_name": service_name,
                "scope": scope,
            }))
        }
        KernelOperation::RemoveServiceScope => {
            let service_name = required_str(&payload, "service_name")?;
            let scope = payload_field::<ScopeRef>(&payload, "scope")?;
            store.remove_service_scope(&service_name, &scope).await?;
            Ok(json!({"service_name": service_name, "scope": scope}))
        }
        KernelOperation::ListAgents => {
            let agents = store.list_agents().await?;
            Ok(json!({"agents": agents, "total": agents.len()}))
        }
        KernelOperation::ShowConfig => store.show_config().await,
        KernelOperation::ResetConfig => {
            store.reset_config().await?;
            Ok(json!({"status": "ok"}))
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
            let resources = store.list_resources(instance_id).await?;
            Ok(json!({"resources": resources, "total": resources.len()}))
        }
        KernelOperation::ResourcesTemplates => {
            let instance_id = instance_id(&payload)?;
            let templates = store.list_resource_templates(instance_id).await?;
            Ok(json!({"templates": templates, "total": templates.len()}))
        }
        KernelOperation::ResourcesRead => {
            let instance_id = instance_id(&payload)?;
            let uri = required_str(&payload, "uri")?;
            Ok(json!({"resource": store.read_resource(instance_id, &uri).await?}))
        }
        KernelOperation::PromptsList => {
            let instance_id = instance_id(&payload)?;
            let prompts = store.list_prompts(instance_id).await?;
            Ok(json!({"prompts": prompts, "total": prompts.len()}))
        }
        KernelOperation::PromptGet => {
            let instance_id = instance_id(&payload)?;
            let prompt_name = required_str(&payload, "prompt_name")?;
            let arguments = payload.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let prompt = store.get_prompt(instance_id, &prompt_name, arguments).await?;
            Ok(json!({"prompt": prompt}))
        }
        KernelOperation::CompleteArgument => {
            let instance_id = instance_id(&payload)?;
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
    if definition_exists {
        let lifecycle = config
            .mcpstore
            .as_ref()
            .and_then(|extension| extension.lifecycle.clone());
        store
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
            .await?;
    } else {
        store.add_service(&name, config).await?;
    }
    Ok(json!({"service_name": name, "scope": scope}))
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
