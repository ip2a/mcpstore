use std::{
    collections::HashMap,
    net::IpAddr,
    process::Stdio,
    sync::{Arc, Mutex},
};

use crate::mcp_server::{McpServerLaunchDescriptor, McpServerOptions, McpServerTransport};
use axum::{
    extract::State,
    routing::{get, post, put},
    Router,
};
use clap::Args;
use mcpstore::{
    client_config::{import_selected_services, inspect_client_config, ClientKind},
    config::ScopeDescriptor,
    AuthFlow, InstanceId, MCPStore, McpCompletionRequest, OpenApiBundleOptions,
    OpenApiImportOptions, OpenApiRefCachePolicy, PromptOverridePatch, ResourceOverridePatch,
    ResourceTemplateOverridePatch, ScopeRef, ScopeView, ServerConfig, ToolOverridePatch,
};
use serde_json::json;
#[cfg(test)]
use serde_json::Value;
use tokio::process::{Child, Command};
use tower_http::cors::CorsLayer;

use crate::{
    store_args::{build_store, StoreSourceArgs},
    BoxErr,
};

mod app;
mod auth;
mod cache;
mod client;
mod envelope;
mod openapi;
mod parse;
mod service;
mod session;

use envelope::{success, ApiError, ApiResult};

use parse::{
    extract_prompt_args, extract_prompt_name, extract_tool_args, extract_tool_name,
    normalize_prefix, parse_scope_ref, ScopeQuery,
};

#[derive(Args)]
pub struct ApiArgs {
    #[arg(long, help = "API 服务端口；未指定时读取 app 配置")]
    pub port: Option<u16>,
    #[arg(long, default_value = "127.0.0.1", help = "绑定地址")]
    pub host: String,
    #[arg(long, default_value = "", help = "URL 前缀，例如 /mcp")]
    pub url_prefix: String,
    #[arg(long, help = "显式允许非 loopback API 绑定")]
    pub allow_remote: bool,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Clone)]
pub struct ApiState {
    store: Arc<MCPStore>,
    mcp_hub_process: Arc<Mutex<Option<McpHubProcess>>>,
}

struct McpHubProcess {
    child: Child,
    descriptor: McpServerLaunchDescriptor,
}

impl Drop for McpHubProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// Resolve `(service_name, scope)` into an instance_id; returns 404 when the service is not declared in that scope.
/// New-style URLs are addressed by "service name + scope"; instance_id only flows inside the API layer and is no longer exposed to users.
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
                    format!("服务 {service_name} 未在该作用域声明"),
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

pub async fn run(args: ApiArgs) -> Result<(), BoxErr> {
    let loopback = args.host == "localhost"
        || args
            .host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !loopback && !args.allow_remote {
        return Err("API 默认只允许 loopback 绑定；使用 --allow-remote 明确开启远程暴露".into());
    }

    let store = build_store(&args.store)?;
    store.load_from_source().await?;

    let config = store.config_manager().load_app_config_or_default()?;
    let port = args.port.unwrap_or(config.server.port);

    let prefix = normalize_prefix(&args.url_prefix);
    let app = router_for_store(store, &prefix);

    let addr = format!("{}:{}", args.host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let display_prefix = if prefix.is_empty() {
        "/".to_string()
    } else {
        prefix.clone()
    };
    println!("[API] Starting at http://{addr}{display_prefix}");

    axum::serve(listener, app).await?;
    Ok(())
}

pub fn router_for_store(store: Arc<MCPStore>, prefix: &str) -> Router {
    let state = Arc::new(ApiState {
        store,
        mcp_hub_process: Arc::new(Mutex::new(None)),
    });
    if !state.store.is_data_plane() {
        let store = state.store.clone();
        tokio::spawn(async move {
            if let Err(error) = store.restart_control_reactor().await {
                tracing::error!(
                    "[API] Failed to restart event reactor after cache switch: {error}"
                );
            }
        });
    }
    router(state, prefix)
}

fn router(state: Arc<ApiState>, prefix: &str) -> Router {
    let base = Router::new()
        // ===== app: app config / meta / history (app-only, not core) =====
        .route("/health", get(app::health))
        .route("/v1/meta", get(app::meta))
        .route("/v1/settings", put(app::update_settings))
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
        // ===== services: lifecycle / status (addressed by service name + scope) =====
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
        // ===== resources / prompts (addressed by service name + scope) =====
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
        // ===== sessions (collapsed into a single form: session_key via query/body) =====
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
        // ===== Tool policies / conversion rules / completion / resource subscriptions (addressed by service name + scope) =====
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
        // ===== OAuth auth (addressed by service name + scope) =====
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
        // ===== OpenAPI import (legacy but kept) =====
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
        // ===== Config / coding assistant / aggregate / cache (app-only, not core) =====
        .route("/config", get(service::store_show_config))
        .route("/config/reset", post(service::store_reset_config))
        .route("/client-config/import", post(client::client_config_import))
        .route("/mcp-hub/descriptor", get(client::mcp_hub_descriptor))
        .route("/mcp-hub/status", get(client::mcp_hub_status))
        .route("/mcp-hub/start", post(client::mcp_hub_start))
        .route("/mcp-hub/stop", post(client::mcp_hub_stop))
        .route("/cache/health", get(cache::health))
        .route("/cache/inspect", get(cache::inspect))
        .route("/cache/switch", post(cache::switch))
        .with_state(state);

    if prefix.is_empty() {
        base.layer(CorsLayer::permissive())
    } else {
        Router::new()
            .nest(prefix, base)
            .layer(CorsLayer::permissive())
    }
}

#[cfg(test)]
mod tests;
