use super::catalog::{
    project_catalog_uris, project_prompt_names, resolve_projected_catalog_uri,
    resolve_projected_prompt,
};
use super::tools::{
    build_cache_tools, build_openapi_tools, build_prompt_override_tools,
    build_resource_override_tools, build_resource_template_override_tools, build_search_tools,
    build_service_tools, build_session_state_tools, build_tool_override_tools, deserialize_item,
    deserialize_items,
};
use super::tools::{
    call_cache_tool, call_openapi_tool, call_prompt_override_tool, call_resource_override_tool,
    call_resource_template_override_tool, call_service_tool, call_session_state_tool,
    call_tool_override_tool, extract_business_session_key, map_store_error,
};
use super::transport::{build_tool_bindings, connect_target_instances};
use super::*;
use serde_json::json;

impl McpStoreServer {
    pub(super) async fn from_store(
        store: Arc<MCPStore>,
        scope: ScopeRef,
        instance_id: Option<InstanceId>,
        session_key: Option<String>,
        expose_session_state_tools: bool,
        expose_tool_override_tools: bool,
        expose_prompt_override_tools: bool,
        expose_resource_override_tools: bool,
        expose_openapi_tools: bool,
        expose_service_tools: bool,
        expose_cache_tools: bool,
        expose_search_tools: bool,
    ) -> Result<Self, BoxErr> {
        connect_target_instances(&store, &scope, instance_id).await?;
        if let Some(session_key) = session_key.as_deref() {
            store.session_by_key(session_key).status().await?;
        }
        let bindings =
            build_tool_bindings(&store, &scope, instance_id, session_key.as_deref()).await?;
        let session_state_tools = if expose_session_state_tools {
            build_session_state_tools()
        } else {
            HashMap::new()
        };
        let tool_override_tools = if expose_tool_override_tools {
            build_tool_override_tools()
        } else {
            HashMap::new()
        };
        let prompt_override_tools = if expose_prompt_override_tools {
            build_prompt_override_tools()
        } else {
            HashMap::new()
        };
        let resource_override_tools = if expose_resource_override_tools {
            build_resource_override_tools()
                .into_iter()
                .chain(build_resource_template_override_tools())
                .collect()
        } else {
            HashMap::new()
        };
        let openapi_tools = if expose_openapi_tools {
            build_openapi_tools()
        } else {
            HashMap::new()
        };
        let service_tools = if expose_service_tools {
            build_service_tools()
        } else {
            HashMap::new()
        };
        let cache_tools = if expose_cache_tools {
            build_cache_tools()
        } else {
            HashMap::new()
        };
        let search_tools = if expose_search_tools {
            build_search_tools()
        } else {
            HashMap::new()
        };
        for tool_name in session_state_tools
            .keys()
            .chain(tool_override_tools.keys())
            .chain(prompt_override_tools.keys())
            .chain(resource_override_tools.keys())
            .chain(openapi_tools.keys())
            .chain(service_tools.keys())
            .chain(cache_tools.keys())
            .chain(search_tools.keys())
        {
            if bindings.contains_key(tool_name) {
                return Err(format!(
                    "MCPStore 管理工具与下游工具重名，无法构建 Rust MCP server: tool={tool_name}"
                )
                .into());
            }
        }
        let mut tools = bindings
            .values()
            .map(|binding| binding.tool.clone())
            .collect::<Vec<_>>();
        tools.extend(session_state_tools.values().cloned());
        tools.extend(tool_override_tools.values().cloned());
        tools.extend(prompt_override_tools.values().cloned());
        tools.extend(resource_override_tools.values().cloned());
        tools.extend(openapi_tools.values().cloned());
        tools.extend(service_tools.values().cloned());
        tools.extend(cache_tools.values().cloned());
        tools.extend(search_tools.values().cloned());
        tools.sort_by(|left, right| left.name.cmp(&right.name));

        let scope_label = match &scope {
            ScopeRef::Store => "store".to_string(),
            ScopeRef::Agent { agent_id } => format!("agent:{agent_id}"),
        };
        let scope_label = match instance_id {
            Some(instance_id) => format!("{scope_label} instance:{instance_id}"),
            None => scope_label,
        };
        let scope_label = match session_key.as_deref() {
            Some(session_key) => format!("{scope_label} session:{session_key}"),
            None => scope_label,
        };

        Ok(Self {
            store,
            scope,
            instance_id,
            session_key,
            scope_label,
            bindings: Arc::new(bindings),
            session_state_tools: Arc::new(session_state_tools),
            tool_override_tools: Arc::new(tool_override_tools),
            prompt_override_tools: Arc::new(prompt_override_tools),
            resource_override_tools: Arc::new(resource_override_tools),
            openapi_tools: Arc::new(openapi_tools),
            service_tools: Arc::new(service_tools),
            cache_tools: Arc::new(cache_tools),
            search_tools: Arc::new(search_tools),
            tools: Arc::new(tools),
        })
    }

    fn instructions(&self) -> String {
        format!(
            "Rust MCPStore server. scope={} tool_count={}",
            self.scope_label,
            self.tools.len()
        )
    }

    async fn current_bindings(&self) -> Result<HashMap<String, ToolBinding>, ErrorData> {
        build_tool_bindings(
            &self.store,
            &self.scope,
            self.instance_id,
            self.session_key.as_deref(),
        )
        .await
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))
    }

    async fn current_tools(&self) -> Result<Vec<Tool>, ErrorData> {
        let bindings = self.current_bindings().await?;
        for tool_name in self
            .session_state_tools
            .keys()
            .chain(self.tool_override_tools.keys())
            .chain(self.prompt_override_tools.keys())
            .chain(self.resource_override_tools.keys())
            .chain(self.openapi_tools.keys())
            .chain(self.service_tools.keys())
            .chain(self.cache_tools.keys())
            .chain(self.search_tools.keys())
        {
            if bindings.contains_key(tool_name) {
                return Err(ErrorData::internal_error(
                    format!(
                        "MCPStore 管理工具与下游工具重名，无法构建 Rust MCP server: tool={tool_name}"
                    ),
                    None,
                ));
            }
        }

        let mut tools = bindings
            .values()
            .map(|binding| binding.tool.clone())
            .collect::<Vec<_>>();
        tools.extend(self.session_state_tools.values().cloned());
        tools.extend(self.tool_override_tools.values().cloned());
        tools.extend(self.prompt_override_tools.values().cloned());
        tools.extend(self.resource_override_tools.values().cloned());
        tools.extend(self.openapi_tools.values().cloned());
        tools.extend(self.service_tools.values().cloned());
        tools.extend(self.cache_tools.values().cloned());
        tools.extend(self.search_tools.values().cloned());
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(tools)
    }

    async fn call_search_tool(
        &self,
        arguments: Map<String, Value>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let top_k = arguments.get("top_k").and_then(Value::as_u64).unwrap_or(10) as usize;
        let tools = self.current_tools().await?;
        let hits = super::search::search_tools(&tools, &query, top_k);
        let payload = serde_json::to_value(&hits).map_err(|error| {
            ErrorData::internal_error(format!("search result serialization failed: {error}"), None)
        })?;
        Ok(CallToolResult::structured(payload))
    }
}

impl ServerHandler for McpStoreServer {
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2026_07_28])
    }

    fn accepted_subscription_filter(
        &self,
        requested: &SubscriptionFilter,
    ) -> Option<SubscriptionFilter> {
        Some(requested.supported_by(&self.get_info().capabilities))
    }

    async fn listen(&self, context: SubscriptionContext) -> Result<(), ErrorData> {
        let active = Arc::new(AtomicBool::new(true));
        self.store
            .event_bus()
            .subscribe(
                mcpstore::events::types::EventKind::McpToolsChanged.as_str(),
                0,
                Arc::new(AggregateToolsChangedNotification {
                    sink: context.sink().clone(),
                    active: Arc::clone(&active),
                    instance_id: self.instance_id,
                }),
            )
            .await;
        context.cancelled().await;
        active.store(false, Ordering::Release);
        Ok(())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("mcpstore", env!("CARGO_PKG_VERSION")))
        .with_instructions(self.instructions())
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        async move { Ok(ListToolsResult::with_all_items(self.current_tools().await?)) }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.bindings
            .get(name)
            .map(|binding| binding.tool.clone())
            .or_else(|| self.session_state_tools.get(name).cloned())
            .or_else(|| self.tool_override_tools.get(name).cloned())
            .or_else(|| self.prompt_override_tools.get(name).cloned())
            .or_else(|| self.resource_override_tools.get(name).cloned())
            .or_else(|| self.openapi_tools.get(name).cloned())
            .or_else(|| self.service_tools.get(name).cloned())
            .or_else(|| self.cache_tools.get(name).cloned())
            .or_else(|| self.search_tools.get(name).cloned())
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let resources = if let Some(session_key) = session_key.as_deref() {
                store.list_resources_in_session(session_key).await
            } else if let Some(instance_id) = instance_id {
                store.list_resources_for_instance(instance_id).await
            } else {
                store.list_resources_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let resources = project_catalog_uris(resources, "uri", false)?;
            let resources = deserialize_items::<Resource>(resources, "resource")?;
            Ok(ListResourcesResult::with_all_items(resources))
        }
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourceTemplatesResult, ErrorData>> + '_
    {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let templates = if let Some(session_key) = session_key.as_deref() {
                store.list_resource_templates_in_session(session_key).await
            } else if let Some(instance_id) = instance_id {
                store
                    .list_resource_templates_for_instance(instance_id)
                    .await
            } else {
                store.list_resource_templates_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let templates = project_catalog_uris(templates, "uriTemplate", true)?;
            let templates = deserialize_items::<ResourceTemplate>(templates, "resource template")?;
            Ok(ListResourceTemplatesResult::with_all_items(templates))
        }
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResponse, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let target_instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let resources = if let Some(session_key) = session_key.as_deref() {
                store.list_resources_in_session(session_key).await
            } else if let Some(instance_id) = target_instance_id {
                store.list_resources_for_instance(instance_id).await
            } else {
                store.list_resources_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let (instance_id, original_uri) =
                resolve_projected_catalog_uri(&resources, "uri", false, &request.uri)?;
            let result = store
                .read_resource_scoped(instance_id, &original_uri)
                .await
                .map_err(map_store_error)?;
            deserialize_item::<ReadResourceResult>(result, "read resource result").map(Into::into)
        }
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListPromptsResult, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let prompts = if let Some(session_key) = session_key.as_deref() {
                store.list_prompts_in_session(session_key).await
            } else if let Some(instance_id) = instance_id {
                store.list_prompts_for_instance(instance_id).await
            } else {
                store.list_prompts_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let prompts = project_prompt_names(prompts)?;
            let prompts = deserialize_items::<Prompt>(prompts, "prompt")?;
            Ok(ListPromptsResult::with_all_items(prompts))
        }
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetPromptResponse, ErrorData>> + '_ {
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let target_instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        async move {
            let arguments = Value::Object(request.arguments.unwrap_or_default());
            let prompts = if let Some(session_key) = session_key.as_deref() {
                store.list_prompts_in_session(session_key).await
            } else if let Some(instance_id) = target_instance_id {
                store.list_prompts_for_instance(instance_id).await
            } else {
                store.list_prompts_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let (instance_id, original_name) = resolve_projected_prompt(&prompts, &request.name)?;
            let result = store
                .get_prompt_scoped(instance_id, &original_name, arguments)
                .await
                .map_err(map_store_error)?;
            deserialize_item::<GetPromptResult>(result, "prompt result").map(Into::into)
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResponse, ErrorData>> + '_ {
        let tool_name = request.name.as_ref().to_string();
        let binding = self.bindings.get(tool_name.as_str()).cloned();
        let is_session_state_tool = self.session_state_tools.contains_key(tool_name.as_str());
        let is_tool_override_tool = self.tool_override_tools.contains_key(tool_name.as_str());
        let is_prompt_override_tool = self.prompt_override_tools.contains_key(tool_name.as_str());
        let is_resource_override_tool = self
            .resource_override_tools
            .contains_key(tool_name.as_str());
        let is_openapi_tool = self.openapi_tools.contains_key(tool_name.as_str());
        let is_service_tool = self.service_tools.contains_key(tool_name.as_str());
        let is_cache_tool = self.cache_tools.contains_key(tool_name.as_str());
        let is_search_tool = self.search_tools.contains_key(tool_name.as_str());
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let default_session_key = self.session_key.clone();
        let meta = request.meta.clone();
        let arguments = request.arguments.unwrap_or_default();
        async move {
            if is_session_state_tool {
                return call_session_state_tool(
                    &store,
                    &tool_name,
                    meta.as_ref(),
                    arguments,
                    default_session_key.as_deref(),
                )
                .await
                .map(Into::into);
            }
            if is_tool_override_tool {
                return call_tool_override_tool(&store, &tool_name, &scope, instance_id, arguments)
                    .await
                    .map(Into::into);
            }
            if is_prompt_override_tool {
                return call_prompt_override_tool(
                    &store,
                    &tool_name,
                    &scope,
                    instance_id,
                    arguments,
                )
                .await
                .map(Into::into);
            }
            if is_resource_override_tool {
                if tool_name.starts_with("mcpstore_resource_template_override_") {
                    return call_resource_template_override_tool(
                        &store,
                        &tool_name,
                        &scope,
                        instance_id,
                        arguments,
                    )
                    .await
                    .map(Into::into);
                }
                return call_resource_override_tool(
                    &store,
                    &tool_name,
                    &scope,
                    instance_id,
                    arguments,
                )
                .await
                .map(Into::into);
            }
            if is_openapi_tool {
                return call_openapi_tool(&store, &tool_name, arguments)
                    .await
                    .map(Into::into);
            }
            if is_service_tool {
                return call_service_tool(&store, &tool_name, &scope, arguments)
                    .await
                    .map(Into::into);
            }
            if is_cache_tool {
                return call_cache_tool(&store, &tool_name, arguments)
                    .await
                    .map(Into::into);
            }
            if is_search_tool {
                return self.call_search_tool(arguments).await.map(Into::into);
            }

            let (args, session_key) = extract_business_session_key(
                meta.as_ref(),
                arguments,
                default_session_key.as_deref(),
            );
            let binding = if let Some(session_key) = session_key.as_deref() {
                build_tool_bindings(&store, &scope, instance_id, Some(session_key))
                    .await
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?
                    .remove(tool_name.as_str())
            } else {
                binding.or(build_tool_bindings(&store, &scope, instance_id, None)
                    .await
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?
                    .remove(tool_name.as_str()))
            }
            .ok_or_else(|| ErrorData::invalid_params(format!("未知工具: {tool_name}"), None))?;
            let execution = match store
                .start_tool_execution(
                    binding.instance_id,
                    &binding.tool_name,
                    args,
                    meta.clone(),
                    McpExecutionOptions::default(),
                )
                .await
            {
                Ok(execution) => execution,
                Err(error) => return Ok(llm_error_result(&error)),
            };
            let result = match execution.wait().await {
                Ok(McpToolExecution::Immediate { result }) => result,
                Ok(McpToolExecution::Task { .. }) => {
                    return Ok(llm_error_result(&mcpstore::Error::new(
                        mcpstore::error::FailureCode::ToolFailed,
                        "aggregate tool call unexpectedly returned a task",
                    )));
                }
                Err(error) => return Ok(llm_error_result(&error)),
            };

            let mut content = Vec::with_capacity(result.content.len());
            for item in result.content {
                match item {
                    ContentItem::Text { text, .. } => {
                        content.push(ContentBlock::text(text));
                    }
                    ContentItem::Image {
                        data, mime_type, ..
                    } => {
                        content.push(ContentBlock::image(data, mime_type));
                    }
                    ContentItem::Audio {
                        data, mime_type, ..
                    } => {
                        content.push(ContentBlock::audio(data, mime_type));
                    }
                    ContentItem::Resource { resource, .. } => {
                        content.push(match serde_json::from_value::<ResourceContents>(resource) {
                            Ok(resource) => ContentBlock::resource(resource),
                            Err(error) => ContentBlock::text(format!(
                                "Failed to decode resource content: {error}"
                            )),
                        });
                    }
                    ContentItem::ResourceLink { resource, .. } => {
                        content.push(match serde_json::from_value::<Resource>(resource) {
                            Ok(resource) => ContentBlock::resource_link(resource),
                            Err(error) => ContentBlock::text(format!(
                                "Failed to decode resource link: {error}"
                            )),
                        });
                    }
                }
            }

            Ok(CallToolResponse::Complete(if result.is_error {
                CallToolResult::error(content)
            } else {
                CallToolResult::success(content)
            }))
        }
    }
}

/// LLM-facing tool failure: `isError: true` plus one text block carrying the
/// structured failure payload (code/message/hint/retryable).
pub(super) fn llm_error_result(error: &mcpstore::Error) -> CallToolResponse {
    let code = error.code();
    let payload = json!({
        "code": code.as_str(),
        "message": error.message(),
        "hint": code.hint(),
        "retryable": error.retryable(),
    });
    CallToolResponse::Complete(CallToolResult::error(vec![ContentBlock::text(
        payload.to_string(),
    )]))
}
