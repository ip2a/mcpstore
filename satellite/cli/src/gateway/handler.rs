use super::catalog::{
    apply_service_overrides, project_catalog_uris, project_prompt_names,
    resolve_projected_catalog_uri, resolve_projected_prompt, service_override_map,
};
use super::tools::{
    build_kv_tools, build_openapi_tools, build_prompt_override_tools,
    build_resource_override_tools, build_resource_template_override_tools, build_search_tools,
    build_service_tools, build_session_state_tools, build_tool_override_tools, deserialize_item,
    deserialize_items,
};
use super::tools::{
    call_kv_tool, call_openapi_tool, call_prompt_override_tool, call_resource_override_tool,
    call_resource_template_override_tool, call_service_tool, call_session_state_tool,
    call_tool_override_tool, extract_business_session_key, map_store_error,
};
use super::transport::{build_tool_bindings, connect_target_instances};
use super::*;
use serde_json::json;

impl GatewayServer {
    pub(crate) async fn from_store(
        store: Arc<MCPStore>,
        options: &GatewayOptions,
        caller: Option<String>,
        call_log: Arc<GatewayCallLog>,
    ) -> Result<Self, BoxErr> {
        let scope = options.scope.clone();
        let instance_id = options.instance_id;
        let session_key = options.session_key.clone();
        let down = connect_target_instances(&store, &scope, instance_id).await?;
        if let Some(session_key) = session_key.as_deref() {
            store.session_by_key(session_key).status().await?;
        }
        let (bindings, issues) =
            build_tool_bindings(&store, &scope, instance_id, session_key.as_deref(), &down).await?;
        let management = Arc::new(GatewayManagementTools {
            session_state_tools: Arc::new(if options.expose_session_state_tools {
                build_session_state_tools()
            } else {
                HashMap::new()
            }),
            tool_override_tools: Arc::new(if options.expose_tool_override_tools {
                build_tool_override_tools()
            } else {
                HashMap::new()
            }),
            prompt_override_tools: Arc::new(if options.expose_prompt_override_tools {
                build_prompt_override_tools()
            } else {
                HashMap::new()
            }),
            resource_override_tools: Arc::new(if options.expose_resource_override_tools {
                build_resource_override_tools()
                    .into_iter()
                    .chain(build_resource_template_override_tools())
                    .collect()
            } else {
                HashMap::new()
            }),
            openapi_tools: Arc::new(if options.expose_openapi_tools {
                build_openapi_tools()
            } else {
                HashMap::new()
            }),
            service_tools: Arc::new(if options.expose_service_tools {
                build_service_tools()
            } else {
                HashMap::new()
            }),
            kv_tools: Arc::new(if options.expose_kv_tools {
                build_kv_tools()
            } else {
                HashMap::new()
            }),
            search_tools: Arc::new(if options.expose_search_tools {
                build_search_tools()
            } else {
                HashMap::new()
            }),
        });
        // A management tool colliding with a downstream name fails the mount
        // at build (the surface would be ambiguous from day one).
        let tools =
            merged_tools(&bindings, &management).map_err(|error| -> BoxErr { error.into() })?;
        let catalog = GatewayCatalog::new(
            Arc::clone(&store),
            scope.clone(),
            instance_id,
            session_key.clone(),
            Arc::new(down),
            Arc::clone(&management),
            CatalogSnapshot { bindings, tools },
        );
        if let Ok(mut projection) = catalog.projection.lock() {
            *projection = issues;
        }
        // Mount-lifetime invalidation: tool syncs and override writes publish
        // McpToolsChanged on the local bus (the store owns the upstream
        // connections); mark the snapshot dirty and warm a rebuild.
        store
            .event_bus()
            .subscribe(
                mcpstore::events::types::EventKind::McpToolsChanged.as_str(),
                0,
                Arc::new(CatalogInvalidator {
                    catalog: Arc::downgrade(&catalog),
                }),
            )
            .await;

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
            catalog,
            management,
            caller,
            call_log,
        })
    }

    /// Latest projection issues (down/skipped/degraded) for the panel view.
    pub(crate) fn projection_handle(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<Vec<ProjectionIssue>>> {
        Arc::clone(&self.catalog.projection)
    }

    /// Shared catalog handle: the reconciliation tick watches scopes.toml
    /// through it (daemon-side override writes land there via writeback) and
    /// health observability reads the rebuild counter.
    pub(crate) fn catalog_handle(&self) -> Arc<GatewayCatalog> {
        Arc::clone(&self.catalog)
    }

    /// Caller identity of this mount (`/mcp/<id>`); None on the generic mount.
    pub(crate) fn caller_name(&self) -> Option<String> {
        self.caller.clone()
    }

    fn instructions(&self) -> String {
        format!(
            "Rust MCPStore server. scope={} tool_count={}",
            self.scope_label,
            self.catalog.current().tools.len()
        )
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
        let tools = self.catalog.ensure().await.tools.clone();
        let hits = super::search::search_tools(&tools, &query, top_k);
        let payload = serde_json::to_value(&hits).map_err(|error| {
            ErrorData::internal_error(format!("search result serialization failed: {error}"), None)
        })?;
        Ok(CallToolResult::structured(payload))
    }
}

impl ServerHandler for GatewayServer {
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        // Inclusive gateway: advertise every revision rmcp knows, so the
        // version echoed by the initialize handshake is always accepted again
        // by per-request inline negotiation.
        Cow::Borrowed(ProtocolVersion::KNOWN_VERSIONS)
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
                Arc::new(GatewayToolsChangedNotification {
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

    fn get_info(&self) -> McpServerConfig {
        McpServerConfig::new(
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
        async move {
            Ok(ListToolsResult::with_all_items(
                self.catalog.ensure().await.tools.clone(),
            ))
        }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.catalog
            .current()
            .bindings
            .get(name)
            .map(|binding| binding.tool.clone())
            .or_else(|| self.management.session_state_tools.get(name).cloned())
            .or_else(|| self.management.tool_override_tools.get(name).cloned())
            .or_else(|| self.management.prompt_override_tools.get(name).cloned())
            .or_else(|| self.management.resource_override_tools.get(name).cloned())
            .or_else(|| self.management.openapi_tools.get(name).cloned())
            .or_else(|| self.management.service_tools.get(name).cloned())
            .or_else(|| self.management.kv_tools.get(name).cloned())
            .or_else(|| self.management.search_tools.get(name).cloned())
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
            let resources = apply_service_overrides(
                resources,
                &service_override_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
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
            let templates = apply_service_overrides(
                templates,
                &service_override_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
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
        let call_log = Arc::clone(&self.call_log);
        let caller = self.caller.clone();
        async move {
            let call_start = std::time::Instant::now();
            let resources = if let Some(session_key) = session_key.as_deref() {
                store.list_resources_in_session(session_key).await
            } else if let Some(instance_id) = target_instance_id {
                store.list_resources_for_instance(instance_id).await
            } else {
                store.list_resources_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let resources = apply_service_overrides(
                resources,
                &service_override_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
            let (instance_id, original_uri, service_name) =
                resolve_projected_catalog_uri(&resources, "uri", false, &request.uri)?;
            let result = store.read_resource_scoped(instance_id, &original_uri).await;
            call_log.record(
                caller.as_deref(),
                GatewayCallKind::Resource,
                &original_uri,
                instance_id,
                &service_name,
                call_start.elapsed(),
                result.is_ok(),
            );
            deserialize_item::<ReadResourceResult>(
                result.map_err(map_store_error)?,
                "read resource result",
            )
            .map(Into::into)
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
            let prompts = apply_service_overrides(
                prompts,
                &service_override_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
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
        let call_log = Arc::clone(&self.call_log);
        let caller = self.caller.clone();
        async move {
            let call_start = std::time::Instant::now();
            let arguments = Value::Object(request.arguments.unwrap_or_default());
            let prompts = if let Some(session_key) = session_key.as_deref() {
                store.list_prompts_in_session(session_key).await
            } else if let Some(instance_id) = target_instance_id {
                store.list_prompts_for_instance(instance_id).await
            } else {
                store.list_prompts_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let prompts = apply_service_overrides(
                prompts,
                &service_override_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
            let (instance_id, original_name, service_name) =
                resolve_projected_prompt(&prompts, &request.name)?;
            let result = store
                .get_prompt_scoped(instance_id, &original_name, arguments)
                .await;
            call_log.record(
                caller.as_deref(),
                GatewayCallKind::Prompt,
                &original_name,
                instance_id,
                &service_name,
                call_start.elapsed(),
                result.is_ok(),
            );
            deserialize_item::<GetPromptResult>(result.map_err(map_store_error)?, "prompt result")
                .map(Into::into)
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResponse, ErrorData>> + '_ {
        let tool_name = request.name.as_ref().to_string();
        let is_session_state_tool = self
            .management
            .session_state_tools
            .contains_key(tool_name.as_str());
        let is_tool_override_tool = self
            .management
            .tool_override_tools
            .contains_key(tool_name.as_str());
        let is_prompt_override_tool = self
            .management
            .prompt_override_tools
            .contains_key(tool_name.as_str());
        let is_resource_override_tool = self
            .management
            .resource_override_tools
            .contains_key(tool_name.as_str());
        let is_openapi_tool = self
            .management
            .openapi_tools
            .contains_key(tool_name.as_str());
        let is_service_tool = self
            .management
            .service_tools
            .contains_key(tool_name.as_str());
        let is_cache_tool = self.management.kv_tools.contains_key(tool_name.as_str());
        let is_search_tool = self
            .management
            .search_tools
            .contains_key(tool_name.as_str());
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let default_session_key = self.session_key.clone();
        let meta = request.meta.clone();
        let arguments = request.arguments.unwrap_or_default();
        let management_tool = is_session_state_tool
            || is_tool_override_tool
            || is_prompt_override_tool
            || is_resource_override_tool
            || is_openapi_tool
            || is_service_tool
            || is_cache_tool
            || is_search_tool;
        let management_store = Arc::clone(&store);
        let management_caller = self.caller.clone();
        let management_scope = scope_label(&scope);
        let management_tool_name = tool_name.clone();
        let management_args = management_tool.then(|| Value::Object(arguments.clone()));
        let management_started = std::time::Instant::now();
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
                return call_kv_tool(&store, &tool_name, arguments)
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
            // A call naming its own business session key sees a different tool
            // surface than the mount snapshot: resolve live. Otherwise resolve
            // against the mount snapshot; a miss refreshes the snapshot once
            // (override renames and new tools apply on the next call without
            // waiting for an event).
            let binding = if session_key.as_deref() != self.session_key.as_deref() {
                match self.catalog.fresh_bindings(session_key.as_deref()).await {
                    Ok(mut bindings) => bindings.remove(tool_name.as_str()).ok_or_else(|| {
                        super::tools::unknown_tool_error(&tool_name, self.instance_id.is_some())
                    })?,
                    Err(error) => return Err(ErrorData::internal_error(error.to_string(), None)),
                }
            } else {
                let snapshot = self.catalog.ensure().await;
                match snapshot.bindings.get(tool_name.as_str()) {
                    Some(binding) => binding.clone(),
                    None => {
                        self.catalog.invalidate();
                        let snapshot = self.catalog.ensure().await;
                        snapshot
                            .bindings
                            .get(tool_name.as_str())
                            .cloned()
                            .ok_or_else(|| {
                                super::tools::unknown_tool_error(
                                    &tool_name,
                                    self.instance_id.is_some(),
                                )
                            })?
                    }
                }
            };
            let call_start = std::time::Instant::now();
            let record_binding = binding.clone();
            let mut execution_options = McpExecutionOptions::default();
            if let Some(caller) = self.caller.clone() {
                execution_options = execution_options.with_caller(caller);
            }
            if let Some(session_key) = session_key.as_deref() {
                execution_options = execution_options.with_session_key(session_key);
            }
            let response: Result<CallToolResponse, ErrorData> =
                tokio::time::timeout(super::tools::GATEWAY_TOOL_CALL_BUDGET, async {
                    let execution = match store
                        .start_tool_execution(
                            binding.instance_id,
                            &binding.tool_name,
                            args,
                            meta.clone(),
                            execution_options,
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
                                "gateway tool call unexpectedly returned a task",
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
                                content.push(
                                    match serde_json::from_value::<ResourceContents>(resource) {
                                        Ok(resource) => ContentBlock::resource(resource),
                                        Err(error) => ContentBlock::text(format!(
                                            "Failed to decode resource content: {error}"
                                        )),
                                    },
                                );
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

                    let mut call_result = if result.is_error {
                        CallToolResult::error(content)
                    } else {
                        CallToolResult::success(content)
                    };
                    call_result.structured_content = result.structured_content;

                    Ok(CallToolResponse::Complete(call_result))
                })
                .await
                .unwrap_or_else(|_| {
                    // the ladder's outer belt: the store deadline should have
                    // fired already; if the gateway budget expires instead, the
                    // store layer itself hung — answer with the translated
                    // timeout rather than letting the agent's client time out
                    // blind
                    Ok(llm_error_result(&mcpstore::Error::new(
                        mcpstore::error::FailureCode::CallTimedOut,
                        "gateway tool-call budget expired while the upstream call was pending",
                    )))
                });
            let success = match &response {
                Ok(CallToolResponse::Complete(result)) => result.is_error != Some(true),
                _ => false,
            };
            self.call_log.record(
                self.caller.as_deref(),
                GatewayCallKind::Tool,
                &tool_name,
                record_binding.instance_id,
                &record_binding.service_name,
                call_start.elapsed(),
                success,
            );
            if management_tool {
                record_management_call(
                    &management_store,
                    management_caller.as_deref(),
                    &management_scope,
                    &management_tool_name,
                    management_args,
                    management_started.elapsed(),
                    &response,
                );
            }
            response
        }
    }
}

fn scope_label(scope: &mcpstore::ScopeRef) -> String {
    match scope {
        mcpstore::ScopeRef::Store => "store".to_string(),
        mcpstore::ScopeRef::Agent { agent_id } => format!("agent:{agent_id}"),
    }
}

/// Gateway management tools (session-state/override/openapi/service/kv/search)
/// execute store methods directly instead of the business execution funnel, so
/// they are audited here. One record per management call, full args and result.
fn record_management_call(
    store: &MCPStore,
    caller: Option<&str>,
    scope: &str,
    tool_name: &str,
    args: Option<Value>,
    duration: std::time::Duration,
    response: &Result<CallToolResponse, ErrorData>,
) {
    let Some(telemetry) = store.telemetry() else {
        return;
    };
    let mut record = mcpstore::telemetry::RequestRecord::new("management", tool_name);
    record.duration_ms = duration.as_millis() as i64;
    record.caller = caller.map(str::to_string);
    record.scope = Some(scope.to_string());
    record.args_json = args.map(|args| args.to_string());
    match response {
        Ok(CallToolResponse::Complete(result)) => {
            record.success = result.is_error != Some(true);
            record.result_json = serde_json::to_string(result).ok();
        }
        Ok(_) => {
            record.success = false;
        }
        Err(error) => {
            record.success = false;
            record.error_summary = Some(error.to_string());
        }
    }
    telemetry.record(record);
}

/// LLM-facing tool failure: `isError: true` plus one text block carrying the
/// structured failure payload (code/message/hint/retryable).
pub(super) fn llm_error_result(error: &mcpstore::Error) -> CallToolResponse {
    let code = error.code();
    let mut payload = serde_json::Map::new();
    payload.insert("code".into(), json!(code.as_str()));
    payload.insert("message".into(), json!(error.message()));
    payload.insert("retryable".into(), json!(error.retryable()));
    if let Some(hint) = super::tools::agent_hint(code).or(code.hint()) {
        payload.insert("hint".into(), json!(hint));
    }
    CallToolResponse::Complete(CallToolResult::error(vec![ContentBlock::text(
        Value::Object(payload).to_string(),
    )]))
}
