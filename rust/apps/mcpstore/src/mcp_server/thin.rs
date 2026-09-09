//! stdio 聚合 thin client：进程形态保留（stdio 归 MCP 客户端所有），业务全部
//! 转发 daemon —— 不再有第二个 kernel/连接池。scope 模式；elicitation 为
//! headless 语义（daemon 侧调用不开交互会话）。

use std::collections::HashMap;

use mcpstore::{InstanceId, ScopeRef};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, GetPromptRequestParams,
    GetPromptResponse, Implementation, ListPromptsResult, ListResourceTemplatesResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
    ReadResourceRequestParams, ReadResourceResponse, ResourceContents, ServerCapabilities,
    ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::catalog::{
    catalog_name_counts, project_catalog_uris, project_prompt_names,
    resolve_projected_catalog_uri, resolve_projected_prompt, service_namespace,
};
use super::handler::llm_error_result;
use super::tools::{read_required_instance_id, read_required_object, read_required_string};
use super::ToolBinding;

fn deserialize_item<T: serde::de::DeserializeOwned>(
    payload: Value,
    what: &str,
) -> Result<T, ErrorData> {
    serde_json::from_value(payload)
        .map_err(|error| ErrorData::internal_error(format!("failed to decode {what}: {error}"), None))
}

fn deserialize_items<T: serde::de::DeserializeOwned>(
    payloads: Vec<Value>,
    what: &str,
) -> Result<Vec<T>, ErrorData> {
    payloads.into_iter().map(|payload| deserialize_item(payload, what)).collect()
}
use crate::daemon::client::KernelClient;
use crate::daemon::protocol::KernelOperation;

pub struct ThinAggregate {
    scope: ScopeRef,
    client: Mutex<KernelClient>,
    bindings: std::sync::RwLock<HashMap<String, ToolBinding>>,
}

/// 启动 stdio thin client：确保 daemon 在运行，然后以 daemon 代理身份服务 stdio。
pub async fn run(scope: ScopeRef) -> Result<(), crate::BoxErr> {
    if !crate::daemon::ensure::is_daemon_ready().await {
        crate::daemon::ensure::spawn_detached_daemon()?;
        crate::daemon::ensure::wait_daemon_ready(std::time::Duration::from_secs(30)).await?;
    }
    let client = crate::daemon::client::connect_admin().await?;
    let server = ThinAggregate {
        scope,
        client: Mutex::new(client),
        bindings: std::sync::RwLock::new(HashMap::new()),
    };
    eprintln!("[MCP] thin aggregate: forwarding to daemon");
    let running = rmcp::serve_server(server, rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}

impl ThinAggregate {
    async fn op(&self, operation: KernelOperation, payload: Value) -> Result<Value, ErrorData> {
        let mut client = self.client.lock().await;
        client
            .request(
                operation,
                payload,
                crate::daemon::protocol::DEFAULT_REQUEST_TIMEOUT,
            )
            .await
            .map(|(result, _)| result)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))
    }

    /// 拉取 scope 工具目录并重建绑定缓存（命名空间去重与 embedded 聚合同一套规则）。
    async fn refresh_bindings(&self) -> Result<(), ErrorData> {
        let result = self
            .op(
                KernelOperation::ListScopeTools,
                json!({"scope": self.scope}),
            )
            .await?;
        let payloads = result["tools"].as_array().cloned().unwrap_or_default();
        let catalog_err = |error: crate::BoxErr| -> ErrorData {
            ErrorData::internal_error(format!("invalid tool catalog from daemon: {error}"), None)
        };
        let names =
            catalog_name_counts(&payloads, "name").map_err(catalog_err)?;
        let mut bindings = HashMap::with_capacity(payloads.len());
        for payload in payloads {
            let original_name =
                read_required_string(&payload, "name").map_err(catalog_err)?;
            let canonical_tool_name =
                read_required_string(&payload, "tool_name").map_err(catalog_err)?;
            let instance_id =
                read_required_instance_id(&payload, "instance_id").map_err(catalog_err)?;
            let service_name =
                read_required_string(&payload, "service_name").map_err(catalog_err)?;
            let exposed_name =
                if names.get(&original_name).copied().unwrap_or_default() > 1 {
                    format!("{}__{}", service_namespace(&service_name), original_name)
                } else {
                    original_name
                };
            let description: Option<String> = payload
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string);
            let schema = read_required_object(&payload, "input_schema").map_err(catalog_err)?;
            let tool = Tool::new_with_raw(
                exposed_name.clone(),
                description.map(Into::into),
                std::sync::Arc::new(schema),
            );
            bindings.insert(
                exposed_name,
                ToolBinding {
                    tool,
                    instance_id,
                    tool_name: canonical_tool_name,
                },
            );
        }
        *self.bindings.write().expect("thin bindings poisoned") = bindings;
        Ok(())
    }

    fn binding(&self, name: &str) -> Option<ToolBinding> {
        self.bindings
            .read()
            .expect("thin bindings poisoned")
            .get(name)
            .cloned()
    }
}

impl ServerHandler for ThinAggregate {
    fn supported_protocol_versions(&self) -> std::borrow::Cow<'static, [ProtocolVersion]> {
        std::borrow::Cow::Borrowed(&[ProtocolVersion::V_2026_07_28])
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("mcpstore", env!("CARGO_PKG_VERSION")))
        .with_instructions("MCPStore aggregate (daemon-backed thin client)")
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        async move {
            self.refresh_bindings().await?;
            let tools = self
                .bindings
                .read()
                .expect("thin bindings poisoned")
                .values()
                .map(|binding| binding.tool.clone())
                .collect();
            Ok(ListToolsResult::with_all_items(tools))
        }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.binding(name).map(|binding| binding.tool)
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResponse, ErrorData>> + '_ {
        let tool_name = request.name.as_ref().to_string();
        let arguments = request.arguments.unwrap_or_default();
        async move {
            let binding = match self.binding(&tool_name) {
                Some(binding) => binding,
                None => {
                    self.refresh_bindings().await?;
                    self.binding(&tool_name).ok_or_else(|| {
                        ErrorData::invalid_params(format!("Unknown tool: {tool_name}"), None)
                    })?
                }
            };
            let result = match self
                .op(
                    KernelOperation::CallTool,
                    json!({
                        "instance_id": binding.instance_id.to_string(),
                        "tool_name": binding.tool_name,
                        "args": Value::Object(arguments),
                    }),
                )
                .await
            {
                Ok(result) => result,
                Err(error) => {
                    // daemon 返回的是 store 层错误，保留 LLM 面错误语义
                    let _ = error;
                    return Ok(CallToolResponse::Complete(CallToolResult::error(vec![
                        ContentBlock::text(format!("Daemon call failed: {error}")),
                    ])));
                }
            };
            let result: mcpstore::ToolCallResult = match serde_json::from_value(result) {
                Ok(result) => result,
                Err(error) => {
                    return Ok(llm_error_result(&mcpstore::Error::new(
                        mcpstore::error::FailureCode::Internal,
                        format!("Failed to decode call result: {error}"),
                    )))
                }
            };
            let mut content = Vec::with_capacity(result.content.len());
            for item in result.content {
                content.push(match item {
                    mcpstore::transport::ContentItem::Text { text, .. } => ContentBlock::text(text),
                    mcpstore::transport::ContentItem::Image { data, mime_type, .. } => {
                        ContentBlock::image(data, mime_type)
                    }
                    mcpstore::transport::ContentItem::Audio { data, mime_type, .. } => {
                        ContentBlock::audio(data, mime_type)
                    }
                    mcpstore::transport::ContentItem::Resource { resource, .. } => {
                        match serde_json::from_value::<ResourceContents>(resource) {
                            Ok(resource) => ContentBlock::resource(resource),
                            Err(error) => {
                                ContentBlock::text(format!("Failed to decode resource content: {error}"))
                            }
                        }
                    }
                    mcpstore::transport::ContentItem::ResourceLink { resource, .. } => {
                        match serde_json::from_value::<rmcp::model::Resource>(resource) {
                            Ok(resource) => ContentBlock::resource_link(resource),
                            Err(error) => {
                                ContentBlock::text(format!("Failed to decode resource link: {error}"))
                            }
                        }
                    }
                });
            }
            Ok(CallToolResponse::Complete(if result.is_error {
                CallToolResult::error(content)
            } else {
                CallToolResult::success(content)
            }))
        }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, ErrorData>> + '_ {
        async move {
            let result = self
                .op(
                    KernelOperation::ListScopeResources,
                    json!({"scope": self.scope}),
                )
                .await?;
            let resources = result["resources"].as_array().cloned().unwrap_or_default();
            let resources = project_catalog_uris(resources, "uri", false)?;
            let resources = deserialize_items::<rmcp::model::Resource>(resources, "resource")?;
            Ok(ListResourcesResult::with_all_items(resources))
        }
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourceTemplatesResult, ErrorData>> + '_
    {
        async move {
            let result = self
                .op(
                    KernelOperation::ListScopeResourceTemplates,
                    json!({"scope": self.scope}),
                )
                .await?;
            let templates = result["templates"].as_array().cloned().unwrap_or_default();
            let templates = project_catalog_uris(templates, "uriTemplate", true)?;
            let templates =
                deserialize_items::<rmcp::model::ResourceTemplate>(templates, "resource template")?;
            Ok(ListResourceTemplatesResult::with_all_items(templates))
        }
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResponse, ErrorData>> + '_ {
        let uri = request.uri;
        async move {
            let result = self
                .op(
                    KernelOperation::ListScopeResources,
                    json!({"scope": self.scope}),
                )
                .await?;
            let resources = result["resources"].as_array().cloned().unwrap_or_default();
            let (instance_id, original_uri) =
                resolve_projected_catalog_uri(&resources, "uri", false, &uri)?;
            let result = self
                .op(
                    KernelOperation::ResourcesRead,
                    json!({
                        "instance_id": instance_id.to_string(),
                        "uri": original_uri,
                    }),
                )
                .await?;
            deserialize_item::<rmcp::model::ReadResourceResult>(
                result["resource"].clone(),
                "read resource result",
            )
            .map(Into::into)
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListPromptsResult, ErrorData>> + '_ {
        async move {
            let result = self
                .op(
                    KernelOperation::ListScopePrompts,
                    json!({"scope": self.scope}),
                )
                .await?;
            let prompts = result["prompts"].as_array().cloned().unwrap_or_default();
            let prompts = project_prompt_names(prompts)?;
            let prompts = deserialize_items::<rmcp::model::Prompt>(prompts, "prompt")?;
            Ok(ListPromptsResult::with_all_items(prompts))
        }
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetPromptResponse, ErrorData>> + '_ {
        let name = request.name;
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        async move {
            let result = self
                .op(
                    KernelOperation::ListScopePrompts,
                    json!({"scope": self.scope}),
                )
                .await?;
            let prompts = result["prompts"].as_array().cloned().unwrap_or_default();
            let (instance_id, original_name) = resolve_projected_prompt(&prompts, &name)?;
            let result = self
                .op(
                    KernelOperation::PromptGet,
                    json!({
                        "instance_id": instance_id.to_string(),
                        "prompt_name": original_name,
                        "arguments": arguments,
                    }),
                )
                .await?;
            deserialize_item::<rmcp::model::GetPromptResult>(
                result["prompt"].clone(),
                "prompt result",
            )
            .map(Into::into)
        }
    }
}
