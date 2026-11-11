use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use clap::Args;
use mcpstore::client_wire;
use mcpstore::{InstanceId, MCPStore, ScopeRef};

use crate::gateway::{
    streamable_http_router, GatewayCallLog, GatewayCatalog, GatewayOptions, GatewayServer,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower::ServiceExt as _;

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

/// Panel role of the gateway process. The gateway is a serving surface, never
/// a second control plane: on a shared store (shm/redis/valkey) the daemon owns
/// the `@control` state column and the self-healing supervisor, so a second
/// control panel would fight it CAS-for-CAS on the same column (the
/// auth-transition panics at startup were exactly that) and double-connect
/// every service. Shared stores therefore attach as a data panel with their
/// own identity — suffixed so it can never collide with a real data-panel
/// daemon defaulting to the same hostname; the suffix keeps their
/// `{instance}@{panel}` state rows apart. A private memory store is the
/// gateway's own universe and keeps the control panel. Explicit flags win —
/// and `--control-panel` on a shared store is refused outright.
fn gateway_panel_role(mut store_args: StoreSourceArgs) -> Result<StoreSourceArgs, BoxErr> {
    // Probe the same config load_kernel will use: a --config run must judge
    // its own store, not the default user-data-dir config.
    let backend = store_args.store.clone().unwrap_or_else(|| {
        let manager = match store_args.config_path.as_deref() {
            Some(path) => mcpstore::ConfigManager::with_path(path),
            None => mcpstore::ConfigManager::new(),
        };
        manager
            .load_app_config_or_default()
            .map(|config| config.kv.store)
            .unwrap_or_else(|_| "memory".to_string())
    });
    let shared = matches!(backend.as_str(), "shm" | "redis" | "valkey");
    if store_args.control_panel {
        if shared {
            return Err("the gateway cannot run as a control panel on a shared store \
                (shm/redis/valkey): the daemon owns the @control column and supervision; \
                attach with --data-panel instead"
                .into());
        }
        return Ok(store_args);
    }
    if !store_args.data_panel && shared {
        store_args.data_panel = true;
        store_args.panel_id = Some(format!(
            "{}-gateway",
            mcpstore::PanelRole::default_panel_id()
        ));
    }
    Ok(store_args)
}

pub async fn run(args: GatewayArgs) -> Result<(), BoxErr> {
    let store_args = gateway_panel_role(args.store)?;
    let handle = crate::store_args::load_kernel(&store_args).await?;
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
    let wired_path = wired_ledger_path(&store);
    // Subscribe before building mounts: Latest subscriptions can't see writes
    // made before subscribing, so the projection bridge must exist first or a
    // change landing during mount build is lost.
    let replay_scopes = match store.subscribe_tool_projection_changes().await {
        Ok((subscription, generation)) => {
            // In shm mode overrides and policies propagate only through the
            // shared KV + ChangeFeed; the scopes.toml writeback replay would
            // resurrect stale file state behind the feed's back.
            let shared = store.store_config().await.store == "shm";
            crate::gateway::bridge::spawn_projection_bridge(
                Arc::clone(&store),
                subscription,
                generation,
            );
            !shared
        }
        Err(error) => {
            // Memory store (or any backend without a feed): keep the 1s
            // scopes replay; cross-process projection simply doesn't apply.
            eprintln!(
                "[GATEWAY] no shared change feed ({error}); keeping scopes replay"
            );
            true
        }
    };
    let (generic_router, mount_set) = build_mounts(
        &store,
        &options,
        Arc::clone(&call_log),
        &wired_path,
    )
    .await?;
    // Wired mounts live behind a shared registry so the reconciliation loop
    // can add/rebuild/remove them while the listener keeps running.
    let mounts = Arc::new(std::sync::RwLock::new(mount_set.routers));
    let generic_projection = mount_set.generic_projection;
    // Cloned for the health route; the original moves into reconcile_loop.
    let generic_catalog = mount_set.generic_catalog.clone();
    let projections = Arc::new(std::sync::RwLock::new(Vec::new()));
    publish_projections(&mounts, &generic_projection, &projections);
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
            generic_catalog,
        });
    let addr = format!("{}:{}", options.host, options.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    // Guard chain (magpie port): peer loopback, Host rebinding, Origin,
    // POST-only, per-mount rate limit, 4 MiB body ceiling. See
    // gateway::guards for the incident history behind each guard.
    let limiter = std::sync::Arc::new(crate::gateway::guards::RateLimiter::new());
    let generic_path = options.path.clone();
    let app = generic_router
        .merge(calls)
        .merge(projection)
        .merge(health)
        .fallback(axum::routing::any(mount_dispatch).with_state(Arc::clone(&mounts)))
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
    // Reconciliation: the wired ledger (CLI, panel, and manual edits are all
    // writers) is re-read every 5s so a running gateway follows it without a
    // restart (magpie hubPrune). An unreadable ledger leaves every mount
    // exactly as it is — missing ≠ empty, corrupt ≠ absent. The same tick
    // watches scopes.toml: daemon-side override writes land there via
    // writeback, and the gateway's own KV only sees them through a replay.
    tokio::spawn(reconcile_loop(
        Arc::clone(&store),
        options.clone(),
        call_log,
        wired_path,
        Arc::clone(&mounts),
        Arc::clone(&projections),
        generic_projection,
        mount_set.generic_catalog,
        replay_scopes,
    ));
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

fn wired_ledger_path(store: &MCPStore) -> std::path::PathBuf {
    let app_config_path = store.config_manager().app_config_path();
    app_config_path
        .parent()
        .map(|dir| dir.join("gateway-wired.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("gateway-wired.json"))
}

/// What a mount must match to stay as it is: the ledger's derived scope and
/// exposure. A diverging fingerprint rebuilds the mount (magpie hubKey).
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
    generic_projection: Option<MountProjection>,
    generic_catalog: Option<Arc<GatewayCatalog>>,
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
        expose_cache_tools: expose.kv,
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

/// Initial mount construction: fail loud on an unreadable ledger (the child
/// process is starting — there is nothing to preserve), skip-with-warning on
/// a mount that individually fails to build so one bad client doesn't take
/// down the rest of the gateway (projection resilience, M2).
async fn build_mounts(
    store: &Arc<MCPStore>,
    options: &GatewayOptions,
    shared_log: Arc<GatewayCallLog>,
    wired_path: &std::path::Path,
) -> Result<(axum::Router, MountSet), BoxErr> {
    let mut set = MountSet::default();
    let wired_state = client_wire::load_wired_state(wired_path)?;
    for entry in &wired_state.entries {
        match build_mount(store, options, &shared_log, entry).await {
            Ok((path, mount)) => {
                set.routers.insert(path, mount);
            }
            Err(error) => {
                eprintln!(
                    "[GATEWAY] skipping mount for {}: {error}",
                    entry.client_id
                );
            }
        }
    }

    // Generic mount from the CLI options; its exposure is fixed for the
    // process lifetime, so reconciliation never touches it.
    let generic_server = build_server(Arc::clone(store), options.clone(), None, shared_log).await?;
    let generic_projection = MountProjection {
        caller: generic_server.caller_name(),
        issues: generic_server.projection_handle(),
    };
    let generic_catalog = generic_server.catalog_handle();
    let generic_router = streamable_http_router(generic_server, &options.path);
    set.generic_projection = Some(generic_projection);
    set.generic_catalog = Some(generic_catalog);

    Ok((generic_router, set))
}

/// Rebuild the shared projection list from the live mounts plus the generic
/// mount's handle.
fn publish_projections(
    mounts: &Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
    generic_projection: &Option<MountProjection>,
    projections: &Arc<std::sync::RwLock<ProjectionMounts>>,
) {
    let wired = mounts
        .read()
        .map(|map| {
            map.values()
                .map(|mount| mount.projection.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut all = wired;
    all.extend(generic_projection.clone());
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

/// What the gateway health route reads: the live mount registry (per-mount
/// catalogs), the projection list (served-mount count), and the generic
/// mount's catalog.
#[derive(Clone)]
struct HealthState {
    mounts: Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
    projections: Arc<std::sync::RwLock<ProjectionMounts>>,
    generic_catalog: Option<Arc<crate::gateway::GatewayCatalog>>,
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
    let mut rebuilds: u64 = health
        .mounts
        .read()
        .map(|mounts| mounts.values().map(|mount| mount.catalog.rebuild_count()).sum())
        .unwrap_or(0);
    if let Some(catalog) = &health.generic_catalog {
        rebuilds += catalog.rebuild_count();
    }
    axum::Json(json!({
        "success": true,
        "data": { "running": true, "mounts": mount_count, "catalog_rebuilds": rebuilds },
    }))
    .into_response()
}

/// Dispatch a request onto its wired mount's router. The registry changes
/// under the reconciliation loop, so the map is read per request; a Router
/// clone is an Arc clone.
async fn mount_dispatch(
    State(mounts): State<Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>>,
    request: axum::extract::Request,
) -> Response {
    let path = request.uri().path().to_string();
    let router = mounts
        .read()
        .ok()
        .and_then(|map| {
            map.get(&path)
                // nest semantics: the longest mount path that prefixes the request
                .or_else(|| {
                    map.iter()
                        .filter(|(mount_path, _)| path.starts_with(mount_path.as_str()))
                        .max_by_key(|(mount_path, _)| mount_path.len())
                        .map(|(_, mount)| mount)
                })
                .map(|mount| mount.router.clone())
        });
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
/// scopes.toml is a content-hash check (microseconds) on the daemon→gateway
/// data channel; polled faster than the ledger tick so a daemon override
/// write lands in the gateway within ~2s worst case, rebuild included.
const SCOPES_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// Cheap fingerprint of the shared scopes.toml: content hash, or a stable
/// marker while the file is absent. A change means daemon-side override
/// writes landed via writeback — the gateway's own KV (memory backend) only
/// sees them through a replay.
fn scopes_fingerprint(store: &MCPStore) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match store.scopes_config_path() {
        Ok(path) => match std::fs::read(&path) {
            Ok(bytes) => bytes.hash(&mut hasher),
            Err(_) => path.display().to_string().hash(&mut hasher),
        },
        Err(_) => "unresolvable".hash(&mut hasher),
    }
    hasher.finish()
}

/// Replay scopes.toml into the gateway's KV and refresh every live mount's
/// catalog (event + call-miss are the other two invalidation channels).
async fn refresh_scopes_projection(
    store: &Arc<MCPStore>,
    mounts: &Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
    generic_catalog: Option<&Arc<GatewayCatalog>>,
) {
    if let Err(error) = store.load_scopes_config().await {
        eprintln!("[GATEWAY] scopes.toml replay failed: {error}");
    }
    let catalogs = mounts
        .read()
        .map(|map| {
            map.values()
                .map(|mount| Arc::clone(&mount.catalog))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for catalog in catalogs.into_iter().chain(generic_catalog.cloned()) {
        catalog.invalidate();
        let catalog = Arc::clone(&catalog);
        tokio::spawn(async move { let _ = catalog.ensure().await; });
    }
}

/// Every tick: refresh the scopes.toml projection when the file changed, then
/// re-read the ledger and apply the diff to the live mounts.
/// Fail-safe per magpie hubPrune — a ledger that can't be read (parse
/// failure or vanished file) means nothing is killed; the warning repeats
/// only when the ledger state changes, not every 5s.
async fn reconcile_loop(
    store: Arc<MCPStore>,
    options: GatewayOptions,
    call_log: Arc<GatewayCallLog>,
    wired_path: std::path::PathBuf,
    mounts: Arc<std::sync::RwLock<std::collections::HashMap<String, LiveMount>>>,
    projections: Arc<std::sync::RwLock<ProjectionMounts>>,
    generic_projection: Option<MountProjection>,
    generic_catalog: Option<Arc<GatewayCatalog>>,
    replay_scopes: bool,
) {
    let mut ticker = tokio::time::interval(SCOPES_POLL_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // the first tick fires immediately; the mounts were just built
    ticker.tick().await;
    // SHM mode: the projection bridge carries every scopes-relevant change;
    // the file replay would resurrect stale writeback state behind the feed.
    let mut last_scopes = replay_scopes.then(|| scopes_fingerprint(&store));
    let mut quiet = false;
    let mut last_ledger = tokio::time::Instant::now();
    loop {
        ticker.tick().await;
        if replay_scopes {
            let scopes = scopes_fingerprint(&store);
            if last_scopes != Some(scopes) {
                last_scopes = Some(scopes);
                refresh_scopes_projection(&store, &mounts, generic_catalog.as_ref()).await;
            }
        }
        if last_ledger.elapsed() < RECONCILE_INTERVAL {
            continue;
        }
        last_ledger = tokio::time::Instant::now();
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
                    &generic_projection,
                )
                .await;
            }
            unreadable => {
                let reason = match unreadable {
                    LedgerRead::Unreadable(error) => format!("unreadable: {error}"),
                    _ => "missing while mounts are live".to_string(),
                };
                if !quiet {
                    eprintln!(
                        "[GATEWAY] wired ledger {reason}; keeping all mounts unchanged"
                    );
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
    generic_projection: &Option<MountProjection>,
) {
    let (adds, rebuilds, removes, warnings) = plan_mount_changes(&mounts, state);
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
        publish_projections(mounts, generic_projection, projections);
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
    use crate::store_args::SourceArg;
    use mcpstore::client_wire::{ExposeConfig, WiredState};

    fn store_args(store: Option<&str>, control_panel: bool, data_panel: bool) -> StoreSourceArgs {
        StoreSourceArgs {
            control_panel,
            data_panel,
            config_path: None,
            source: SourceArg::Local,
            store: store.map(str::to_string),
            store_config: None,
            namespace: None,
            panel_id: None,
        }
    }

    #[test]
    fn shared_stores_derive_a_data_panel_with_its_own_identity() {
        for backend in ["shm", "redis", "valkey"] {
            let resolved = gateway_panel_role(store_args(Some(backend), false, false)).unwrap();
            assert!(!resolved.control_panel);
            assert!(resolved.data_panel);
            let mcpstore::PanelRole::DataPanel { panel_id } = resolved.panel_role() else {
                panic!("{backend} gateway must resolve to a data panel");
            };
            assert_ne!(
                panel_id,
                mcpstore::state::CONTROL_NODE_ID,
                "the gateway must never write the @control column"
            );
        }
    }

    #[test]
    fn control_panel_on_a_shared_store_is_refused() {
        for backend in ["shm", "redis", "valkey"] {
            let error = gateway_panel_role(store_args(Some(backend), true, false)).unwrap_err();
            assert!(error.to_string().contains("--data-panel"));
        }
    }

    #[test]
    fn memory_keeps_the_control_panel_universe() {
        let resolved = gateway_panel_role(store_args(Some("memory"), false, false)).unwrap();
        assert!(matches!(
            resolved.panel_role(),
            mcpstore::PanelRole::ControlPanel
        ));
    }

    #[test]
    fn explicit_data_panel_flag_is_kept() {
        let mut args = store_args(Some("shm"), false, true);
        args.panel_id = Some("edge-01".into());
        let resolved = gateway_panel_role(args).unwrap();
        assert_eq!(resolved.panel_id.as_deref(), Some("edge-01"));
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

    fn running_with(entries: &[client_wire::WiredEntry]) -> std::sync::Arc<
        std::sync::RwLock<std::collections::HashMap<String, LiveMount>>,
    > {
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
        mounts: &std::sync::Arc<
            std::sync::RwLock<std::collections::HashMap<String, LiveMount>>,
        >,
        entries: Vec<client_wire::WiredEntry>,
    ) -> MountPlan {
        plan_mount_changes(
            mounts,
            &WiredState {
                entries,
            },
        )
    }

    #[test]
    fn plan_adds_new_and_removes_gone() {
        let mounts = running_with(&[entry("agent-a", false)]);
        let (adds, rebuilds, removes, warnings) =
            plan_of(&mounts, vec![entry("agent-a", false), entry("agent-b", false)]);
        assert_eq!(adds.iter().map(|e| e.client_id.as_str()).collect::<Vec<_>>(), ["agent-b"]);
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
        let (adds, rebuilds, removes, warnings) =
            plan_of(&mounts, vec![entry("agent-a", true)]);
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
        let dir = std::env::temp_dir().join(format!("gateway-reconcile-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("gateway-wired.json");

        // missing ledger reads as Missing
        assert!(matches!(read_wired_ledger(&path), LedgerRead::Missing));

        // valid ledger reads as State
        std::fs::write(&path, br#"{"entries":[]}"#).expect("write ledger");
        assert!(matches!(read_wired_ledger(&path), LedgerRead::State(_)));

        // corrupt ledger reads as Unreadable
        std::fs::write(&path, b"{not json").expect("write corrupt ledger");
        assert!(matches!(read_wired_ledger(&path), LedgerRead::Unreadable(_)));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
