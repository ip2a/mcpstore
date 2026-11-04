use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::gateway::{GatewayLaunchDescriptor, GatewayOptions};
use axum::{
    extract::State,
    routing::{get, post, put},
    Router,
};
use mcpstore::{
    client_config::{import_selected_services, inspect_client_config},
    config::{ConfigManager, ScopeDescriptor},
    AuthFlow, InstanceId, MCPStore, McpCompletionRequest, OpenApiBundleOptions,
    OpenApiImportOptions, OpenApiRefCachePolicy, PromptOverridePatch, ResourceOverridePatch,
    ResourceTemplateOverridePatch, ScopeRef, ScopeView, ServerConfig, ServiceOverridePatch,
    ToolOverridePatch,
};
use serde_json::json;
#[cfg(test)]
use serde_json::Value;
use tower_http::cors::CorsLayer;

mod app;
mod auth;
mod cache;
pub(crate) mod client;
mod config_ops;
mod envelope;
mod filesystem;
mod hosts;
mod openapi;
mod panel_settings;
mod parse;
mod plugins;
mod service;
mod session;
mod telemetry;

use envelope::{success, ApiError, ApiResult};

use parse::{
    extract_prompt_args, extract_prompt_name, extract_tool_args, extract_tool_name,
    parse_scope_ref, ScopeQuery,
};

/// What a panel-local handler needs from its host process. The daemon's
/// [`ApiState`] and the standalone web panel's [`PanelState`] both provide it;
/// [`PanelCtx::daemon`] tells them apart — `Some` in the daemon (local imports
/// land in-process), `None` in the web panel (the local daemon is reached over
/// HTTP like any other host).
pub(crate) trait PanelCtx: Send + Sync + 'static {
    fn config_manager(&self) -> &ConfigManager;
    fn daemon(&self) -> Option<&MCPStore>;
    fn http_client(&self) -> &reqwest::Client;
}

#[derive(Clone)]
pub struct ApiState {
    store: Arc<MCPStore>,
    gateway: Arc<Mutex<Option<GatewayProcess>>>,
    /// Host-side telemetry persistence for history/detail/stats queries. The
    /// recording path goes through the store's sink; queries need the concrete type.
    telemetry: Arc<Mutex<Option<std::sync::Arc<crate::telemetry::TelemetryLog>>>>,
    http_client: reqwest::Client,
}

/// State of the standalone web panel process: no embedded store, panel assets
/// on local disk, every daemon — including the local one — reached over HTTP.
#[derive(Clone)]
pub(crate) struct PanelState {
    config_manager: Arc<ConfigManager>,
    http_client: reqwest::Client,
}

impl PanelState {
    pub(crate) fn new(config_manager: Arc<ConfigManager>) -> Self {
        Self {
            config_manager,
            http_client: host_http_client(),
        }
    }
}

impl PanelCtx for ApiState {
    fn config_manager(&self) -> &ConfigManager {
        self.store.config_manager()
    }

    fn daemon(&self) -> Option<&MCPStore> {
        Some(&self.store)
    }

    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
}

impl PanelCtx for PanelState {
    fn config_manager(&self) -> &ConfigManager {
        &self.config_manager
    }

    fn daemon(&self) -> Option<&MCPStore> {
        None
    }

    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
}

/// One-time panel asset adoption at panel startup: a panel-settings.json from
/// before the hosts move adopts the daemon config's `[hosts]` section. Doing
/// it at boot keeps the request-routing hot path read-only.
pub(crate) fn adopt_panel_settings(manager: &ConfigManager) -> Result<(), String> {
    let _persistence = panel_settings::persistence_lock();
    panel_settings::load_or_migrate(manager).map(|_| ())
}

fn host_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("host routing HTTP client configuration is valid")
}

/// A gateway child process (`mcpstore gateway`) spawned by POST /gateway/start.
/// The routed-call log lives inside the child; /gateway/requests forwards to it.
struct GatewayProcess {
    child: tokio::process::Child,
    pid: u32,
    /// Management URL of the child's /calls route.
    calls_url: String,
    descriptor: GatewayLaunchDescriptor,
    summary: serde_json::Value,
}

/// Resolve `(service_name, scope)` to an instance_id; 404 when the service isn't declared in that scope.
/// New-style URLs address by service name + scope; instance_id stays internal to the API layer and is no longer exposed to users.
async fn resolve_instance(
    state: &ApiState,
    service_name: &str,
    scope: &ScopeRef,
) -> ApiResult<InstanceId> {
    state
        .store
        .instance_id_for_scope(service_name, scope)
        .await
        .map_err(|error| {
            if error.code() == mcpstore::error::FailureCode::ServiceNotFound {
                ApiError::not_found(
                    mcpstore::error::FailureCode::ServiceNotFound,
                    format!("Service {service_name} is not declared in this scope"),
                    Some("service_name"),
                    Some(json!({
                        "service_name": service_name,
                        "scope": serde_json::to_value(scope).unwrap_or(serde_json::Value::Null),
                    })),
                )
            } else {
                ApiError::from_store(error)
            }
        })
}

/// Build the daemon-shared ApiState.
pub fn state_for_store(store: Arc<MCPStore>) -> Arc<ApiState> {
    Arc::new(ApiState {
        store,
        gateway: Arc::new(Mutex::new(None)),
        telemetry: Arc::new(Mutex::new(None)),
        http_client: host_http_client(),
    })
}

impl ApiState {
    pub(crate) fn set_telemetry(&self, log: std::sync::Arc<crate::telemetry::TelemetryLog>) {
        *self.telemetry.lock().unwrap() = Some(log);
    }

    pub(crate) fn telemetry(&self) -> Option<std::sync::Arc<crate::telemetry::TelemetryLog>> {
        self.telemetry.lock().unwrap().clone()
    }
}

pub fn router_for_store(store: Arc<MCPStore>, prefix: &str) -> Router {
    full_router(state_for_store(store), prefix)
}

/// App plane: panel-local operations plus selected-host resource forwarding.
pub fn app_router(state: Arc<ApiState>) -> Router {
    let resources = resource_routes(Arc::clone(&state))
        .merge(core_routes(Arc::clone(&state)))
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            hosts::route_selected_host::<ApiState>,
        ));
    // Cross-resource import orchestrates in the handler: read the client
    // config on this host, then write parsed definitions to the selected
    // daemon. It neither proxies the whole request nor rejects the selector.
    let orchestrated_import = Router::new()
        .route(
            "/client-config/import",
            post(client::client_config_import::<ApiState>),
        )
        .with_state(Arc::clone(&state));
    app_routes::<ApiState>()
        .layer(axum::middleware::from_fn(hosts::reject_selected_host))
        .with_state(state)
        .merge(resources)
        .merge(orchestrated_import)
        .layer(CorsLayer::permissive())
}

/// Router of the standalone web panel: panel-local operations answer locally,
/// everything else forwards to a daemon — a selector routes to that host, and
/// without a selector the panel's "local" entry is the target. No embedded
/// store anywhere: a missing or unreachable daemon is a loud 503.
pub(crate) fn panel_router<T: PanelCtx>(state: Arc<T>) -> Router {
    let app = app_routes::<T>()
        .layer(axum::middleware::from_fn(hosts::reject_selected_host))
        .with_state(Arc::clone(&state));
    let import = Router::new()
        .route(
            "/client-config/import",
            post(client::client_config_import::<T>),
        )
        .with_state(Arc::clone(&state));
    let forward = Router::<Arc<T>>::new()
        .fallback(hosts::forward_any::<T>)
        .with_state(Arc::clone(&state));
    app.merge(import)
        .merge(forward)
        .layer(CorsLayer::permissive())
}

/// Core plane: daemon business and resource-local app operations.
pub fn core_router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/health", get(app::health))
        .route("/v1/daemon/meta", get(app::daemon_meta))
        .with_state(state.clone())
        .merge(resource_routes(Arc::clone(&state)))
        .merge(core_routes(state))
        .layer(CorsLayer::permissive())
}

fn full_router(state: Arc<ApiState>, prefix: &str) -> Router {
    let full = app_routes::<ApiState>()
        .with_state(Arc::clone(&state))
        .merge(resource_routes(Arc::clone(&state)))
        .merge(core_routes(Arc::clone(&state)))
        .merge(
            Router::new()
                .route(
                    "/client-config/import",
                    post(client::client_config_import::<ApiState>),
                )
                .with_state(Arc::clone(&state)),
        )
        .layer(CorsLayer::permissive());
    if prefix.is_empty() {
        full
    } else {
        Router::new().nest(prefix, full)
    }
}

fn app_routes<T: PanelCtx>() -> Router<Arc<T>> {
    Router::new()
        .route("/health", get(app::health::<T>))
        .route("/v1/meta", get(app::meta::<T>))
        .route("/v1/settings", put(app::update_settings::<T>))
        .route("/v1/hosts/:name/status", get(hosts::host_status::<T>))
        .route("/clients/:id/version", get(client::client_version))
        .route("/clients", get(client::clients_list::<T>))
        .route("/filesystem/directories", get(filesystem::directories_list::<T>))
        .route("/plugins", get(plugins::plugins_list::<T>))
        .route("/plugins/market", get(plugins::plugins_market::<T>))
        .route("/plugins/detect", post(plugins::plugins_detect::<T>))
        .route("/plugins/:id/install", post(plugins::plugin_install::<T>))
        .route(
            "/plugins/:id/uninstall",
            post(plugins::plugin_uninstall::<T>),
        )
        .route("/plugins/:id/enable", post(plugins::plugin_enable::<T>))
        .route("/plugins/:id/disable", post(plugins::plugin_disable::<T>))
        .route("/gateway/wired", get(client::gateway_wired::<T>))
        .route("/gateway/wire", post(client::gateway_wire::<T>))
        .route("/gateway/unwire", post(client::gateway_unwire::<T>))
}

fn resource_routes(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/config", get(service::store_show_config))
        .route("/config/reset", post(service::store_reset_config))
        .route("/gateway/descriptor", get(client::gateway_descriptor))
        .route("/gateway/status", get(client::gateway_status))
        .route("/gateway/requests", get(client::gateway_requests))
        .route(
            "/gateway/requests/history",
            get(telemetry::requests_history),
        )
        .route(
            "/gateway/requests/detail/:id",
            get(telemetry::request_detail),
        )
        .route("/gateway/stats", get(telemetry::usage_stats))
        .route("/gateway/stats/daily", get(telemetry::usage_stats_daily))
        .route(
            "/gateway/stats/heatmap",
            get(telemetry::usage_stats_heatmap),
        )
        .route(
            "/gateway/stats/frame-activity",
            get(telemetry::frame_activity_stats),
        )
        .route(
            "/gateway/stats/frame-activity-by-service",
            get(telemetry::frame_activity_by_service),
        )
        .route(
            "/gateway/stats/frame-activity-by-method",
            get(telemetry::frame_activity_by_method),
        )
        .route("/gateway/frames/sessions", get(telemetry::frame_sessions))
        .route("/gateway/frames/history", get(telemetry::frame_history))
        .route("/gateway/frames/recent", get(telemetry::frame_recent))
        .route("/gateway/frames/detail/:id", get(telemetry::frame_detail))
        .route("/gateway/start", post(client::gateway_start))
        .route("/gateway/stop", post(client::gateway_stop))
        .route("/cache/health", get(cache::health))
        .route("/cache/inspect", get(cache::inspect))
        .route("/cache/switch", post(cache::switch))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            audit_mutations,
        ))
        .with_state(state)
}

fn core_routes(state: Arc<ApiState>) -> Router {
    Router::new()
        // ===== agents / scopes =====
        .route("/agents/list", get(service::list_agents))
        .route("/agents/:agent_id", get(service::agent_info))
        .route("/scopes/list", get(service::scopes_list))
        .route("/scopes/root", get(service::scope_info_root))
        .route("/scopes/store", get(service::scope_info_store))
        .route("/scopes/agents/:agent_id", get(service::scope_info_agent))
        .route(
            "/scopes/agents/:agent_id/config",
            get(service::agent_show_config),
        )
        .route(
            "/scopes/agents/:agent_id/reset",
            post(service::agent_reset_config),
        )
        // ===== services: service definitions (root-level CRUD) + service info =====
        .route("/services/list", get(service::service_list_services))
        .route(
            "/services/:service_name",
            get(service::service_info)
                .post(service::add_service_definition)
                .put(service::update_service_definition)
                .delete(service::remove_service_definition),
        )
        // ===== services: lifecycle / status (service name + scope addressing) =====
        .route("/services/:service_name/state", get(service::service_state))
        .route(
            "/services/:service_name/connect",
            post(service::service_connect),
        )
        .route(
            "/services/:service_name/disconnect",
            post(service::service_disconnect),
        )
        .route(
            "/services/:service_name/restart",
            post(service::service_restart),
        )
        .route("/services/:service_name/wait", get(service::service_wait))
        .route("/services/:service_name/check", get(service::service_check))
        // ===== tools: service-nested form + top-level form =====
        .route(
            "/services/:service_name/tools/list",
            get(service::service_list_tools),
        )
        .route(
            "/services/:service_name/tools/call",
            post(service::service_call_tool),
        )
        .route("/tools/list", get(service::tools_list))
        .route("/tools/call", post(service::tools_call))
        // ===== resources / prompts (service name + scope addressing) =====
        .route(
            "/services/:service_name/resources/list",
            get(service::service_list_resources),
        )
        .route(
            "/services/:service_name/resources/templates",
            get(service::service_list_resource_templates),
        )
        .route(
            "/services/:service_name/resources/read",
            get(service::service_read_resource),
        )
        .route(
            "/services/:service_name/prompts/list",
            get(service::service_list_prompts),
        )
        .route(
            "/services/:service_name/prompts/get",
            post(service::service_get_prompt),
        )
        // ===== services: scope declarations =====
        .route(
            "/services/:service_name/scopes/store",
            put(service::declare_store_scope).delete(service::remove_store_scope),
        )
        .route(
            "/services/:service_name/scopes/agents/:agent_id",
            put(service::declare_agent_scope).delete(service::remove_agent_scope),
        )
        // ===== sessions (collapsed to a single form: session_key via query/body) =====
        .route("/sessions/create", post(session::session_create))
        .route("/sessions/get", get(session::session_get))
        .route("/sessions/find", get(session::session_find))
        .route("/sessions/list", get(session::session_list))
        .route("/sessions/snapshot", get(session::session_export_snapshot))
        .route(
            "/sessions/snapshot/import",
            post(session::session_import_snapshot),
        )
        .route("/sessions/status", get(session::session_status))
        .route("/sessions/close", post(session::session_close))
        .route("/sessions/extend", post(session::session_extend))
        .route(
            "/sessions/bind_service",
            post(session::session_bind_service),
        )
        .route(
            "/sessions/unbind_service",
            post(session::session_unbind_service),
        )
        .route(
            "/sessions/list_services",
            get(session::session_list_services),
        )
        .route("/sessions/list_tools", get(session::session_list_tools))
        .route("/sessions/call_tool", post(session::session_call_tool))
        .route("/sessions/state/list", get(session::session_list_state))
        .route(
            "/sessions/state/value",
            get(session::session_get_state_value),
        )
        .route("/sessions/state/set", post(session::session_set_state))
        .route(
            "/sessions/state/delete",
            post(session::session_delete_state),
        )
        .route("/sessions/state/clear", post(session::session_clear_state))
        // ===== tool policy / override rules / completions / resource subscriptions (service name + scope addressing) =====
        .route(
            "/services/:service_name/tool-policy",
            get(service::service_get_tool_policy)
                .put(service::service_set_tool_policy)
                .delete(service::service_clear_tool_policy),
        )
        .route("/tool_overrides", get(service::store_list_tool_overrides))
        .route(
            "/services/:service_name/tool_overrides/:tool_name",
            get(service::service_get_tool_override)
                .put(service::service_set_tool_override)
                .delete(service::service_delete_tool_override),
        )
        // ===== config management =====
        .route("/config/export", get(config_ops::config_export))
        .route("/config/reload", post(config_ops::config_reload))
        .route("/config/import", post(config_ops::config_import))
        .route(
            "/prompt_overrides",
            get(service::store_list_prompt_overrides),
        )
        .route(
            "/services/:service_name/prompt_overrides/:prompt_name",
            get(service::service_get_prompt_override)
                .put(service::service_set_prompt_override)
                .delete(service::service_delete_prompt_override),
        )
        .route(
            "/resource_overrides",
            get(service::store_list_resource_overrides),
        )
        .route(
            "/services/:service_name/resource_overrides",
            get(service::service_get_resource_override)
                .put(service::service_set_resource_override)
                .delete(service::service_delete_resource_override),
        )
        .route(
            "/resource_template_overrides",
            get(service::store_list_resource_template_overrides),
        )
        .route(
            "/services/:service_name/resource_template_overrides",
            get(service::service_get_resource_template_override)
                .put(service::service_set_resource_template_override)
                .delete(service::service_delete_resource_template_override),
        )
        .route(
            "/services/:service_name/service_override",
            get(service::service_get_service_override)
                .put(service::service_set_service_override)
                .delete(service::service_delete_service_override),
        )
        .route(
            "/services/:service_name/completions",
            post(service::service_complete_argument),
        )
        .route(
            "/services/:service_name/resources/subscribe",
            post(service::service_subscribe_resource),
        )
        .route(
            "/services/:service_name/resources/unsubscribe",
            post(service::service_unsubscribe_resource),
        )
        // ===== OAuth (service name + scope addressing) =====
        .route(
            "/services/:service_name/auth",
            get(auth::service_auth_status),
        )
        .route(
            "/services/:service_name/auth/start",
            post(auth::service_auth_start),
        )
        .route(
            "/services/:service_name/auth/callback",
            get(auth::service_auth_callback_get).post(auth::service_auth_callback_post),
        )
        .route(
            "/services/:service_name/auth/refresh",
            post(auth::service_auth_refresh),
        )
        .route(
            "/services/:service_name/auth/logout",
            post(auth::service_auth_logout),
        )
        .route(
            "/services/:service_name/auth/client-secret",
            post(auth::service_auth_save_client_secret),
        )
        .route(
            "/services/:service_name/auth/private-key",
            post(auth::service_auth_save_private_key),
        )
        .route(
            "/services/:service_name/auth/scope-upgrade",
            post(auth::service_auth_scope_upgrade),
        )
        // ===== OpenAPI import (kept) =====
        .route("/openapi_imports", get(openapi::store_list_openapi_imports))
        .route(
            "/openapi_imports/:name",
            get(openapi::store_get_openapi_import_by_path),
        )
        .route(
            "/openapi_imports/:name/import",
            post(openapi::store_import_openapi_by_path),
        )
        .route(
            "/openapi_imports/bundle",
            post(openapi::store_bundle_openapi),
        )
        .route(
            "/openapi_imports/bundle_artifact",
            post(openapi::store_bundle_openapi_artifact),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            audit_mutations,
        ))
        .with_state(state)
}

/// Audit every mutating API operation (POST/PUT/DELETE on the app and core planes)
/// into request telemetry: method, path, query, body, duration, HTTP outcome.
/// Read-only calls are not recorded. The request body is buffered once so the
/// handler sees the same bytes; 2 MiB cap matches the existing API payloads.
async fn audit_mutations(
    State(state): State<Arc<ApiState>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if matches!(
        request.method(),
        &axum::http::Method::GET | &axum::http::Method::HEAD | &axum::http::Method::OPTIONS
    ) {
        return next.run(request).await;
    }
    let Some(telemetry) = state.store.telemetry() else {
        return next.run(request).await;
    };
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(str::to_string);
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, 2 * 1024 * 1024)
        .await
        .unwrap_or_default();
    let body_text = (!body.is_empty()).then(|| String::from_utf8_lossy(&body).into_owned());
    let started = std::time::Instant::now();
    let response = next
        .run(axum::extract::Request::from_parts(
            parts,
            axum::body::Body::from(body),
        ))
        .await;
    let mut record =
        mcpstore::telemetry::RequestRecord::new("management", format!("{method} {path}"));
    record.duration_ms = started.elapsed().as_millis() as i64;
    record.args_json = Some(json!({ "query": query, "body": body_text }).to_string());
    record.success = response.status().is_success();
    if !record.success {
        record.error_summary = Some(format!("HTTP {}", response.status().as_u16()));
    }
    telemetry.record(record);
    response
}

#[cfg(test)]
mod tests;
