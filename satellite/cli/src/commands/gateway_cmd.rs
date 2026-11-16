use std::sync::Arc;

use axum::body::{Body, HttpBody};
use axum::extract::{Query, State};
use axum::http::{header, HeaderName, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use clap::Args;
use futures_util::TryStreamExt;
use mcpstore::client_wire;
use mcpstore::{MCPStore, ScopeRef};

use crate::gateway::{
    report_gateway_boot, streamable_http_router, GatewayBoot, GatewayBootPhase,
    normalize_http_path, GatewayBootReporter, GatewayCallLog, GatewayCatalog, GatewayOptions,
    GatewayServer,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower::ServiceExt as _;

use crate::BoxErr;

/// `mcpstore gateway`: a fault-isolated HTTP proxy. The daemon owns the MCP
/// store, mounts, catalogs and every upstream connection; this process only
/// applies the public guard chain and streams requests to the daemon.
#[derive(Args, Debug)]
pub struct GatewayArgs {
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long)]
    pub port: u16,
    #[arg(long, default_value = "/mcp")]
    pub path: String,
    #[arg(long)]
    pub upstream_url: String,
}

#[derive(Clone)]
struct ProxyState {
    client: reqwest::Client,
    upstream: reqwest::Url,
    token: Arc<String>,
}

pub async fn run(args: GatewayArgs) -> Result<(), BoxErr> {
    let token = std::env::var("MCPSTORE_GATEWAY_TOKEN")
        .map_err(|_| err("MCPSTORE_GATEWAY_TOKEN is required"))?;
    let upstream = reqwest::Url::parse(&args.upstream_url)?;
    if upstream.scheme() != "http"
        || !upstream.username().is_empty()
        || upstream.password().is_some()
        || upstream.path() != "/"
        || upstream.query().is_some()
        || upstream.fragment().is_some()
    {
        return Err(err("--upstream-url must be a bare http origin"));
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()?;
    let addr = format!("{}:{}", args.host, args.port);
    report_gateway_boot(GatewayBoot::new(GatewayBootPhase::Binding));
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(error) => {
            report_gateway_boot(GatewayBoot::failed(format!(
                "Failed to bind gateway listener on {addr}: {error}"
            )));
            return Err(error.into());
        }
    };
    eprintln!("[GATEWAY] proxy listening on http://{addr} (upstream {upstream})");
    let state = ProxyState {
        client,
        upstream,
        token: Arc::new(token),
    };
    let limiter = Arc::new(crate::gateway::guards::RateLimiter::new());
    let generic_path = args.path.clone();
    let app = axum::Router::new()
        .fallback(axum::routing::any(proxy_request))
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            crate::gateway::guards::guard_chain(
                generic_path.clone(),
                Arc::clone(&limiter),
                request,
                next,
            )
        }));
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn proxy_request(State(state): State<ProxyState>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let mut upstream = state.upstream.clone();
    let path = path_and_query.split('?').next().unwrap_or("/");
    upstream.set_path(&format!("/internal/gateway{path}"));
    upstream.set_query(parts.uri.query());
    let mut builder = state
        .client
        .request(parts.method, upstream)
        .bearer_auth(state.token.as_str());
    let connection_headers = connection_header_names(&parts.headers);
    for (name, value) in &parts.headers {
        if is_hop_by_hop(name)
            || connection_headers.contains(name)
            || matches!(name.as_str(), "host" | "authorization" | "cookie")
        {
            continue;
        }
        builder = builder.header(name, value);
    }
    if body.size_hint().exact() != Some(0) {
        builder = builder.body(reqwest::Body::wrap_stream(body.into_data_stream().map_err(
            |error| std::io::Error::new(std::io::ErrorKind::Other, error.to_string()),
        )));
    }
    match builder.send().await {
        Ok(response) => proxy_response(response),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            format!("daemon gateway backend unavailable: {error}"),
        )
            .into_response(),
    }
}

fn proxy_response(response: reqwest::Response) -> Response {
    let status = response.status();
    let headers = response.headers().clone();
    let body = Body::from_stream(
        response
            .bytes_stream()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error.to_string())),
    );
    let mut converted = Response::new(body);
    *converted.status_mut() = status;
    let connection_headers = connection_header_names(&headers);
    for (name, value) in &headers {
        if !is_hop_by_hop(name) && !connection_headers.contains(name) {
            converted.headers_mut().append(name, value.clone());
        }
    }
    converted
}

fn connection_header_names(
    headers: &axum::http::HeaderMap,
) -> std::collections::HashSet<HeaderName> {
    headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|name| HeaderName::from_bytes(name.trim().as_bytes()).ok())
        .collect()
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "content-length"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn err(message: &str) -> BoxErr {
    Box::new(std::io::Error::new(std::io::ErrorKind::Other, message))
}

fn wired_ledger_path(store: &MCPStore) -> std::path::PathBuf {
    let app_config_path = store.config_manager().app_config_path();
    app_config_path
        .parent()
        .map(|dir| dir.join("gateway-wired.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("gateway-wired.json"))
}

/// Daemon-owned MCP backend. Its lifetime is independent of proxy children,
/// so stopping or restarting a gateway never rebuilds mounts or connections.
pub(crate) struct GatewayRuntime {
    options: GatewayOptions,
    router: axum::Router,
    reconcile: tokio::task::JoinHandle<()>,
}

impl GatewayRuntime {
    pub(crate) async fn build(
        store: Arc<MCPStore>,
        options: GatewayOptions,
        report: Option<GatewayBootReporter>,
    ) -> Result<Arc<Self>, BoxErr> {
        let call_log = GatewayCallLog::new();
        let wired_path = wired_ledger_path(&store);
        if let Some(report) = &report {
            report(GatewayBoot::new(GatewayBootPhase::Mounting));
        }
        let mount_set =
            build_mounts(&store, &options, Arc::clone(&call_log), &wired_path).await?;
        let generic_path = normalize_http_path(&options.path);
        let mounts = Arc::new(std::sync::RwLock::new(mount_set.routers));
        let projections = Arc::new(std::sync::RwLock::new(Vec::new()));
        publish_projections(&mounts, &projections);
        let calls = axum::Router::new()
            .route("/calls", axum::routing::get(calls_route))
            .with_state(call_log.clone());
        let projection = axum::Router::new()
            .route("/projection", axum::routing::get(projection_route))
            .with_state(Arc::clone(&projections));
        let health = axum::Router::new()
            .route("/health", axum::routing::get(health_route))
            .with_state(HealthState {
                mounts: Arc::clone(&mounts),
                projections: Arc::clone(&projections),
            });
        // Wired mounts and the generic `/mcp` mount all live in `mounts` and
        // are reached through `mount_dispatch` (longest-prefix). Nesting the
        // generic router on this outer tree would steal `/mcp/<client>` because
        // axum nest(`/mcp`) is a prefix match.
        let router = axum::Router::new()
            .merge(calls)
            .merge(projection)
            .merge(health)
            .fallback(axum::routing::any(mount_dispatch).with_state(Arc::clone(&mounts)));
        let reconcile = tokio::spawn(reconcile_loop(
            store,
            options.clone(),
            call_log,
            wired_path,
            mounts,
            projections,
            generic_path,
        ));
        Ok(Arc::new(Self {
            options,
            router,
            reconcile,
        }))
    }

    pub(crate) fn matches(&self, options: &GatewayOptions) -> bool {
        &self.options == options
    }

    pub(crate) async fn handle(&self, request: Request<Body>) -> Response {
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("gateway backend router is infallible")
    }
}

impl Drop for GatewayRuntime {
    fn drop(&mut self) {
        self.reconcile.abort();
    }
}

/// What a mount must match to stay as it is: the ledger's derived scope and
/// exposure. A diverging fingerprint rebuilds the mount ( hubKey).
#[derive(Clone, PartialEq)]
struct MountFingerprint {
    scope: ScopeRef,
    expose: mcpstore::client_wire::ExposeConfig,
}

/// One wired mount as currently served: its dispatch router plus the
/// fingerprint, projection handle, and catalog handle reconciliation needs.
struct LiveMount {
    fingerprint: MountFingerprint,
    router: axum::Router,
    projection: MountProjection,
    catalog: Arc<GatewayCatalog>,
}

#[derive(Default)]
struct MountSet {
    routers: std::collections::HashMap<String, LiveMount>,
}

async fn build_mount(
    store: &Arc<MCPStore>,
    options: &GatewayOptions,
    shared_log: &Arc<GatewayCallLog>,
    entry: &client_wire::WiredEntry,
) -> Result<(String, LiveMount), BoxErr> {
    let scope = entry.effective_scope().map_err(|error| {
        err(&format!(
            "failed to resolve scope for {}: {error}",
            entry.client_id
        ))
    })?;
    let expose = entry.effective_expose();
    let wired_options = GatewayOptions {
        scope: scope.clone(),
        expose_session_state_tools: expose.session_state,
        expose_tool_override_tools: expose.tool,
        expose_prompt_override_tools: expose.prompt,
        expose_resource_override_tools: expose.resource,
        expose_openapi_tools: expose.openapi,
        expose_service_tools: expose.service,
        expose_kv_tools: expose.kv,
        expose_search_tools: expose.search,
        ..options.clone()
    };
    let path = format!("/mcp/{}", entry.client_id);
    let caller = Some(entry.client_id.clone());
    let server = build_server(
        Arc::clone(store),
        wired_options,
        caller.clone(),
        Arc::clone(shared_log),
    )
    .await?;
    let projection = MountProjection {
        caller,
        issues: server.projection_handle(),
    };
    let catalog = server.catalog_handle();
    let router = streamable_http_router(server, &path);
    Ok((
        path,
        LiveMount {
            fingerprint: MountFingerprint { scope, expose },
            router,
            projection,
            catalog,
        },
    ))
}

/// Initial mount construction: fail loud on an unreadable ledger (there is no
/// previous runtime to preserve), skip-with-warning on
/// a mount that individually fails to build so one bad client doesn't take
/// down the rest of the gateway (projection resilience, M2).
///
/// The generic `/mcp` mount is registered in the same map as wired
/// `/mcp/<client>` mounts so dispatch can longest-prefix pick the client
/// mount instead of letting an axum nest(`/mcp`) steal those paths.
async fn build_mounts(
    store: &Arc<MCPStore>,
    options: &GatewayOptions,
    shared_log: Arc<GatewayCallLog>,
    wired_path: &std::path::Path,
) -> Result<MountSet, BoxErr> {
    let mut set = MountSet::default();
    let wired_state = client_wire::load_wired_state(wired_path)?;
    for entry in &wired_state.entries {
        match build_mount(store, options, &shared_log, entry).await {
            Ok((path, mount)) => {
                set.routers.insert(path, mount);
            }
            Err(error) => {
                eprintln!("[GATEWAY] skipping mount for {}: {error}", entry.client_id);
            }
        }
    }

    // Generic mount from the CLI options; its exposure is fixed for the
    // process lifetime, so reconciliation never removes it.
    let generic_path = normalize_http_path(&options.path);
    let generic_server =
        build_server(Arc::clone(store), options.clone(), None, shared_log).await?;
    let generic_projection = MountProjection {
        caller: None,
        issues: generic_server.projection_handle(),
    };
    let generic_catalog = generic_server.catalog_handle();
    let generic_router = streamable_http_router(generic_server, &generic_path);
    set.routers.insert(
        generic_path,
        LiveMount {
            fingerprint: MountFingerprint {
                scope: options.scope.clone(),
                expose: mcpstore::client_wire::ExposeConfig {
                    session_state: options.expose_session_state_tools,
                    tool: options.expose_tool_override_tools,
                    prompt: options.expose_prompt_override_tools,
                    resource: options.expose_resource_override_tools,
                    openapi: options.expose_openapi_tools,
                    service: options.expose_service_tools,
                    kv: options.expose_kv_tools,
                    search: options.expose_search_tools,
                },
            },
            router: generic_router,
            projection: generic_projection,
            catalog: generic_catalog,
        },
    );

    Ok(set)
}

/// Rebuild the shared projection list from every live mount (wired + generic).
fn publish_projections(
    mounts: &Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
    projections: &Arc<std::sync::RwLock<ProjectionMounts>>,
) {
    let all = mounts
        .read()
        .map(|map| {
            map.values()
                .map(|mount| mount.projection.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Ok(mut guard) = projections.write() {
        *guard = all;
    }
}

/// One gateway mount's live projection report, for the /projection route the
/// daemon's /gateway/projection forwards to.
#[derive(Clone)]
struct MountProjection {
    caller: Option<String>,
    issues: std::sync::Arc<std::sync::Mutex<Vec<crate::gateway::ProjectionIssue>>>,
}

type ProjectionMounts = Vec<MountProjection>;

async fn projection_route(
    State(projections): State<Arc<std::sync::RwLock<ProjectionMounts>>>,
) -> Response {
    let mounts: Vec<Value> = projections
        .read()
        .map(|mounts| {
            mounts
                .iter()
                .map(|mount| {
                    let issues = mount
                        .issues
                        .lock()
                        .map(|guard| guard.iter().cloned().collect::<Vec<_>>())
                        .unwrap_or_default();
                    json!({ "caller": mount.caller, "issues": issues })
                })
                .collect()
        })
        .unwrap_or_default();
    axum::Json(json!({
        "success": true,
        "data": { "running": true, "mounts": mounts },
    }))
    .into_response()
}

/// What the gateway health route reads: the live mount registry and the
/// projection list (served-mount count).
#[derive(Clone)]
struct HealthState {
    mounts: Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
    projections: Arc<std::sync::RwLock<ProjectionMounts>>,
}

/// Gateway liveness plus the two counters the R4 stress trajectory correlates
/// against shell-side RSS: how many mounts are served and how many catalog
/// rebuilds have completed since mount builds.
async fn health_route(State(health): State<HealthState>) -> Response {
    let mount_count = health
        .projections
        .read()
        .map(|mounts| mounts.len())
        .unwrap_or(0);
    let rebuilds: u64 = health
        .mounts
        .read()
        .map(|mounts| {
            mounts
                .values()
                .map(|mount| mount.catalog.rebuild_count())
                .sum()
        })
        .unwrap_or(0);
    axum::Json(json!({
        "success": true,
        "data": { "running": true, "mounts": mount_count, "catalog_rebuilds": rebuilds },
    }))
    .into_response()
}

/// Pick the mount whose path is an exact or boundary-aware prefix of `path`,
/// preferring the longest match (`/mcp/claude` over `/mcp`).
fn select_mount<'a>(
    path: &str,
    mounts: &'a std::collections::HashMap<String, LiveMount>,
) -> Option<&'a LiveMount> {
    mounts.get(path).or_else(|| {
        mounts
            .iter()
            .filter(|(mount_path, _)| mount_path_matches(path, mount_path))
            .max_by_key(|(mount_path, _)| mount_path.len())
            .map(|(_, mount)| mount)
    })
}

fn mount_path_matches(request_path: &str, mount_path: &str) -> bool {
    request_path == mount_path
        || request_path
            .strip_prefix(mount_path)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Dispatch a request onto its mount's router (wired `/mcp/<id>` or generic
/// `/mcp`). The registry changes under the reconciliation loop, so the map is
/// read per request.
async fn mount_dispatch(
    State(mounts): State<Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>>,
    request: axum::extract::Request,
) -> Response {
    let path = request.uri().path().to_string();
    let router = mounts
        .read()
        .ok()
        .and_then(|map| select_mount(&path, &map).map(|mount| mount.router.clone()));
    match router {
        Some(router) => router
            .oneshot(request)
            .await
            .expect("router service is infallible"),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

/// Outcome of reading the wired ledger on a reconciliation tick. Missing is
/// distinct from empty: with live mounts running, a vanished ledger is an
/// unreadable state (missing ≠ empty) and must not unmount anything.
enum LedgerRead {
    State(client_wire::WiredState),
    Missing,
    Unreadable(String),
}

fn read_wired_ledger(path: &std::path::Path) -> LedgerRead {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_or_else(
            |error| LedgerRead::Unreadable(error.to_string()),
            LedgerRead::State,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => LedgerRead::Missing,
        Err(error) => LedgerRead::Unreadable(error.to_string()),
    }
}

const RECONCILE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

/// Re-read the wired ledger and apply the diff to the live mounts. Projection
/// invalidation already rides the daemon's local event bus; no cross-process
/// scopes replay is needed.
/// Fail-safe per  hubPrune — a ledger that can't be read (parse
/// failure or vanished file) means nothing is killed; the warning repeats
/// only when the ledger state changes, not every 5s.
async fn reconcile_loop(
    store: Arc<MCPStore>,
    options: GatewayOptions,
    call_log: Arc<GatewayCallLog>,
    wired_path: std::path::PathBuf,
    mounts: Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
    projections: Arc<std::sync::RwLock<ProjectionMounts>>,
    generic_path: String,
) {
    let mut ticker = tokio::time::interval(RECONCILE_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // the first tick fires immediately; the mounts were just built
    ticker.tick().await;
    let mut quiet = false;
    loop {
        ticker.tick().await;
        match read_wired_ledger(&wired_path) {
            LedgerRead::State(state) => {
                quiet = false;
                reconcile_once(
                    &store,
                    &options,
                    &call_log,
                    &state,
                    &mounts,
                    &projections,
                    &generic_path,
                )
                .await;
            }
            unreadable => {
                let reason = match unreadable {
                    LedgerRead::Unreadable(error) => format!("unreadable: {error}"),
                    _ => "missing while mounts are live".to_string(),
                };
                if !quiet {
                    eprintln!("[GATEWAY] wired ledger {reason}; keeping all mounts unchanged");
                }
                quiet = true;
            }
        }
    }
}

async fn reconcile_once(
    store: &Arc<MCPStore>,
    options: &GatewayOptions,
    call_log: &Arc<GatewayCallLog>,
    state: &client_wire::WiredState,
    mounts: &Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
    projections: &Arc<std::sync::RwLock<ProjectionMounts>>,
    generic_path: &str,
) {
    let (adds, rebuilds, removes, warnings) = plan_mount_changes(mounts, state, generic_path);
    for warning in &warnings {
        eprintln!("[GATEWAY] {warning}");
    }
    let mut changed = !removes.is_empty();
    for path in removes {
        if let Ok(mut map) = mounts.write() {
            // Stateless mounts spawn nothing of their own — the store owns
            // the upstream instances — so dropping the entry leaves no
            // orphan process behind.
            map.remove(&path);
            eprintln!("[GATEWAY] unmounted {path}");
        }
    }
    for entry in adds {
        match build_mount(store, options, call_log, &entry).await {
            Ok((path, mount)) => {
                eprintln!("[GATEWAY] mounted {path}");
                if let Ok(mut map) = mounts.write() {
                    map.insert(path, mount);
                }
                changed = true;
            }
            Err(error) => {
                eprintln!(
                    "[GATEWAY] skipping new mount for {}: {error}",
                    entry.client_id
                );
            }
        }
    }
    for entry in rebuilds {
        match build_mount(store, options, call_log, &entry).await {
            Ok((path, mount)) => {
                eprintln!("[GATEWAY] rebuilt mount {path}");
                if let Ok(mut map) = mounts.write() {
                    map.insert(path, mount);
                }
                changed = true;
            }
            Err(error) => {
                // fail-safe per mount: a mount that fails to rebuild keeps
                // serving its previous router
                eprintln!(
                    "[GATEWAY] keeping existing mount for {}: {error}",
                    entry.client_id
                );
            }
        }
    }
    if changed {
        publish_projections(mounts, projections);
    }
}

/// Pure diff between the running mounts and the ledger: what to add, what
/// to rebuild (fingerprint drift), what to remove, and per-entry warnings
/// for entries whose scope can't be resolved (fail-safe: keep what runs).
/// Ordering note: entries are `Clone` so adds and rebuilds carry their own
/// copy and `rebuilds.contains(entry)` works by value.
type MountPlan = (
    Vec<client_wire::WiredEntry>,
    Vec<client_wire::WiredEntry>,
    Vec<String>,
    Vec<String>,
);

fn plan_mount_changes(
    mounts: &Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
    state: &client_wire::WiredState,
    generic_path: &str,
) -> MountPlan {
    let mut adds = Vec::new();
    let mut rebuilds = Vec::new();
    let mut removes = Vec::new();
    let mut warnings = Vec::new();
    let running = match mounts.read() {
        Ok(map) => map
            .iter()
            .map(|(path, mount)| (path.clone(), mount.fingerprint.clone()))
            .collect::<std::collections::HashMap<String, MountFingerprint>>(),
        Err(_) => {
            warnings.push("mount registry poisoned; reconciliation skipped".to_string());
            return (adds, rebuilds, removes, warnings);
        }
    };
    let mut ledger_paths = Vec::new();
    for entry in &state.entries {
        let path = format!("/mcp/{}", entry.client_id);
        ledger_paths.push(path.clone());
        let fingerprint = entry.effective_scope().ok().map(|scope| MountFingerprint {
            scope,
            expose: entry.effective_expose(),
        });
        match (running.get(&path), fingerprint) {
            (None, Some(_fingerprint)) => adds.push(entry.clone()),
            (Some(existing), Some(fingerprint)) if existing != &fingerprint => {
                rebuilds.push(entry.clone())
            }
            (Some(_), None) => warnings.push(format!(
                "scope for {} no longer resolvable; keeping its current mount",
                entry.client_id
            )),
            _ => {}
        }
    }
    for path in running.keys() {
        // The generic `/mcp` mount is not ledger-backed; never prune it.
        if path == generic_path {
            continue;
        }
        if !ledger_paths.contains(path) {
            removes.push(path.clone());
        }
    }
    (adds, rebuilds, removes, warnings)
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

#[cfg(test)]
mod tests {
    use super::*;
    use mcpstore::client_wire::{ExposeConfig, WiredState};

    #[tokio::test]
    async fn unavailable_daemon_is_a_loud_bad_gateway() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let upstream = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let state = ProxyState {
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_millis(100))
                .build()
                .unwrap(),
            upstream: reqwest::Url::parse(&upstream).unwrap(),
            token: Arc::new("test-token".to_string()),
        };
        let response = proxy_request(
            State(state),
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    fn entry(client_id: &str, expose_tool: bool) -> client_wire::WiredEntry {
        client_wire::WiredEntry {
            client_id: client_id.into(),
            path: "/tmp/unused-agent-config.json".into(),
            name: client_id.into(),
            url: format!("http://127.0.0.1:1830/mcp/{client_id}"),
            applied_at_unix_ms: 0,
            scope: Some(mcpstore::ScopeRef::Store),
            expose: Some(ExposeConfig {
                tool: expose_tool,
                ..Default::default()
            }),
        }
    }

    fn running_with(
        entries: &[client_wire::WiredEntry],
    ) -> std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>> {
        let store = Arc::new(
            mcpstore::MCPStore::setup_with_options(mcpstore::StoreOptions {
                store: Some(mcpstore::JsonStoreConfig::memory()),
                ..mcpstore::StoreOptions::default()
            })
            .expect("memory store for mount planning"),
        );
        let map = entries
            .iter()
            .map(|entry| {
                let scope = entry.effective_scope().expect("explicit scope resolves");
                let expose = entry.effective_expose();
                (
                    format!("/mcp/{}", entry.client_id),
                    LiveMount {
                        fingerprint: MountFingerprint { scope, expose },
                        router: axum::Router::new(),
                        projection: MountProjection {
                            caller: Some(entry.client_id.clone()),
                            issues: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                        },
                        catalog: GatewayCatalog::blank(
                            Arc::clone(&store),
                            mcpstore::ScopeRef::Store,
                            None,
                            None,
                        ),
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        Arc::new(std::sync::RwLock::new(map))
    }

    fn plan_of(
        mounts: &std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
        entries: Vec<client_wire::WiredEntry>,
    ) -> MountPlan {
        plan_mount_changes(mounts, &WiredState { entries }, "/mcp")
    }

    #[test]
    fn mount_dispatch_prefers_client_path_over_generic_prefix() {
        assert!(mount_path_matches("/mcp/claude", "/mcp/claude"));
        assert!(mount_path_matches("/mcp/claude", "/mcp"));
        assert!(mount_path_matches("/mcp/", "/mcp"));
        assert!(!mount_path_matches("/mcpfoo", "/mcp"));

        let mounts = running_with(&[entry("claude", false)]);
        // Inject the generic `/mcp` mount alongside the wired client.
        {
            let store = Arc::new(
                mcpstore::MCPStore::setup_with_options(mcpstore::StoreOptions {
                    store: Some(mcpstore::JsonStoreConfig::memory()),
                    ..mcpstore::StoreOptions::default()
                })
                .expect("memory store"),
            );
            let mut map = mounts.write().unwrap();
            map.insert(
                "/mcp".into(),
                LiveMount {
                    fingerprint: MountFingerprint {
                        scope: mcpstore::ScopeRef::Store,
                        expose: ExposeConfig::default(),
                    },
                    router: axum::Router::new(),
                    projection: MountProjection {
                        caller: None,
                        issues: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                    },
                    catalog: GatewayCatalog::blank(
                        Arc::clone(&store),
                        mcpstore::ScopeRef::Store,
                        None,
                        None,
                    ),
                },
            );
            let selected = select_mount("/mcp/claude", &map).expect("mount");
            assert_eq!(selected.projection.caller.as_deref(), Some("claude"));
            let selected = select_mount("/mcp", &map).expect("generic");
            assert_eq!(selected.projection.caller, None);
        }

        // Empty ledger must not prune the generic mount.
        let (adds, rebuilds, removes, warnings) = plan_of(&mounts, vec![]);
        assert!(adds.is_empty() && rebuilds.is_empty() && warnings.is_empty());
        assert_eq!(removes, ["/mcp/claude"]);
    }

    #[test]
    fn plan_adds_new_and_removes_gone() {
        let mounts = running_with(&[entry("agent-a", false)]);
        let (adds, rebuilds, removes, warnings) = plan_of(
            &mounts,
            vec![entry("agent-a", false), entry("agent-b", false)],
        );
        assert_eq!(
            adds.iter()
                .map(|e| e.client_id.as_str())
                .collect::<Vec<_>>(),
            ["agent-b"]
        );
        assert!(rebuilds.is_empty());
        assert_eq!(removes, Vec::<String>::new());
        assert!(warnings.is_empty());

        let (adds, rebuilds, removes, warnings) = plan_of(&mounts, vec![]);
        assert!(adds.is_empty() && rebuilds.is_empty() && warnings.is_empty());
        assert_eq!(removes, ["/mcp/agent-a"]);
    }

    #[test]
    fn plan_rebuilds_on_fingerprint_drift() {
        let mounts = running_with(&[entry("agent-a", false)]);
        // same client, exposure flipped → rebuild, not keep
        let (adds, rebuilds, removes, warnings) = plan_of(&mounts, vec![entry("agent-a", true)]);
        assert!(adds.is_empty() && removes.is_empty() && warnings.is_empty());
        assert_eq!(rebuilds.len(), 1);
        assert_eq!(rebuilds[0].client_id, "agent-a");
    }

    #[test]
    fn plan_keeps_mount_when_scope_unresolvable() {
        // the running mount was built with an explicit Store scope, but the
        // ledger entry now carries no scope and its plugin is not installed:
        // reconciliation keeps the mount alive and warns instead of
        // unmounting it (fail-safe: unreadable intent ≠ absence)
        let mounts = running_with(&[entry("ghost-plugin", false)]);
        let mut orphan = entry("ghost-plugin", false);
        orphan.scope = None;
        let (adds, rebuilds, removes, warnings) = plan_of(&mounts, vec![orphan]);
        assert!(adds.is_empty() && rebuilds.is_empty());
        assert_eq!(removes, Vec::<String>::new());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn ledger_read_states() {
        let dir =
            std::env::temp_dir().join(format!("gateway-reconcile-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("gateway-wired.json");

        // missing ledger reads as Missing
        assert!(matches!(read_wired_ledger(&path), LedgerRead::Missing));

        // valid ledger reads as State
        std::fs::write(&path, br#"{"entries":[]}"#).expect("write ledger");
        assert!(matches!(read_wired_ledger(&path), LedgerRead::State(_)));

        // corrupt ledger reads as Unreadable
        std::fs::write(&path, b"{not json").expect("write corrupt ledger");
        assert!(matches!(
            read_wired_ledger(&path),
            LedgerRead::Unreadable(_)
        ));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
