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
    #[arg(long, default_value_t = false)]
    pub expose_session_state_tools: bool,
    #[arg(long, default_value_t = false)]
    pub expose_tool_override_tools: bool,
    #[arg(long, default_value_t = false)]
    pub expose_prompt_override_tools: bool,
    #[arg(long, default_value_t = false)]
    pub expose_resource_override_tools: bool,
    #[arg(long, default_value_t = false)]
    pub expose_openapi_tools: bool,
    #[arg(long, default_value_t = false)]
    pub expose_service_tools: bool,
    #[arg(long, default_value_t = false)]
    pub expose_cache_tools: bool,
    #[arg(long, default_value_t = false)]
    pub expose_search_tools: bool,
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
        expose_session_state_tools: args.expose_session_state_tools,
        expose_tool_override_tools: args.expose_tool_override_tools,
        expose_prompt_override_tools: args.expose_prompt_override_tools,
        expose_resource_override_tools: args.expose_resource_override_tools,
        expose_openapi_tools: args.expose_openapi_tools,
        expose_service_tools: args.expose_service_tools,
        expose_cache_tools: args.expose_cache_tools,
        expose_search_tools: args.expose_search_tools,
    };

    let call_log = GatewayCallLog::new();
    let (router, projection_mounts) = build_router(
        &store,
        &options,
        Arc::clone(&call_log),
        store.config_manager().app_config_path().to_path_buf(),
    )
    .await?;
    let calls = axum::Router::new()
        .route("/calls", axum::routing::get(calls_route))
        .with_state(call_log);
    let projection = axum::Router::new()
        .route("/projection", axum::routing::get(projection_route))
        .with_state(projection_mounts);
    let addr = format!("{}:{}", options.host, options.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    // Guard chain (magpie port): peer loopback, Host rebinding, Origin,
    // POST-only, per-mount rate limit, 4 MiB body ceiling. See
    // gateway::guards for the incident history behind each guard.
    let limiter = std::sync::Arc::new(crate::gateway::guards::RateLimiter::new());
    let generic_path = options.path.clone();
    let app = router
        .merge(calls)
        .merge(projection)
        .layer(axum::middleware::from_fn(move |request, next| {
            crate::gateway::guards::guard_chain(
                generic_path.clone(),
                std::sync::Arc::clone(&limiter),
                request,
                next,
            )
        }));
    eprintln!(
        "[GATEWAY] listening on http://{addr} (generic mount {})",
        options.path
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
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
) -> Result<(axum::Router, ProjectionMounts), BoxErr> {
    let mut router = axum::Router::new();
    let mut mounts: ProjectionMounts = Vec::new();

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
            caller.clone(),
            Arc::clone(&shared_log),
        )
        .await?;
        mounts.push(MountProjection {
            caller,
            issues: server.projection_handle(),
        });
        router = router.merge(streamable_http_router(server, &path));
    }

    // Generic mount from the CLI options.
    let generic_server = build_server(Arc::clone(store), options.clone(), None, shared_log).await?;
    mounts.push(MountProjection {
        caller: generic_server.caller_name(),
        issues: generic_server.projection_handle(),
    });
    router = router.merge(streamable_http_router(generic_server, &options.path));

    Ok((router, mounts))
}

/// One gateway mount's live projection report, for the /projection route the
/// daemon's /gateway/projection forwards to.
#[derive(Clone)]
struct MountProjection {
    caller: Option<String>,
    issues: std::sync::Arc<std::sync::Mutex<Vec<crate::gateway::ProjectionIssue>>>,
}

type ProjectionMounts = Vec<MountProjection>;

async fn projection_route(State(mounts): State<ProjectionMounts>) -> Response {
    let mounts: Vec<Value> = mounts
        .iter()
        .map(|mount| {
            let issues = mount
                .issues
                .lock()
                .map(|guard| guard.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            json!({ "caller": mount.caller, "issues": issues })
        })
        .collect();
    axum::Json(json!({
        "success": true,
        "data": { "running": true, "mounts": mounts },
    }))
    .into_response()
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
