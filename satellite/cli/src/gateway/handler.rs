use super::catalog::{
    apply_service_routes, project_catalog_uris, project_prompt_names,
    resolve_projected_catalog_uri, resolve_projected_prompt, service_route_map,
};
use super::tools::{
    build_kv_tools, build_openapi_tools, build_prompt_route_tools,
    build_resource_route_tools, build_resource_template_route_tools, build_search_helpers,
    build_service_tools, build_session_state_tools, build_tool_route_tools, deserialize_item,
    deserialize_items,
};
use super::search::{
    entries_from_prompts, entries_from_resources, entries_from_templates, parse_search_arguments,
    rank, resource_view, search_payload, search_text, Entry, EntryKind,
};
use super::tools::{
    call_kv_tool, call_openapi_tool, call_prompt_route_tool, call_resource_route_tool,
    call_resource_template_override_tool, call_service_tool, call_session_state_tool,
    call_tool_route_tool, extract_business_session_key, map_store_error,
};
use super::transport::build_tool_bindings;
use super::*;
use serde_json::json;

fn gateway_frame_call_id(
    context: &rmcp::service::RequestContext<rmcp::RoleServer>,
) -> Option<String> {
    context
        .extensions
        .get::<axum::http::request::Parts>()?
        .extensions
        .get::<super::transport::GatewayFrameCallId>()
        .map(|id| id.0.clone())
}

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
        if let Some(session_key) = session_key.as_deref() {
            store.session_by_key(session_key).status().await?;
        }
        let (bindings, issues) =
            build_tool_bindings(&store, &scope, instance_id, session_key.as_deref()).await?;
        let management = Arc::new(GatewayManagementTools {
            session_state_tools: Arc::new(if options.expose_session_state_tools {
                build_session_state_tools()
            } else {
                HashMap::new()
            }),
            tool_route_tools: Arc::new(if options.expose_tool_route_tools {
                build_tool_route_tools()
            } else {
                HashMap::new()
            }),
            prompt_route_tools: Arc::new(if options.expose_prompt_route_tools {
                build_prompt_route_tools()
            } else {
                HashMap::new()
            }),
            resource_route_tools: Arc::new(if options.expose_resource_route_tools {
                build_resource_route_tools()
                    .into_iter()
                    .chain(build_resource_template_route_tools())
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
        });
        // Search helpers are not a management family: permission only at
        // build, residency decided per kind by the activation truth table.
        let search_enabled = options.expose_search_tools;
        let search_helper_defs = Arc::new(if search_enabled {
            build_search_helpers()
        } else {
            HashMap::new()
        });
        // A management tool colliding with a downstream name fails the mount
        // at build (the surface would be ambiguous from day one). Discovery
        // rows start unknown: helpers activate as complete native catalog
        // pages are observed while serving.
        let (snapshot, mut projection) = GatewayCatalog::build_snapshot(
            bindings,
            &management,
            search_enabled,
            &search_helper_defs,
            &DiscoveryRows::default(),
        )
        .map_err(|error| -> BoxErr { error.into() })?;
        projection.extend(issues);
        let catalog = GatewayCatalog::new(
            Arc::clone(&store),
            scope.clone(),
            instance_id,
            session_key.clone(),
            Arc::clone(&management),
            search_enabled,
            search_helper_defs,
            snapshot,
        );
        if let Ok(mut report) = catalog.projection.lock() {
            *report = projection;
        }
        // Mount-lifetime invalidation: tool syncs and route writes publish
        // McpToolsChanged on the local bus (the store owns the upstream
        // connections); mark the snapshot dirty and warm a rebuild. Upstream
        // prompt/resource events clear the affected discovery columns.
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
        store
            .event_bus()
            .subscribe(
                mcpstore::events::types::EventKind::McpPromptsChanged.as_str(),
                0,
                Arc::new(DiscoveryInvalidator {
                    catalog: Arc::downgrade(&catalog),
                    event_kind: mcpstore::events::types::EventKind::McpPromptsChanged.as_str(),
                    kinds: &[EntryKind::Prompt],
                }),
            )
            .await;
        store
            .event_bus()
            .subscribe(
                mcpstore::events::types::EventKind::McpResourcesChanged.as_str(),
                0,
                Arc::new(DiscoveryInvalidator {
                    catalog: Arc::downgrade(&catalog),
                    event_kind: mcpstore::events::types::EventKind::McpResourcesChanged.as_str(),
                    kinds: &[EntryKind::Resource, EntryKind::ResourceTemplate],
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

    /// Shared catalog handle: the reconciliation tick watches route.toml
    /// through it (daemon-side route writes land there via writeback) and
    /// health observability reads the rebuild counter.
    pub(crate) fn catalog_handle(&self) -> Arc<GatewayCatalog> {
        Arc::clone(&self.catalog)
    }

    /// Caller identity of this mount (`/mcp/<id>`); None on the generic mount.
    pub(crate) fn caller_name(&self) -> Option<String> {
        self.caller.clone()
    }

    fn instructions(&self) -> String {
        let mut instructions = format!(
            "Rust MCPStore server. scope={} tool_count={}",
            self.scope_label,
            self.catalog.current().tools.len()
        );
        if self.catalog.search_allowed() {
            instructions.push_str(
                " Search helpers (search_tools, search_prompt, search_resource) appear on this mount once the kind's visible catalog reaches 10 entries; prompts and resources only after a complete native catalog page was observed.",
            );
        }
        instructions
    }

    /// One of the three search helpers. Dispatch only routes names present in
    /// the served snapshot, so an inactive or suppressed helper never reaches
    /// this path — it falls through to the business unknown-tool route.
    async fn call_search_helper(
        &self,
        helper: &str,
        meta: Option<&rmcp::model::RequestMetaObject>,
        arguments: Map<String, Value>,
    ) -> Result<CallToolResult, ErrorData> {
        // The helpers search the mount's own view. A call explicitly naming a
        // different business session key targets a surface the helper cannot
        // see — reject instead of silently searching the mount view.
        let (args, session_key) =
            extract_business_session_key(meta, arguments, self.session_key.as_deref());
        if session_key.as_deref() != self.session_key.as_deref() {
            return Err(ErrorData::invalid_params(
                format!(
                    "search helpers only see this mount's own view; session key '{}' is a different surface",
                    session_key.as_deref().unwrap_or_default()
                ),
                None,
            ));
        }
        let Value::Object(args) = args else {
            return Err(ErrorData::invalid_params(
                "search arguments must be an object",
                None,
            ));
        };
        let request =
            parse_search_arguments(&args).map_err(|error| ErrorData::invalid_params(error, None))?;
        let snapshot = self.catalog.ensure().await;
        let tool_entries = snapshot.tool_entries.clone();
        match helper {
            SEARCH_TOOLS_HELPER => {
                // Tools are always completely known from the snapshot.
                let refs: Vec<&Entry> = tool_entries.iter().collect();
                let hits = rank(&refs, &request.query, request.limit);
                Ok(self.search_answer(&hits, &[]))
            }
            SEARCH_PROMPT_HELPER => {
                if !self.catalog.verify_discovery_rows(&[EntryKind::Prompt]).await {
                    return Ok(self.search_answer(&[], &["prompt"]));
                }
                let rows = self.catalog.search_rows();
                match rows.prompts {
                    Some(prompts) => {
                        let refs: Vec<&Entry> = prompts.iter().collect();
                        let hits = rank(&refs, &request.query, request.limit);
                        Ok(self.search_answer(&hits, &[]))
                    }
                    None => Ok(self.search_answer(&[], &["prompt"])),
                }
            }
            SEARCH_RESOURCE_HELPER => {
                let kinds = [EntryKind::Resource, EntryKind::ResourceTemplate];
                if !self.catalog.verify_discovery_rows(&kinds).await {
                    return Ok(self.search_answer(&[], &["resource", "resource_template"]));
                }
                let rows = self.catalog.search_rows();
                match resource_view(&rows) {
                    Some((entries, resources_known, templates_known)) => {
                        let hits = rank(&entries, &request.query, request.limit);
                        let mut unknown_kinds = Vec::new();
                        if !resources_known {
                            unknown_kinds.push("resource");
                        }
                        if !templates_known {
                            unknown_kinds.push("resource_template");
                        }
                        Ok(self.search_answer(&hits, &unknown_kinds))
                    }
                    None => Ok(self.search_answer(&[], &["resource", "resource_template"])),
                }
            }
            other => unreachable!("dispatch only routes the three published helper names: {other}"),
        }
    }

    fn search_answer(&self, hits: &[&Entry], unknown_kinds: &[&str]) -> CallToolResult {
        let mut result = CallToolResult::structured(search_payload(hits, unknown_kinds));
        // The structured payload is authoritative; the text companion renders
        // the hits readably instead of dumping raw JSON.
        result.content = vec![ContentBlock::text(search_text(hits, unknown_kinds))];
        result
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
        let mut helper_changes = self.catalog.subscribe_search_changes();
        loop {
            tokio::select! {
                _ = context.cancelled() => break,
                changed = helper_changes.changed() => {
                    // The published helper set moved: the tools surface
                    // changed for this client too.
                    if changed.is_err() || context.sink().notify_tool_list_changed().await.is_err() {
                        break;
                    }
                }
            }
        }
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
        let catalog = self.catalog.current();
        catalog
            .bindings
            .get(name)
            .map(|binding| binding.tool.clone())
            .or_else(|| {
                catalog
                    .search_helpers
                    .iter()
                    .find(|tool| tool.name.as_ref() == name)
                    .cloned()
            })
            .or_else(|| self.management.session_state_tools.get(name).cloned())
            .or_else(|| self.management.tool_route_tools.get(name).cloned())
            .or_else(|| self.management.prompt_route_tools.get(name).cloned())
            .or_else(|| self.management.resource_route_tools.get(name).cloned())
            .or_else(|| self.management.openapi_tools.get(name).cloned())
            .or_else(|| self.management.service_tools.get(name).cloned())
            .or_else(|| self.management.kv_tools.get(name).cloned())
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
            // Discovery sampling: capture the authority state before the
            // native catalog read so the rows publish only when nothing moved.
            let capture = if self.catalog.search_allowed() {
                self.catalog.discovery_capture().await.ok()
            } else {
                None
            };
            let (resources, skipped_instances) = if let Some(session_key) = session_key.as_deref()
            {
                store
                    .list_resources_in_session(session_key)
                    .await
                    .map(|entries| (entries, 0usize))
            } else if let Some(instance_id) = instance_id {
                store
                    .list_resources_for_instance(instance_id)
                    .await
                    .map(|entries| (entries, 0usize))
            } else {
                store
                    .list_resources_scoped_page(&scope)
                    .await
                    .map(|page| (page.entries, page.skipped_instances))
            }
            .map_err(map_store_error)?;
            let resources = apply_service_routes(
                resources,
                &service_route_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
            let resources = project_catalog_uris(resources, "uri", false)?;
            let resources = deserialize_items::<Resource>(resources, "resource")?;
            // Session/instance views fail loud (Ok = complete); a scope page
            // with skipped instances is partial and never publishes as the
            // catalog.
            if let Some(capture) = capture.filter(|_| skipped_instances == 0) {
                let entries = entries_from_resources(&resources);
                self.catalog
                    .publish_discovery(capture, EntryKind::Resource, entries.into())
                    .await;
            }
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
            let capture = if self.catalog.search_allowed() {
                self.catalog.discovery_capture().await.ok()
            } else {
                None
            };
            let (templates, skipped_instances) = if let Some(session_key) = session_key.as_deref()
            {
                store
                    .list_resource_templates_in_session(session_key)
                    .await
                    .map(|entries| (entries, 0usize))
            } else if let Some(instance_id) = instance_id {
                store
                    .list_resource_templates_for_instance(instance_id)
                    .await
                    .map(|entries| (entries, 0usize))
            } else {
                store
                    .list_resource_templates_scoped_page(&scope)
                    .await
                    .map(|page| (page.entries, page.skipped_instances))
            }
            .map_err(map_store_error)?;
            let templates = apply_service_routes(
                templates,
                &service_route_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
            let templates = project_catalog_uris(templates, "uriTemplate", true)?;
            let templates = deserialize_items::<ResourceTemplate>(templates, "resource template")?;
            if let Some(capture) = capture.filter(|_| skipped_instances == 0) {
                let entries = entries_from_templates(&templates);
                self.catalog
                    .publish_discovery(capture, EntryKind::ResourceTemplate, entries.into())
                    .await;
            }
            Ok(ListResourceTemplatesResult::with_all_items(templates))
        }
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResponse, ErrorData>> + '_ {
        let frame_call_id = gateway_frame_call_id(&context);
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let target_instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        let call_log = Arc::clone(&self.call_log);
        let caller = self.caller.clone();
        async move {
            let mut call = call_log.begin(
                caller.as_deref(),
                GatewayCallKind::Resource,
                &request.uri,
                target_instance_id,
                "",
                frame_call_id.as_deref(),
            );
            let resources = if let Some(session_key) = session_key.as_deref() {
                store.list_resources_in_session(session_key).await
            } else if let Some(instance_id) = target_instance_id {
                store.list_resources_for_instance(instance_id).await
            } else {
                store.list_resources_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let resources = apply_service_routes(
                resources,
                &service_route_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
            let (instance_id, original_uri, service_name) =
                resolve_projected_catalog_uri(&resources, "uri", false, &request.uri)?;
            call.record.instance_id = Some(instance_id);
            call.record.service_name = service_name.clone();
            call.record.upstream_name = Some(original_uri.clone());
            call.record.namespace_applied = request.uri != original_uri;
            call.record.routing_ms = Some(call.started.elapsed().as_millis() as u64);
            call.publish();
            call.set_phase(super::GatewayCallPhase::CallingUpstream);
            let result = store
                .read_resource_scoped_with_trace(
                    instance_id,
                    &original_uri,
                    Arc::clone(&call.trace),
                )
                .await;
            let error = result.as_ref().err().map(ToString::to_string);
            call.finish(result.is_ok(), error);
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
            let capture = if self.catalog.search_allowed() {
                self.catalog.discovery_capture().await.ok()
            } else {
                None
            };
            let (prompts, skipped_instances) = if let Some(session_key) = session_key.as_deref() {
                store
                    .list_prompts_in_session(session_key)
                    .await
                    .map(|entries| (entries, 0usize))
            } else if let Some(instance_id) = instance_id {
                store
                    .list_prompts_for_instance(instance_id)
                    .await
                    .map(|entries| (entries, 0usize))
            } else {
                store
                    .list_prompts_scoped_page(&scope)
                    .await
                    .map(|page| (page.entries, page.skipped_instances))
            }
            .map_err(map_store_error)?;
            let prompts = apply_service_routes(
                prompts,
                &service_route_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
            let prompts = project_prompt_names(prompts)?;
            let prompts = deserialize_items::<Prompt>(prompts, "prompt")?;
            if let Some(capture) = capture.filter(|_| skipped_instances == 0) {
                let entries = entries_from_prompts(&prompts);
                self.catalog
                    .publish_discovery(capture, EntryKind::Prompt, entries.into())
                    .await;
            }
            Ok(ListPromptsResult::with_all_items(prompts))
        }
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetPromptResponse, ErrorData>> + '_ {
        let frame_call_id = gateway_frame_call_id(&context);
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let target_instance_id = self.instance_id;
        let session_key = self.session_key.clone();
        let call_log = Arc::clone(&self.call_log);
        let caller = self.caller.clone();
        async move {
            let mut call = call_log.begin(
                caller.as_deref(),
                GatewayCallKind::Prompt,
                &request.name,
                target_instance_id,
                "",
                frame_call_id.as_deref(),
            );
            let arguments = Value::Object(request.arguments.unwrap_or_default());
            let prompts = if let Some(session_key) = session_key.as_deref() {
                store.list_prompts_in_session(session_key).await
            } else if let Some(instance_id) = target_instance_id {
                store.list_prompts_for_instance(instance_id).await
            } else {
                store.list_prompts_scoped(&scope).await
            }
            .map_err(map_store_error)?;
            let prompts = apply_service_routes(
                prompts,
                &service_route_map(&store)
                    .await
                    .map_err(map_store_error)?,
            )?;
            let (instance_id, original_name, service_name) =
                resolve_projected_prompt(&prompts, &request.name)?;
            call.record.instance_id = Some(instance_id);
            call.record.service_name = service_name.clone();
            call.record.upstream_name = Some(original_name.clone());
            call.record.namespace_applied = request.name != original_name;
            call.record.routing_ms = Some(call.started.elapsed().as_millis() as u64);
            call.publish();
            call.set_phase(super::GatewayCallPhase::CallingUpstream);
            let result = store
                .get_prompt_scoped_with_trace(
                    instance_id,
                    &original_name,
                    arguments,
                    Arc::clone(&call.trace),
                )
                .await;
            let error = result.as_ref().err().map(ToString::to_string);
            call.finish(result.is_ok(), error);
            deserialize_item::<GetPromptResult>(result.map_err(map_store_error)?, "prompt result")
                .map(Into::into)
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResponse, ErrorData>> + '_ {
        let frame_call_id = gateway_frame_call_id(&context);
        let tool_name = request.name.as_ref().to_string();
        let is_session_state_tool = self
            .management
            .session_state_tools
            .contains_key(tool_name.as_str());
        let is_tool_route_tool = self
            .management
            .tool_route_tools
            .contains_key(tool_name.as_str());
        let is_prompt_override_tool = self
            .management
            .prompt_route_tools
            .contains_key(tool_name.as_str());
        let is_resource_override_tool = self
            .management
            .resource_route_tools
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
        // Search helpers are snapshot-resident, not families: the served
        // snapshot decides whether the name is a helper right now. An
        // inactive helper name falls through to the business unknown-tool
        // route below.
        let search_helper = self
            .catalog
            .current()
            .search_helpers
            .iter()
            .find(|tool| tool.name.as_ref() == tool_name.as_str())
            .map(|tool| tool.name.to_string());
        let store = Arc::clone(&self.store);
        let scope = self.scope.clone();
        let instance_id = self.instance_id;
        let default_session_key = self.session_key.clone();
        let meta = request.meta.clone();
        let arguments = request.arguments.unwrap_or_default();
        let management_tool = is_session_state_tool
            || is_tool_route_tool
            || is_prompt_override_tool
            || is_resource_override_tool
            || is_openapi_tool
            || is_service_tool
            || is_cache_tool
            || search_helper.is_some();
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
            if is_tool_route_tool {
                return call_tool_route_tool(&store, &tool_name, &scope, instance_id, arguments)
                    .await
                    .map(Into::into);
            }
            if is_prompt_override_tool {
                return call_prompt_route_tool(
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
                if tool_name.starts_with("mcpstore_resource_template_route_") {
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
                return call_resource_route_tool(
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
            if let Some(helper) = search_helper.as_deref() {
                return self
                    .call_search_helper(helper, meta.as_ref(), arguments)
                    .await
                    .map(Into::into);
            }

            let mut call = self.call_log.begin(
                self.caller.as_deref(),
                GatewayCallKind::Tool,
                &tool_name,
                self.instance_id,
                "",
                frame_call_id.as_deref(),
            );
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
            let route = binding.pick_route().clone();
            call.record.instance_id = Some(route.instance_id);
            call.record.service_name = route.service_name.clone();
            call.record.upstream_name = Some(route.tool_name.clone());
            call.record.namespace_applied = route.namespace_applied;
            call.record.name_mapped = route.name_mapped;
            call.record.routing_ms = Some(call.started.elapsed().as_millis() as u64);
            call.publish();
            call.set_phase(super::GatewayCallPhase::CallingUpstream);
            let mut execution_options = McpExecutionOptions::default()
                .with_call_trace(Arc::clone(&call.trace));
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
                            route.instance_id,
                            &route.tool_name,
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
            let error = if success {
                None
            } else {
                match &response {
                    Ok(CallToolResponse::Complete(result)) => result
                        .content
                        .iter()
                        .find_map(|item| item.as_text().map(|text| text.text.to_string())),
                    Err(error) => Some(error.to_string()),
                    _ => None,
                }
            };
            call.finish(success, error);
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
