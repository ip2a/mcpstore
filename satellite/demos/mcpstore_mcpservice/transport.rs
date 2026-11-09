use super::catalog::{catalog_name_counts, stable_namespace};
use super::tools::{read_required_instance_id, read_required_object, read_required_string};
use super::*;

pub async fn run(args: McpServerOptions) -> Result<(), BoxErr> {
    let store = Arc::new(MCPStore::setup_with_options(args.to_store_options())?);
    store.load_from_source().await?;

    let server = McpStoreServer::from_store(
        Arc::clone(&store),
        args.scope.clone(),
        args.instance_id,
        args.session_key.clone(),
        args.expose_session_state_tools,
        args.expose_tool_transform_tools,
        args.expose_openapi_tools,
        args.expose_service_tools,
        args.expose_cache_tools,
        args.expose_event_tools,
    )
    .await?;
    match args.transport {
        McpServerTransport::Stdio => {
            let running = serve_server(server, stdio()).await?;
            running.waiting().await?;
            Ok(())
        }
        McpServerTransport::StreamableHttp => run_streamable_http(server, &args).await,
    }
}

pub(super) async fn connect_target_instances(
    store: &MCPStore,
    scope: &ScopeRef,
    target_instance_id: Option<InstanceId>,
) -> Result<(), BoxErr> {
    let instances = store.list_scope_instances(scope).await?;
    let instance_ids = if let Some(instance_id) = target_instance_id {
        if !instances
            .iter()
            .any(|instance| instance.instance_id == instance_id)
        {
            return Err(
                format!("Instance {instance_id} is not declared in scope {scope:?}").into(),
            );
        }
        vec![instance_id]
    } else {
        instances
            .into_iter()
            .map(|instance| instance.instance_id)
            .collect()
    };

    for instance_id in instance_ids {
        store.connect_service(instance_id).await?;
    }
    Ok(())
}

pub(super) async fn build_tool_bindings(
    store: &MCPStore,
    scope: &ScopeRef,
    target_instance_id: Option<InstanceId>,
    session_key: Option<&str>,
) -> Result<HashMap<String, ToolBinding>, BoxErr> {
    let tool_payloads = if let Some(session_key) = session_key {
        serde_json::to_value(store.list_tools_in_session(session_key).await?)
            .and_then(serde_json::from_value::<Vec<Value>>)
            .map_err(|error| format!("session tool metadata serialization failed: {error}"))?
    } else if let Some(instance_id) = target_instance_id {
        store
            .list_tools_for_instance_with_filter(
                instance_id,
                crate::agent::tool_visibility::ToolVisibilityFilter::Available,
            )
            .await?
    } else {
        store.list_tools_scoped(scope).await?
    };

    let names = catalog_name_counts(&tool_payloads, "name")?;

    let mut bindings = HashMap::with_capacity(tool_payloads.len());
    for payload in tool_payloads {
        let original_name = read_required_string(&payload, "name")?;
        let canonical_tool_name = read_required_string(&payload, "tool_name")?;
        let instance_id = read_required_instance_id(&payload, "instance_id")?;
        let service_name = read_required_string(&payload, "service_name")?;
        let exposed_name = if names.get(&original_name).copied().unwrap_or_default() > 1 {
            format!(
                "{}__{}",
                stable_namespace(&service_name, instance_id),
                original_name
            )
        } else {
            original_name
        };
        let description = payload
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);
        let schema = read_required_object(&payload, "input_schema")?;

        let tool = Tool::new_with_raw(
            exposed_name.clone(),
            description.clone().map(Into::into),
            Arc::new(schema),
        );
        if bindings
            .insert(
                exposed_name.clone(),
                ToolBinding {
                    tool,
                    instance_id,
                    tool_name: canonical_tool_name,
                },
            )
            .is_some()
        {
            return Err(format!(
                "工具名称投影冲突，无法构建 Rust MCP server: scope={scope:?} tool={exposed_name}"
            )
            .into());
        }
    }

    Ok(bindings)
}

pub(super) async fn run_streamable_http(
    server: McpStoreServer,
    args: &McpServerOptions,
) -> Result<(), BoxErr> {
    let path = normalize_http_path(&args.path);
    let service: StreamableHttpService<McpStoreServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(server.clone()),
            Default::default(),
            StreamableHttpServerConfig::default(),
        );
    let router = axum::Router::new().nest_service(&path, service);
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!("[MCP] Starting streamable-http at http://{addr}{path}");
    axum::serve(listener, router).await?;
    Ok(())
}

pub(super) fn normalize_http_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/mcp".to_string();
    }
    let normalized = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    normalized.trim_end_matches('/').to_string()
}
