use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::gateway::{
    run_streamable_http, GatewayCallLog, GatewayLaunchDescriptor, GatewayOptions, GatewayServer,
};
use axum::{
    extract::State,
    routing::{get, post, put},
    Router,
};
use mcpstore::{
    client_config::{import_selected_services, inspect_client_config},
    config::ScopeDescriptor,
    AuthFlow, InstanceId, MCPStore, McpCompletionRequest, OpenApiBundleOptions,
    OpenApiImportOptions, OpenApiRefCachePolicy, PromptOverridePatch, ResourceOverridePatch,
    ResourceTemplateOverridePatch, ScopeRef, ScopeView, ServerConfig, ToolOverridePatch,
};
use serde_json::json;
#[cfg(test)]
use serde_json::Value;
use tokio::task::JoinHandle;
use tower_http::cors::CorsLayer;

mod app;
mod auth;
mod cache;
pub(crate) mod client;
mod envelope;
mod openapi;
mod parse;
mod service;
mod session;
mod telemetry;

use envelope::{success, ApiError, ApiResult};

use parse::{
    extract_prompt_args, extract_prompt_name, extract_tool_args, extract_tool_name,
    parse_scope_ref, ScopeQuery,
};

#[derive(Clone)]
pub struct ApiState {
    store: Arc<MCPStore>,
    gateway: Arc<Mutex<Option<GatewayProcess>>>,
    /// Call log of the daemon-face gateway (mounted at startup; no GatewayProcess).
    face_log: Arc<Mutex<Option<std::sync::Arc<GatewayCallLog>>>>,
    /// Host-side telemetry persistence for history/detail/stats queries. The
    /// recording path goes through the store's sink; queries need the concrete type.
    telemetry: Arc<Mutex<Option<std::sync::Arc<crate::telemetry::TelemetryLog>>>>,
}

struct GatewayProcess {
    task: JoinHandle<()>,
    descriptor: GatewayLaunchDescriptor,
    call_log: std::sync::Arc<GatewayCallLog>,
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
        face_log: Arc::new(Mutex::new(None)),
        telemetry: Arc::new(Mutex::new(None)),
    })
}

impl ApiState {
    pub(crate) fn store(&self) -> &Arc<MCPStore> {
        &self.store
    }

    pub(crate) fn set_face_log(&self, log: std::sync::Arc<GatewayCallLog>) {
        *self.face_log.lock().unwrap() = Some(log);
    }

    pub(crate) fn face_log(&self) -> Option<std::sync::Arc<GatewayCallLog>> {
        self.face_log.lock().unwrap().clone()
    }

    pub(crate) fn set_telemetry(&self, log: std::sync::Arc<crate::telemetry::TelemetryLog>) {
        *self.telemetry.lock().unwrap() = Some(log);
    }

    pub(crate) fn telemetry(
        &self,
    ) -> Option<std::sync::Arc<crate::telemetry::TelemetryLog>> {
        self.telemetry.lock().unwrap().clone()
    }
}

pub fn router_for_store(store: Arc<MCPStore>, prefix: &str) -> Router {
    full_router(state_for_store(store), prefix)
}

/// App plane: the daemon itself (health/meta/settings/config/client import/gateway/cache), always local.
pub fn app_router(state: Arc<ApiState>) -> Router {
    app_routes(state).layer(CorsLayer::permissive())
}

/// Core plane: all store business; follows the core base to a remote daemon.
pub fn core_router(state: Arc<ApiState>) -> Router {
    core_routes(state).layer(CorsLayer::permissive())
}

fn full_router(state: Arc<ApiState>, prefix: &str) -> Router {
    let full = app_routes(state.clone())
        .merge(core_routes(state))
        .layer(CorsLayer::permissive());
    if prefix.is_empty() {
        full
    } else {
        Router::new().nest(prefix, full)
    }
}

fn app_routes(state: Arc<ApiState>) -> Router {
    Router::new()
        // ===== app: app config / meta / history (app-only, not core) =====
        .route("/health", get(app::health))
        .route("/v1/meta", get(app::meta))
        .route("/v1/settings", put(app::update_settings))
        // ===== config / client import / gateway / cache (app-only, not core) =====
        .route("/config", get(service::store_show_config))
        .route("/config/reset", post(service::store_reset_config))
        .route("/client-config/import", post(client::client_config_import))
        .route("/clients", get(client::clients_list))
        .route("/gateway/descriptor", get(client::gateway_descriptor))
        .route("/gateway/status", get(client::gateway_status))
        .route("/gateway/requests", get(client::gateway_requests))
        .route("/gateway/requests/history", get(telemetry::requests_history))
        .route("/gateway/requests/detail/:id", get(telemetry::request_detail))
        .route("/gateway/stats", get(telemetry::usage_stats))
        .route("/gateway/wired", get(client::gateway_wired))
        .route("/gateway/wire", post(client::gateway_wire))
        .route("/gateway/unwire", post(client::gateway_unwire))
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
