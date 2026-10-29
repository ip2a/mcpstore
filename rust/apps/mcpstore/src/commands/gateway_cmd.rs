use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use clap::Args;
use mcpstore::client_wire;
use mcpstore::{InstanceId, MCPStore, ScopeRef};

use crate::gateway::{streamable_http_router, GatewayCallLog, GatewayOptions, GatewayServer};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::store_args::StoreSourceArgs;
use crate::BoxErr;

/// `mcpstore gateway`: standalone gateway process, launched on demand.
/// Serves the per-client wired mounts (/mcp/<client-id> from gateway-wired.json)
/// plus one generic mount whose exposure comes from the flags below, and a
/// /calls management route the daemon's /gateway/requests forwards to.
#[derive(Args, Debug)]
pub struct GatewayArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long, help = "URL path of the generic mount (default /mcp)")]
    pub path: Option<String>,
    #[arg(
        long,
        help = "Projection scope of the generic mount: store|agent (default store)"
    )]
    pub scope: Option<String>,
    #[arg(long, requires = "scope")]
    pub agent: Option<String>,
    #[arg(long)]
    pub session_key: Option<String>,
    #[arg(long)]
    pub instance_id: Option<InstanceId>,
    #[arg(default_value_t = false)]
    pub expose_session_state: bool,
    #[arg(default_value_t = false)]
    pub expose_tool: bool,
    #[arg(default_value_t = false)]
    pub expose_prompt: bool,
    #[arg(default_value_t = false)]
    pub expose_resource: bool,
    #[arg(default_value_t = false)]
    pub expose_openapi: bool,
    #[arg(default_value_t = false)]
    pub expose_service: bool,
    #[arg(default_value_t = false)]
    pub expose_cache: bool,
    #[arg(default_value_t = false)]
    pub expose_search: bool,
}

pub async fn run(args: GatewayArgs) -> Result<(), BoxErr> {
    let handle = crate::store_args::load_kernel(&args.store).await?;
    let store = Arc::clone(handle.store());
    let app_config = store.config_manager().load_app_config_or_default()?;
    let port = args.port.unwrap_or(app_config.gateway.port);

    let scope = match args.scope.as_deref().unwrap_or("store") {
        "store" => ScopeRef::Store,
        "agent" => ScopeRef::Agent {
            agent_id: args
                .agent
                .clone()
                .ok_or_else(|| err("--scope agent requires --agent"))?,
        },
        other => return Err(err(&format!("unsupported scope {other:?}"))),
    };

    let options = GatewayOptions {
        config_path: Some(store.config_manager().mcp_path().display().to_string()),
        store: None,
        namespace: None,
        scope,
        instance_id: args.instance_id,
        session_key: args.session_key.clone(),
        host: args.host.clone(),
        port,
        path: args.path.clone().unwrap_or_else(|| "/mcp".into()),
        expose_session_state_tools: args.expose_session_state,
        expose_tool_override_tools: args.expose_tool,
        expose_prompt_override_tools: args.expose_prompt,
        expose_resource_override_tools: args.expose_resource,
        expose_openapi_tools: args.expose_openapi,
        expose_service_tools: args.expose_service,
        expose_cache_tools: args.expose_cache,
        expose_search_tools: args.expose_search,
    };

    let call_log = GatewayCallLog::new();
    let router = build_router(
        &store,
        &options,
        Arc::clone(&call_log),
        store.config_manager().app_config_path().to_path_buf(),
    )
    .await?;
    let calls = axum::Router::new()
        .route("/calls", axum::routing::get(calls_route))
        .with_state(call_log);
    let addr = format!("{}:{}", options.host, options.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!(
        "[GATEWAY] listening on http://{addr} (generic mount {})",
        options.path
    );
    axum::serve(listener, router.merge(calls)).await?;
    Ok(())
}

fn err(message: &str) -> BoxErr {
    Box::new(std::io::Error::new(std::io::ErrorKind::Other, message))
}

async fn build_router(
    store: &Arc<MCPStore>,
    options: &GatewayOptions,
    shared_log: Arc<GatewayCallLog>,
    app_config_path: std::path::PathBuf,
) -> Result<axum::Router, BoxErr> {
    let mut router = axum::Router::new();

    // Per-client mounts: derive GatewayOptions from the wired entry.
    let wired_path = app_config_path
        .parent()
        .map(|dir| dir.join("gateway-wired.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("gateway-wired.json"));
    let wired_state = client_wire::load_wired_state(&wired_path)?;
    for entry in &wired_state.entries {
        let scope = entry.effective_scope().map_err(|error| {
            err(&format!(
                "failed to resolve scope for {}: {error}",
                entry.client_id
            ))
        })?;
        let expose = entry.effective_expose();
        let wired_options = GatewayOptions {
            scope,
            expose_session_state_tools: expose.session_state,
            expose_tool_override_tools: expose.tool,
            expose_prompt_override_tools: expose.prompt,
            expose_resource_override_tools: expose.resource,
            expose_openapi_tools: expose.openapi,
            expose_service_tools: expose.service,
            expose_cache_tools: expose.cache,
            expose_search_tools: expose.search,
            ..options.clone()
        };
        let path = format!("/mcp/{}", entry.client_id);
        let caller = Some(entry.client_id.clone());
        let server = build_server(
            Arc::clone(store),
            wired_options,
            caller,
            Arc::clone(&shared_log),
        )
        .await?;
        router = router.merge(streamable_http_router(server, &path));
    }

    // Generic mount from the CLI options.
    let generic_server = build_server(Arc::clone(store), options.clone(), None, shared_log).await?;
    router = router.merge(streamable_http_router(generic_server, &options.path));

    Ok(router)
}

async fn build_server(
    store: Arc<MCPStore>,
    options: GatewayOptions,
    caller: Option<String>,
    call_log: Arc<GatewayCallLog>,
) -> Result<GatewayServer, BoxErr> {
    GatewayServer::from_store(store, &options, caller, call_log)
        .await
        .map_err(|error| err(&format!("failed to build gateway server: {error}")))
}

#[derive(Deserialize)]
struct CallsQuery {
    after: Option<u64>,
    wait_ms: Option<u64>,
}

/// GET /calls — the gateway's routed-call stream (the daemon's /gateway/requests
/// forwards here; same `after` + `wait_ms` long-poll semantics).
async fn calls_route(
    State(call_log): State<Arc<GatewayCallLog>>,
    Query(query): Query<CallsQuery>,
) -> Response {
    let after = query.after.unwrap_or(0);
    if let Some(wait_ms) = query.wait_ms {
        if wait_ms > 0 && call_log.latest_seq() <= after {
            let mut rx = call_log.subscribe();
            let wait = std::time::Duration::from_millis(wait_ms.min(25_000));
            let _ = tokio::time::timeout(wait, rx.changed()).await;
        }
    }
    let requests: Vec<Value> = call_log
        .snapshot_after(after)
        .iter()
        .map(|record| serde_json::to_value(record).unwrap_or(Value::Null))
        .collect();
    axum::Json(json!({
        "success": true,
        "data": {
            "running": true,
            "requests": requests,
            "latest_seq": call_log.latest_seq(),
        },
    }))
    .into_response()
}
