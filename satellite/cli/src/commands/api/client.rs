use std::{process::Stdio, time::Duration};

use super::*;
use crate::gateway::{GatewayBootPhase, GATEWAY_BOOT_PREFIX};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mcpstore::ScopeRef;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Deserialize)]
pub(super) struct ClientConfigImportRequest {
    client: String,
    path: String,
    names: Vec<String>,
    // Absent = the client's bound agent scope; "store" lands in the global store scope.
    scope: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
pub(super) struct GatewayLaunchQuery {
    scope: Option<String>,
    agent_id: Option<String>,
    instance_id: Option<InstanceId>,
    session_key: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    path: Option<String>,
    expose_session_state: Option<bool>,
    expose_tool: Option<bool>,
    expose_prompt: Option<bool>,
    expose_resource: Option<bool>,
    expose_openapi: Option<bool>,
    expose_service: Option<bool>,
    expose_kv: Option<bool>,
    expose_search: Option<bool>,
}

pub(super) async fn gateway_descriptor(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayLaunchQuery>,
) -> ApiResult {
    let options = gateway_options(&state, &query)?;
    Ok(success(
        "Gateway launch descriptor generated successfully",
        serde_json::to_value(options.launch_descriptor())
            .map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn gateway_status(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayLaunchQuery>,
) -> ApiResult {
    let options = gateway_options(&state, &query)?;
    let descriptor = options.launch_descriptor();
    let (running, pid, summary, boot) = gateway_status_inner(&state)?;
    let starting = boot.as_ref().is_some_and(|boot| boot.phase.is_starting());
    Ok(success(
        "Gateway status fetched successfully",
        json!({
            "running": running,
            "starting": starting,
            "pid": pid,
            "host": options.host,
            "port": options.port,
            "path": options.path,
            "url": descriptor.url,
            "summary": summary,
            "boot": boot,
        }),
    ))
}

pub(super) async fn gateway_start(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayLaunchQuery>,
) -> ApiResult {
    // Running check, backend reuse, and spawn are serialized against stop and
    // concurrent starts. Readiness is supervised in the background.
    let _launch = state.gateway_launch.lock().await;
    let options = gateway_options(&state, &query)?;
    let descriptor = options.launch_descriptor();
    let exe = std::env::current_exe().map_err(|error| {
        ApiError::internal(format!("Failed to locate the mcpstore executable: {error}"))
    })?;
    {
        let mut process = state
            .gateway
            .lock()
            .map_err(|_| ApiError::invalid_request("Gateway status unavailable"))?;
        if let Some(existing) = process.as_mut() {
            match existing.child.try_wait() {
                Ok(None) => {
                    let boot = current_gateway_boot(&state)?;
                    let running = boot
                        .as_ref()
                        .is_some_and(|boot| boot.phase == GatewayBootPhase::Ready);
                    return Ok(success(
                        if running {
                            "Gateway is already running"
                        } else {
                            "Gateway is already starting"
                        },
                        gateway_payload(existing, running, boot),
                    ));
                }
                Ok(Some(_)) => {}
                Err(error) => {
                    return Err(ApiError::internal(format!(
                        "Failed to inspect gateway child process: {error}"
                    )))
                }
            }
        }
    }

    let upstream_url = match gateway_upstream_url(&state) {
        Ok(url) => url,
        Err(error) => {
            set_gateway_boot(&state, GatewayBoot::failed(error.message().to_string()))?;
            return Err(error);
        }
    };
    if let Err(error) = ensure_gateway_backend(&state, &options).await {
        set_gateway_boot(&state, GatewayBoot::failed(error.message().to_string()))?;
        return Err(error);
    }

    set_gateway_boot(&state, GatewayBoot::new(GatewayBootPhase::Spawning))?;
    let mut command = tokio::process::Command::new(exe);
    command
        .arg("gateway")
        .args(["--host", &options.host])
        .args(["--port", &options.port.to_string()])
        .args(["--path", &options.path])
        .args(["--upstream-url", &upstream_url])
        .env("MCPSTORE_GATEWAY_TOKEN", state.gateway_token.as_str());

    let mut child = match command.stderr(Stdio::piped()).kill_on_drop(true).spawn() {
        Ok(child) => child,
        Err(error) => {
            let api_error = ApiError::invalid_request(format!(
                "Failed to start gateway child process: {error}"
            ));
            set_gateway_boot(&state, GatewayBoot::failed(api_error.message().to_string()))?;
            return Err(api_error);
        }
    };
    let pid = child.id().unwrap_or_default();
    let stderr = child.stderr.take();
    let probe_host = match options.host.as_str() {
        "0.0.0.0" => "127.0.0.1",
        "::" => "::1",
        value => value,
    };
    let calls_url = format!("http://{probe_host}:{}/calls", options.port);
    let projection_url = format!("http://{probe_host}:{}/projection", options.port);
    state
        .gateway
        .lock()
        .map_err(|_| ApiError::invalid_request("Gateway status unavailable"))?
        .replace(GatewayProcess {
            pid,
            calls_url,
            projection_url,
            descriptor: descriptor.clone(),
            summary: gateway_summary(&options),
            child,
        });

    if let Some(stderr) = stderr {
        spawn_gateway_stderr_reader(Arc::clone(&state), pid, stderr);
    }
    tokio::spawn(wait_for_gateway_ready(
        Arc::clone(&state),
        pid,
        options.host.clone(),
        options.port,
    ));

    let boot = current_gateway_boot(&state)?;
    let running = boot
        .as_ref()
        .is_some_and(|boot| boot.phase == GatewayBootPhase::Ready);
    let starting = boot.as_ref().is_some_and(|boot| boot.phase.is_starting());
    Ok(success(
        "Gateway start accepted",
        json!({
            "running": running,
            "starting": starting,
            "pid": pid,
            "url": descriptor.url,
            "summary": gateway_summary(&options),
            "boot": boot,
        }),
    ))
}

pub(super) async fn gateway_stop(State(state): State<Arc<ApiState>>) -> ApiResult {
    // Stop only the proxy. The daemon backend and all upstream connections
    // deliberately remain alive for the next launch.
    let _launch = state.gateway_launch.lock().await;
    let process = {
        let mut gateway = state
            .gateway
            .lock()
            .map_err(|_| ApiError::invalid_request("Gateway status unavailable"))?;
        gateway.take()
    };
    clear_gateway_boot(&state)?;
    let Some(mut process) = process else {
        return Ok(success(
            "Gateway is not running",
            json!({ "running": false }),
        ));
    };
    let pid = process.pid;
    if process.child.try_wait().ok().flatten().is_none() {
        if let Err(error) = process.child.kill().await {
            return Err(ApiError::invalid_request(format!(
                "Failed to stop gateway: {error}"
            )));
        }
    }
    let _ = process.child.wait().await;
    Ok(success(
        "Gateway stopped",
        json!({ "running": false, "pid": pid }),
    ))
}

/// Build the backend once per daemon configuration. Proxy restarts reuse the
/// same runtime, including its mounts and call log; a changed projection
/// rebuilds only routers against the daemon's existing connection pool.
pub(super) async fn ensure_gateway_backend(
    state: &ApiState,
    options: &GatewayOptions,
) -> Result<Arc<crate::commands::gateway_cmd::GatewayRuntime>, ApiError> {
    if let Some(runtime) = state
        .gateway_backend
        .lock()
        .map_err(|_| ApiError::internal("Gateway backend unavailable"))?
        .as_ref()
        .filter(|runtime| runtime.matches(options))
        .cloned()
    {
        return Ok(runtime);
    }
    let reporter: crate::gateway::GatewayBootReporter = {
        let state = Arc::new(state.clone());
        Arc::new(move |boot| {
            if let Ok(mut current) = state.gateway_boot.lock() {
                *current = Some(boot);
            }
        })
    };
    let runtime = crate::commands::gateway_cmd::GatewayRuntime::build(
        Arc::clone(&state.store),
        options.clone(),
        Some(reporter),
    )
    .await
    .map_err(|error| ApiError::internal(format!("Failed to build gateway backend: {error}")))?;
    *state
        .gateway_backend
        .lock()
        .map_err(|_| ApiError::internal("Gateway backend unavailable"))? =
        Some(Arc::clone(&runtime));
    Ok(runtime)
}

/// The child reaches only this daemon's authenticated internal MCP face.
fn gateway_upstream_url(state: &ApiState) -> Result<String, ApiError> {
    let config = state
        .store
        .config_manager()
        .load_app_config_or_default()
        .map_err(|error| {
            ApiError::invalid_request(format!("Failed to load app configuration: {error}"))
        })?;
    if !config.server.core_enabled {
        return Err(ApiError::invalid_request(
            "The daemon core HTTP face is disabled; enable it before starting the gateway",
        ));
    }
    let mut bind = crate::daemon::listeners::resolve_bind(&config.server.host, config.server.port)
        .map_err(ApiError::from_store)?;
    if bind.ip().is_unspecified() {
        bind.set_ip(if bind.is_ipv4() {
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        } else {
            std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        });
    }
    Ok(format!("http://{bind}"))
}

/// Authenticated, unprefixed MCP backend nested below /internal/gateway.
pub(super) fn gateway_internal_router(state: Arc<ApiState>) -> axum::Router {
    axum::Router::new().nest(
        "/internal/gateway",
        axum::Router::new()
            .fallback(axum::routing::any(gateway_internal))
            .with_state(Arc::clone(&state))
            .layer(axum::middleware::from_fn_with_state(
                state,
                authorize_gateway_internal,
            )),
    )
}

async fn authorize_gateway_internal(
    State(state): State<Arc<ApiState>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    use subtle::ConstantTimeEq;
    let expected = state.gateway_token.as_bytes();
    let provided = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default()
        .as_bytes();
    if provided.len() != expected.len() || !bool::from(provided.ct_eq(expected)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}

async fn gateway_internal(
    State(state): State<Arc<ApiState>>,
    request: axum::extract::Request,
) -> Response {
    let runtime = state
        .gateway_backend
        .lock()
        .ok()
        .and_then(|backend| backend.clone());
    match runtime {
        Some(runtime) => runtime.handle(request).await,
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "gateway backend is not running; start it through the daemon",
        )
            .into_response(),
    }
}

fn gateway_options(
    state: &ApiState,
    query: &GatewayLaunchQuery,
) -> Result<GatewayOptions, ApiError> {
    let app_config = state
        .store
        .config_manager()
        .load_app_config_or_default()
        .map_err(|error| {
            ApiError::invalid_request(format!("Failed to load app configuration: {error}"))
        })?;
    let scope = match query.scope.as_deref().unwrap_or("store") {
        "store" => ScopeRef::Store,
        "agent" => ScopeRef::Agent {
            agent_id: query
                .agent_id
                .clone()
                .ok_or_else(|| ApiError::missing_parameter("agent_id"))?,
        },
        value => {
            return Err(ApiError::invalid_parameter(
                format!("Unsupported scope: {value}"),
                Some("scope"),
            ))
        }
    };
    Ok(GatewayOptions {
        scope,
        instance_id: query.instance_id,
        session_key: query.session_key.clone(),
        host: query.host.clone().unwrap_or_else(|| "127.0.0.1".into()),
        port: query.port.unwrap_or(app_config.gateway.port),
        path: query.path.clone().unwrap_or_else(|| "/mcp".into()),
        expose_session_state_tools: query.expose_session_state.unwrap_or(false),
        expose_tool_override_tools: query.expose_tool.unwrap_or(false),
        expose_prompt_override_tools: query.expose_prompt.unwrap_or(false),
        expose_resource_override_tools: query.expose_resource.unwrap_or(false),
        expose_openapi_tools: query.expose_openapi.unwrap_or(false),
        expose_service_tools: query.expose_service.unwrap_or(false),
        expose_kv_tools: query.expose_kv.unwrap_or(false),
        expose_search_tools: query.expose_search.unwrap_or(false),
    })
}

/// Running-config snapshot (scope + expose families) carried by GatewayProcess.
fn gateway_summary(options: &GatewayOptions) -> Value {
    let scope = match &options.scope {
        ScopeRef::Store => "store".to_string(),
        ScopeRef::Agent { agent_id } => format!("agent:{agent_id}"),
    };
    json!({
        "scope": scope,
        "expose": {
            "session_state": options.expose_session_state_tools,
            "tool": options.expose_tool_override_tools,
            "prompt": options.expose_prompt_override_tools,
            "resource": options.expose_resource_override_tools,
            "openapi": options.expose_openapi_tools,
            "service": options.expose_service_tools,
            "kv": options.expose_kv_tools,
            "search": options.expose_search_tools,
        },
    })
}

fn set_gateway_boot(state: &ApiState, boot: GatewayBoot) -> Result<(), ApiError> {
    *state
        .gateway_boot
        .lock()
        .map_err(|_| ApiError::invalid_request("Gateway boot status unavailable"))? = Some(boot);
    Ok(())
}

fn clear_gateway_boot(state: &ApiState) -> Result<(), ApiError> {
    *state
        .gateway_boot
        .lock()
        .map_err(|_| ApiError::invalid_request("Gateway boot status unavailable"))? = None;
    Ok(())
}

fn current_gateway_boot(state: &ApiState) -> Result<Option<GatewayBoot>, ApiError> {
    state
        .gateway_boot
        .lock()
        .map(|boot| boot.clone())
        .map_err(|_| ApiError::invalid_request("Gateway boot status unavailable"))
}

/// Apply a child update only to the launch that produced it. A stopped or
/// replaced child must never overwrite the next launch's progress.
fn update_gateway_boot(state: &ApiState, pid: u32, boot: GatewayBoot) {
    let Ok(gateway) = state.gateway.lock() else {
        return;
    };
    if !gateway.as_ref().is_some_and(|process| process.pid == pid) {
        return;
    }
    if let Ok(mut current) = state.gateway_boot.lock() {
        let terminal = current.as_ref().map(|current| current.phase);
        if (terminal == Some(GatewayBootPhase::Failed) && boot.phase != GatewayBootPhase::Failed)
            || (terminal == Some(GatewayBootPhase::Ready) && boot.phase.is_starting())
        {
            return;
        }
        *current = Some(boot);
    }
}

/// Mark a launch failed and take its child for an async kill outside the locks.
fn fail_gateway(state: &ApiState, pid: u32, error: String) -> Option<GatewayProcess> {
    let mut gateway = state.gateway.lock().ok()?;
    if !gateway.as_ref().is_some_and(|process| process.pid == pid) {
        return None;
    }
    if let Ok(mut current) = state.gateway_boot.lock() {
        if current
            .as_ref()
            .is_none_or(|boot| boot.phase != GatewayBootPhase::Failed)
        {
            *current = Some(GatewayBoot::failed(error));
        }
    }
    gateway.take()
}

fn parse_gateway_boot_line(line: &str) -> Option<Result<GatewayBoot, serde_json::Error>> {
    let payload = line.strip_prefix(GATEWAY_BOOT_PREFIX)?.trim_start();
    Some(serde_json::from_str(payload))
}

fn spawn_gateway_stderr_reader(
    state: Arc<ApiState>,
    pid: u32,
    stderr: tokio::process::ChildStderr,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => match parse_gateway_boot_line(&line) {
                    Some(Ok(boot)) => update_gateway_boot(&state, pid, boot),
                    Some(Err(error)) => {
                        eprintln!("[GATEWAY] invalid boot progress from pid {pid}: {error}")
                    }
                    None => eprintln!("{line}"),
                },
                Ok(None) => break,
                Err(error) => {
                    eprintln!("[GATEWAY] failed to read child stderr from pid {pid}: {error}");
                    break;
                }
            }
        }
    });
}

async fn wait_for_gateway_ready(state: Arc<ApiState>, pid: u32, host: String, port: u16) {
    let probe_host = match host.as_str() {
        "0.0.0.0" => "127.0.0.1",
        "::" => "::1",
        value => value,
    };
    let health_url = format!("http://{probe_host}:{port}/health");
    // The proxy binds before this check. Only its daemon-backed /health route
    // proves that the authenticated internal backend is reachable.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);

    loop {
        let terminal_error = {
            let Ok(mut gateway) = state.gateway.lock() else {
                return;
            };
            let Some(process) = gateway.as_mut().filter(|process| process.pid == pid) else {
                return;
            };
            match process.child.try_wait() {
                Ok(None) => {
                    if state
                        .gateway_boot
                        .lock()
                        .ok()
                        .and_then(|boot| {
                            boot.as_ref()
                                .map(|boot| boot.phase == GatewayBootPhase::Ready)
                        })
                        .unwrap_or(false)
                    {
                        return;
                    }
                    None
                }
                Ok(Some(status)) => Some(format!("Gateway process exited with status {status}")),
                Err(error) => Some(format!("Failed to inspect gateway child process: {error}")),
            }
        };
        if let Some(error) = terminal_error {
            fail_gateway(&state, pid, error);
            return;
        }

        let ready = tokio::time::timeout(
            Duration::from_millis(500),
            state.http_client.get(&health_url).send(),
        )
        .await
        .is_ok_and(|result| result.is_ok_and(|response| response.status().is_success()));
        if ready {
            update_gateway_boot(&state, pid, GatewayBoot::new(GatewayBootPhase::Ready));
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            let process = fail_gateway(
                &state,
                pid,
                format!("Timed out waiting for daemon-backed gateway health at {health_url}"),
            );
            if let Some(mut process) = process {
                let _ = process.child.kill().await;
                let _ = process.child.wait().await;
            }
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[allow(clippy::type_complexity)]
fn gateway_status_inner(
    state: &ApiState,
) -> Result<(bool, Option<u32>, Option<Value>, Option<GatewayBoot>), ApiError> {
    let mut gateway = state
        .gateway
        .lock()
        .map_err(|_| ApiError::invalid_request("Gateway status unavailable"))?;
    let mut boot = state
        .gateway_boot
        .lock()
        .map_err(|_| ApiError::invalid_request("Gateway boot status unavailable"))?;

    let terminal_error = if let Some(process) = gateway.as_mut() {
        match process.child.try_wait() {
            Ok(None) => None,
            Ok(Some(status)) => Some(format!("Gateway process exited with status {status}")),
            Err(error) => Some(format!("Failed to inspect gateway child process: {error}")),
        }
    } else {
        None
    };
    if let Some(error) = terminal_error {
        if boot
            .as_ref()
            .is_none_or(|boot| boot.phase != GatewayBootPhase::Failed)
        {
            *boot = Some(GatewayBoot::failed(error));
        }
        *gateway = None;
    }

    let (pid, summary) = gateway
        .as_ref()
        .map(|process| (Some(process.pid), Some(process.summary.clone())))
        .unwrap_or((None, None));
    let running = pid.is_some()
        && boot
            .as_ref()
            .is_some_and(|boot| boot.phase == GatewayBootPhase::Ready);
    Ok((running, pid, summary, boot.clone()))
}

fn gateway_payload(process: &GatewayProcess, running: bool, boot: Option<GatewayBoot>) -> Value {
    let starting = boot.as_ref().is_some_and(|boot| boot.phase.is_starting());
    json!({
        "running": running,
        "starting": starting,
        "pid": process.pid,
        "url": process.descriptor.url,
        "summary": process.summary,
        "boot": boot,
    })
}

#[derive(Deserialize, Default)]
pub(super) struct GatewayRequestsQuery {
    after: Option<u64>,
    wait_ms: Option<u64>,
}

pub(super) async fn gateway_requests(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GatewayRequestsQuery>,
) -> ApiResult {
    let (running, _, _, _) = gateway_status_inner(&state)?;
    let calls_url = if running {
        state
            .gateway
            .lock()
            .map_err(|_| ApiError::invalid_request("Gateway status unavailable"))?
            .as_ref()
            .map(|process| process.calls_url.clone())
    } else {
        None
    };
    let Some(calls_url) = calls_url else {
        return Ok(success(
            "Gateway is not running",
            json!({ "running": false, "requests": [], "latest_seq": 0 }),
        ));
    };
    // Forward to the child's /calls route; it owns the call log and the same
    // `after` + `wait_ms` long-poll semantics.
    let mut url = reqwest::Url::parse(&calls_url)
        .map_err(|error| ApiError::internal(format!("Invalid gateway management URL: {error}")))?;
    url.query_pairs_mut()
        .append_pair("after", &query.after.unwrap_or(0).to_string());
    if let Some(wait_ms) = query.wait_ms {
        url.query_pairs_mut()
            .append_pair("wait_ms", &wait_ms.to_string());
    }
    let response = state
        .http_client
        .get(url)
        .send()
        .await
        .map_err(|_| ApiError::invalid_request("Gateway process has exited"))?;
    let payload: Value = response
        .json()
        .await
        .map_err(|_| ApiError::invalid_request("Failed to parse gateway response"))?;
    Ok(success(
        "Gateway request records fetched successfully",
        payload["data"].clone(),
    ))
}

/// GET /gateway/projection — per-mount projection issues (down/skipped/
/// degraded tools) forwarded to the child's /projection route.
pub(super) async fn gateway_projection(State(state): State<Arc<ApiState>>) -> ApiResult {
    let (running, _, _, _) = gateway_status_inner(&state)?;
    let projection_url = if running {
        state
            .gateway
            .lock()
            .map_err(|_| ApiError::invalid_request("Gateway status unavailable"))?
            .as_ref()
            .map(|process| process.projection_url.clone())
    } else {
        None
    };
    let Some(projection_url) = projection_url else {
        return Ok(success(
            "Gateway is not running",
            json!({ "running": false, "mounts": [] }),
        ));
    };
    let response = state
        .http_client
        .get(projection_url)
        .send()
        .await
        .map_err(|_| ApiError::invalid_request("Gateway process has exited"))?;
    let payload: Value = response
        .json()
        .await
        .map_err(|_| ApiError::invalid_request("Failed to parse gateway response"))?;
    Ok(success(
        "Gateway projection fetched successfully",
        payload["data"].clone(),
    ))
}

pub(super) fn wired_state_path<T: PanelCtx>(state: &T) -> std::path::PathBuf {
    let app_config_path = state.config_manager().app_config_path();
    app_config_path
        .parent()
        .map(|dir| dir.join("gateway-wired.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("gateway-wired.json"))
}

/// Re-point legacy wired entries at their caller-mounted URLs before the faces start.
pub(crate) fn refresh_wired_urls(state: &ApiState) {
    let Ok(app_config) = state.store.config_manager().load_app_config_or_default() else {
        return;
    };
    let (host, port) = (app_config.gateway.host.clone(), app_config.gateway.port);
    mcpstore::client_wire::refresh_wired_urls(&wired_state_path(state), |client_id| {
        gateway_entry_url(&host, port, client_id)
    });
}

/// Entry URL host for wired clients: the address an agent can actually dial
/// the gateway at ( gatewayOf — generate the entry per where the agent
/// reaches the gateway, not where the gateway happens to bind). A loopback
/// bind serves 127.0.0.1; an all-interfaces bind serves the machine's
/// primary LAN address; an explicit host passes through untouched.
fn gateway_entry_host(gateway_host: &str) -> String {
    match gateway_host {
        "127.0.0.1" | "localhost" | "::1" => "127.0.0.1".to_string(),
        "0.0.0.0" | "::" => primary_lan_address().unwrap_or_else(|| "127.0.0.1".to_string()),
        other => other.to_string(),
    }
}

/// Primary outbound address: UDP connect() only picks the default route in
/// the routing table — no packet is sent (ponytail: first IPv4 route wins;
/// multi-homed hosts with per-agent faces need the P1 phase-2 key work).
fn primary_lan_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

/// The single place wired entry URLs are generated from config.
fn gateway_entry_url(gateway_host: &str, port: u16, client_id: &str) -> String {
    format!(
        "http://{}:{port}/mcp/{client_id}",
        gateway_entry_host(gateway_host)
    )
}

#[derive(Deserialize, Default)]
pub(super) struct GatewayWireQuery {
    client: String,
    path: Option<String>,
    url: Option<String>,
    // Scope override: "store" or "agent"; if "agent" without agent_id, derives from client_id.
    scope: Option<String>,
    agent_id: Option<String>,
    // Exposure overrides (8 flags); if omitted, all default to false.
    expose_session_state: Option<bool>,
    expose_tool: Option<bool>,
    expose_prompt: Option<bool>,
    expose_resource: Option<bool>,
    expose_openapi: Option<bool>,
    expose_service: Option<bool>,
    expose_kv: Option<bool>,
    expose_search: Option<bool>,
    // Replace a diverging existing mcpstore entry (the confirm step of the wire-conflict flow).
    force: Option<bool>,
}

fn default_gateway_url<T: PanelCtx>(state: &T, client_id: &str) -> Result<String, ApiError> {
    let app_config = state
        .config_manager()
        .load_app_config_or_default()
        .map_err(|error| {
            ApiError::invalid_request(format!("Failed to load app configuration: {error}"))
        })?;
    Ok(gateway_entry_url(
        &app_config.gateway.host,
        app_config.gateway.port,
        client_id,
    ))
}

pub(super) async fn gateway_wired<T: PanelCtx>(State(state): State<Arc<T>>) -> ApiResult {
    let wired = mcpstore::client_wire::load_wired_state(&wired_state_path(state.as_ref()))
        .map_err(ApiError::from_store)?;
    // Resolve per-client effective scope/expose so the frontend sees derived defaults,
    // not absent fields (the stored entry omits them when unset).
    let mut entries = Vec::with_capacity(wired.entries.len());
    for entry in &wired.entries {
        let mut value = serde_json::to_value(entry)
            .map_err(|error| ApiError::invalid_request(error.to_string()))?;
        let scope = entry.effective_scope().map_err(ApiError::from_store)?;
        value["scope"] = serde_json::to_value(scope)
            .map_err(|error| ApiError::invalid_request(error.to_string()))?;
        value["expose"] = serde_json::to_value(entry.effective_expose())
            .map_err(|error| ApiError::invalid_request(error.to_string()))?;
        entries.push(value);
    }
    Ok(success(
        "Gateway wiring state fetched successfully",
        serde_json::to_value(entries)
            .map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn gateway_wire<T: PanelCtx>(
    State(state): State<Arc<T>>,
    Query(query): Query<GatewayWireQuery>,
) -> ApiResult {
    let setup = mcpstore::agents::by_id(&query.client).ok_or_else(|| {
        ApiError::invalid_parameter(
            format!(
                "unknown client plugin: {}; installed: {}",
                query.client,
                mcpstore::agents::all()
                    .iter()
                    .map(|setup| setup.id())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Some("client"),
        )
    })?;
    let path = match query.path {
        Some(path) => path,
        None => {
            // No explicit path: use the client's own config file from discovery.
            match mcpstore::agents::detect(setup.as_ref()) {
                Some(detection) if detection.by == mcpstore::agents::DetectedBy::ConfigFile => {
                    detection.path.display().to_string()
                }
                _ => {
                    return Err(ApiError::invalid_request(format!(
                        "{} config file not found",
                        setup.label()
                    )))
                }
            }
        }
    };
    let url = match query.url {
        Some(url) => url,
        None => default_gateway_url(state.as_ref(), setup.id())?,
    };

    // Parse scope: explicit "store"/"agent" or None (will derive from client_id).
    let scope = match query.scope.as_deref() {
        Some("store") => Some(ScopeRef::Store),
        Some("agent") => {
            let agent_id = query.agent_id.as_deref().unwrap_or_else(|| setup.id());
            Some(ScopeRef::Agent {
                agent_id: agent_id.to_string(),
            })
        }
        None => None, // Will derive Agent { agent_id: client_id } in effective_scope()
        Some(other) => {
            return Err(ApiError::invalid_request(format!(
                "Invalid scope '{other}'; must be 'store' or 'agent'"
            )))
        }
    };

    // Build expose config if any flag was set; otherwise None (will default to all false).
    let expose = if query.expose_session_state.is_some()
        || query.expose_tool.is_some()
        || query.expose_prompt.is_some()
        || query.expose_resource.is_some()
        || query.expose_openapi.is_some()
        || query.expose_service.is_some()
        || query.expose_kv.is_some()
        || query.expose_search.is_some()
    {
        Some(mcpstore::client_wire::ExposeConfig {
            session_state: query.expose_session_state.unwrap_or(false),
            tool: query.expose_tool.unwrap_or(false),
            prompt: query.expose_prompt.unwrap_or(false),
            resource: query.expose_resource.unwrap_or(false),
            openapi: query.expose_openapi.unwrap_or(false),
            service: query.expose_service.unwrap_or(false),
            kv: query.expose_kv.unwrap_or(false),
            search: query.expose_search.unwrap_or(false),
        })
    } else {
        None
    };

    // Ensure client-agent binding exists (persists client-agents.json; the agent itself
    // materializes when real services are imported into its scope).
    let agent_registry_path = {
        let app_config_path = state.config_manager().app_config_path();
        app_config_path
            .parent()
            .map(|dir| dir.join("client-agents.json"))
            .unwrap_or_else(|| std::path::PathBuf::from("client-agents.json"))
    };
    let _bound_agent_id =
        mcpstore::client_agent::ensure_client_agent_binding(&agent_registry_path, setup.id())
            .map_err(ApiError::from_store)?;

    let outcome = mcpstore::client_wire::wire_gateway(
        setup.id(),
        std::path::Path::new(&path),
        &url,
        &wired_state_path(state.as_ref()),
        scope,
        expose,
        query.force.unwrap_or(false),
    )
    .map_err(ApiError::from_store)?;
    let payload = serde_json::to_value(outcome)
        .map_err(|error| ApiError::invalid_request(error.to_string()))?;
    let message = if payload["outcome"] == "conflict" {
        "Existing mcpstore entry with different content detected"
    } else {
        "Client wired to gateway"
    };
    Ok(success(message, payload))
}

pub(super) async fn gateway_unwire<T: PanelCtx>(
    State(state): State<Arc<T>>,
    Query(query): Query<GatewayWireQuery>,
) -> ApiResult {
    let entry =
        mcpstore::client_wire::unwire_gateway(&query.client, &wired_state_path(state.as_ref()))
            .map_err(ApiError::from_store)?;
    Ok(success(
        "Client unwired from gateway",
        serde_json::to_value(entry)
            .map_err(|error| ApiError::invalid_request(error.to_string()))?,
    ))
}

pub(super) async fn client_config_import<T: PanelCtx>(
    State(state): State<Arc<T>>,
    headers: axum::http::HeaderMap,
    Json(request): Json<ClientConfigImportRequest>,
) -> ApiResult {
    // The client config is read and parsed on this panel host regardless of the
    // target: a selected daemon receives parsed definitions, never a local path.
    let inspection = inspect_client_config(&request.client, &request.path)
        .map_err(|error| ApiError::invalid_request(error.message().to_string()))?;
    let services = import_selected_services(&inspection, &request.names)
        .map_err(|error| ApiError::invalid_request(error.message().to_string()))?;
    match super::hosts::resolve_selected(state.as_ref(), &headers)? {
        None => match state.daemon() {
            // Embedded daemon: definitions land in-process.
            Some(daemon) => import_into_local(state.as_ref(), daemon, request, services).await,
            // Standalone web panel: the local daemon is just the "local" entry.
            None => {
                let (name, entry) = local_host_entry(state.as_ref())?;
                import_into_selected(state.as_ref(), &name, &entry, &request, services).await
            }
        },
        Some((host_name, entry)) => {
            import_into_selected(state.as_ref(), &host_name, &entry, &request, services).await
        }
    }
}

/// The panel's own daemon: the hosts entry named "local". Missing is a loud
/// error — the panel never falls back to an embedded store.
fn local_host_entry<T: PanelCtx>(
    state: &T,
) -> Result<(String, mcpstore::config::HostEntry), ApiError> {
    super::panel_settings::load_hosts(state.config_manager())
        .entries
        .remove("local")
        .map(|entry| ("local".to_owned(), entry))
        .ok_or_else(|| {
            ApiError::invalid_request(
                "no local daemon: add a 'local' entry to the panel hosts table",
            )
        })
}

/// Local import: definitions land in this daemon's store, scoped to the
/// client's bound agent unless scope=store opts out.
async fn import_into_local<T: PanelCtx>(
    state: &T,
    daemon: &MCPStore,
    request: ClientConfigImportRequest,
    services: Vec<(String, ServerConfig)>,
) -> ApiResult {
    for (name, _) in &services {
        if daemon
            .get_definition_config(name)
            .await
            .map_err(ApiError::from_store)?
            .is_some()
        {
            return Err(ApiError::invalid_request(format!(
                "Service {name} already exists in MCPStore, refusing to overwrite"
            )));
        }
    }
    let summaries = services
        .iter()
        .map(|(name, config)| json!({"name": name, "transport": config.infer_transport()}))
        .collect::<Vec<_>>();
    // Default landing scope: the client's bound agent (created on first import/wire);
    // explicit scope=store opts out. Agent visibility in /agents/list comes from these
    // real services — nothing placeholder is added anywhere.
    let scope = match request.scope.as_deref() {
        Some("store") => ScopeRef::Store,
        Some(other) => {
            return Err(ApiError::invalid_request(format!(
                "Invalid scope '{other}'; must be 'store' or 'agent'"
            )))
        }
        None => {
            let agent_registry_path = {
                let app_config_path = state.config_manager().app_config_path();
                app_config_path
                    .parent()
                    .map(|dir| dir.join("client-agents.json"))
                    .unwrap_or_else(|| std::path::PathBuf::from("client-agents.json"))
            };
            let agent_id = mcpstore::client_agent::ensure_client_agent_binding(
                &agent_registry_path,
                &request.client,
            )
            .map_err(ApiError::from_store)?;
            ScopeRef::Agent { agent_id }
        }
    };
    for (name, mut config) in services {
        if let ScopeRef::Agent { agent_id } = &scope {
            // Stamp the agent ownership into the service config (same shape as the
            // daemon AddService op): scopes.agents[agent_id] on the mcpstore extension.
            let previous = config.mcpstore.take();
            let mut scopes = mcpstore::config::ScopeDeclarations::default();
            scopes.agents.insert(
                agent_id.clone(),
                mcpstore::config::ScopeDescriptor::default(),
            );
            config.mcpstore = Some(mcpstore::config::McpStoreExtension {
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
        daemon
            .add_service(&name, config)
            .await
            .map_err(ApiError::from_store)?;
    }
    Ok(success(
        "Client config services imported successfully",
        json!({"imported": summaries, "scope": scope}),
    ))
}

/// Cross-resource import: parsed definitions are written to the selected
/// daemon through its /services/:name API. Pre-flight rejects any name that
/// already exists there instead of overwriting. The remote batch is not one
/// transaction — every per-service outcome is reported as-is, and already
/// written entries must not be retried as if they had not executed.
/// stdio commands, working dirs and absolute paths from this host keep
/// executing on the selected daemon; nothing about the local path travels.
async fn import_into_selected<T: PanelCtx>(
    state: &T,
    host_name: &str,
    entry: &mcpstore::config::HostEntry,
    request: &ClientConfigImportRequest,
    services: Vec<(String, ServerConfig)>,
) -> ApiResult {
    match request.scope.as_deref() {
        None | Some("store") => {}
        Some(other) => {
            return Err(ApiError::invalid_parameter(
                format!("Invalid scope '{other}'; cross-resource import only supports 'store'"),
                Some("scope"),
            ))
        }
    }
    let client = state.http_client();

    if services.is_empty() {
        return Ok(success(
            "No services selected",
            json!({"imported": [], "failed": [], "scope": "store", "target": host_name}),
        ));
    }

    // Find the reachable candidate with a GET probe (GETs rotate candidates).
    let mut selected_base = None;
    let mut last_error = String::from("no candidate URLs configured");
    for base in &entry.urls {
        let probe = super::hosts::join_url(base, &format!("/services/{}", services[0].0))
            .map_err(|(_, message)| ApiError::invalid_request(message))?;
        match client.get(probe).send().await {
            Ok(response) => {
                selected_base = Some(base.clone());
                if response.status().is_success() {
                    return Err(ApiError::invalid_request(format!(
                        "host {host_name} already has service {}, refusing to overwrite",
                        services[0].0
                    )));
                }
                break;
            }
            Err(error) => last_error = error.to_string(),
        }
    }
    let Some(base) = selected_base else {
        return Err(ApiError::invalid_request(format!(
            "host {host_name} unreachable ({last_error}), nothing written"
        )));
    };

    // Pre-flight every name on the reachable base before writing any: a
    // conflict rejects the whole import instead of overwriting.
    for (name, _) in &services {
        let url = super::hosts::join_url(&base, &format!("/services/{name}"))
            .map_err(|(_, message)| ApiError::invalid_request(message))?;
        let response = client.get(url).send().await.map_err(|error| {
            ApiError::invalid_request(format!(
                "host {host_name} preflight for {name} failed: {error}"
            ))
        })?;
        match response.status() {
            status if status.as_u16() == 404 => {}
            status if status.is_success() => {
                return Err(ApiError::invalid_request(format!(
                    "host {host_name} already has service {name}, refusing to overwrite"
                )))
            }
            status => {
                return Err(ApiError::invalid_request(format!(
                    "host {host_name} preflight for {name} returned {status}, nothing written"
                )))
            }
        }
    }

    let mut imported = Vec::new();
    let mut failed = Vec::new();
    for (name, config) in &services {
        let summary = json!({"name": name, "transport": config.infer_transport()});
        let url = match super::hosts::join_url(&base, &format!("/services/{name}")) {
            Ok(url) => url,
            Err((_, message)) => {
                failed.push(json!({"name": name, "error": message}));
                continue;
            }
        };
        let body = match serde_json::to_value(config) {
            Ok(body) => body,
            Err(error) => {
                failed.push(json!({"name": name, "error": error.to_string()}));
                continue;
            }
        };
        match client.post(url).json(&body).send().await {
            Ok(response) if response.status().is_success() => imported.push(summary),
            Ok(response) => failed.push(json!({
                "name": name,
                "error": format!("host {host_name} returned status {}", response.status()),
            })),
            Err(error) => failed.push(json!({"name": name, "error": error.to_string()})),
        }
    }
    if imported.is_empty() && !failed.is_empty() {
        let names = failed
            .iter()
            .filter_map(|item| item["name"].as_str().map(str::to_owned))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ApiError::invalid_request(format!(
            "Cross-resource import to {host_name} failed entirely: {names}"
        )));
    }
    Ok(success(
        format!("Cross-resource import completed: definitions will execute on {host_name}"),
        json!({"imported": imported, "failed": failed, "scope": "store", "target": host_name}),
    ))
}

/// Best-effort CLI semver; runs off the async runtime with a hard timeout.
pub(super) async fn client_version(Path(client_id): Path<String>) -> ApiResult {
    if mcpstore::agents::by_id(&client_id).is_none() {
        return Err(ApiError::invalid_parameter(
            format!("unknown client: {client_id}"),
            Some("client_id"),
        ));
    }
    let probe_id = client_id.clone();
    let version = tokio::time::timeout(
        Duration::from_millis(800),
        tokio::task::spawn_blocking(move || {
            mcpstore::agents::by_id(&probe_id)
                .and_then(|setup| mcpstore::agents::cli_version(setup.as_ref()))
        }),
    )
    .await
    .ok()
    .and_then(|result| result.ok())
    .flatten();
    Ok(success(
        "Client version fetched successfully",
        json!({ "client_id": client_id, "version": version }),
    ))
}

/// Read-only inventory: every known client, discovered, inspected, and matched
/// against the wire ledger. Never writes.
pub(super) async fn clients_list<T: PanelCtx>(State(state): State<Arc<T>>) -> ApiResult {
    let wired = mcpstore::client_wire::load_wired_state(&wired_state_path(state.as_ref()))
        .unwrap_or_default();
    let clients = mcpstore::agents::all()
        .iter()
        .map(|setup| {
            let detection = mcpstore::agents::detect(setup.as_ref());
            let detected_by = detection
                .as_ref()
                .map(|detection| serde_json::to_value(detection.by).unwrap_or(Value::Null));
            let config_exists = detection
                .as_ref()
                .is_some_and(|detection| detection.by == mcpstore::agents::DetectedBy::ConfigFile);
            let config_path = match &detection {
                Some(detection) if config_exists => detection.path.display().to_string(),
                _ => setup
                    .default_config_paths()
                    .first()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
            };
            // Prefer the command-detection path when that is how we found the
            // client; otherwise resolve cli_commands against PATH independently
            // so config_file hits still surface the real binary.
            let executable_path = match &detection {
                Some(detection) if detection.by == mcpstore::agents::DetectedBy::Command => {
                    Some(detection.path.display().to_string())
                }
                _ => mcpstore::agents::cli_executable(setup.as_ref())
                    .map(|path| path.display().to_string()),
            };
            let inspection = if config_exists {
                inspect_client_config(setup.id(), &detection.as_ref().unwrap().path).ok()
            } else {
                None
            };
            let mcp_servers = inspection.as_ref().map(|inspection| {
                inspection
                    .services
                    .iter()
                    .map(|service| service.name.clone())
                    .collect::<Vec<_>>()
            });
            let unsupported_fields = inspection
                .map(|inspection| inspection.unsupported_fields)
                .unwrap_or_default();
            let entry = wired
                .entries
                .iter()
                .find(|entry| entry.client_id == setup.id());
            json!({
                "id": setup.id(),
                "label": setup.label(),
                "detected_by": detected_by,
                "config_path": config_path,
                "executable_path": executable_path,
                "config_exists": config_exists,
                "mcp_servers": mcp_servers,
                "unsupported_fields": unsupported_fields,
                "wired": entry.is_some(),
                "wired_url": entry.map(|entry| entry.url.clone()),
            })
        })
        .collect::<Vec<_>>();
    Ok(success(
        "Client inventory fetched successfully",
        json!({ "clients": clients }),
    ))
}

#[cfg(test)]
mod tests {
    use super::{gateway_entry_host, gateway_entry_url, parse_gateway_boot_line};
    use crate::gateway::GatewayBootPhase;

    #[test]
    fn parses_only_machine_readable_boot_lines() {
        let boot = parse_gateway_boot_line(
            r#"[GATEWAY_BOOT] {"phase":"connecting","done":12,"total":40,"message":"brave_search"}"#,
        )
        .expect("boot marker")
        .expect("valid boot JSON");
        assert_eq!(boot.phase, GatewayBootPhase::Connecting);
        assert_eq!(boot.done, Some(12));
        assert_eq!(boot.total, Some(40));
        assert_eq!(boot.message.as_deref(), Some("brave_search"));
        assert!(boot.phase.is_starting());

        assert!(parse_gateway_boot_line("[GATEWAY] ordinary log").is_none());
        assert!(parse_gateway_boot_line("[GATEWAY_BOOT] not-json")
            .expect("boot marker")
            .is_err());
    }

    #[test]
    fn entry_host_follows_bind_surface() {
        // loopback bind: literal 127.0.0.1
        assert_eq!(gateway_entry_host("127.0.0.1"), "127.0.0.1");
        assert_eq!(gateway_entry_host("localhost"), "127.0.0.1");
        // all-interfaces bind: primary LAN address (deterministic only in
        // that it is a valid, non-loopback IPv4)
        let lan = gateway_entry_host("0.0.0.0");
        let parsed: std::net::IpAddr = lan.parse().expect("lan address parses");
        assert!(!parsed.is_loopback());
        // explicit host passes through untouched
        assert_eq!(gateway_entry_host("192.168.1.10"), "192.168.1.10");
        assert_eq!(
            gateway_entry_host("gateway.example.com"),
            "gateway.example.com"
        );
    }

    #[test]
    fn entry_url_shape_is_stable() {
        let url = gateway_entry_url("127.0.0.1", 1830, "claude-code");
        assert_eq!(url, "http://127.0.0.1:1830/mcp/claude-code");
    }
}
