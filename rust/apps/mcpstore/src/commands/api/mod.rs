use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
};

use axum::{
    extract::State,
    routing::{get, post, put},
    Router,
};
use clap::Args;
use mcpstore::{
    client_config::{
        apply_config_change, import_selected_services, inspect_client_config, plan_add_entries,
        ClientConfigInspection, ClientEntryKind, ClientEntryPlan, ClientEntrySpec,
        ClientEntryStatus, ClientKind, ConfigChangeReceipt,
    },
    config::ScopeDescriptor,
    mcp_server::{McpServerOptions, McpServerTransport},
    AuthFlow, InstanceId, MCPStore, McpCompletionRequest, McpLoggingLevel, OpenApiBundleOptions,
    OpenApiImportOptions, OpenApiRefCachePolicy, ScopeRef, ServerConfig, ToolTransformPatch,
};
use serde_json::json;
#[cfg(test)]
use serde_json::Value;
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
    extract_prompt_args, extract_prompt_name, extract_resource_uri, extract_tool_args,
    extract_tool_name, normalize_prefix, parse_positive_u64,
};

#[derive(Args)]
pub struct ApiArgs {
    #[arg(long, default_value_t = 18200, help = "API 服务端口")]
    pub port: u16,
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
    client_changes: Arc<Mutex<HashMap<String, ConfigChangeReceipt>>>,
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

    let prefix = normalize_prefix(&args.url_prefix);
    let app = router_for_store(store, &prefix);

    let addr = format!("{}:{}", args.host, args.port);
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
        client_changes: Arc::new(Mutex::new(HashMap::new())),
    });
    if !state.store.is_db_source() {
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
        .route("/health", get(app::health))
        .route("/v1/meta", get(app::meta))
        .route("/v1/settings", put(app::update_settings))
        .route("/agents/list", get(service::list_agents))
        .route("/events/history", get(app::event_history))
        .route("/history/tool-calls", get(app::tool_call_history))
        .route(
            "/history/tool-calls/clear",
            post(app::clear_tool_call_history),
        )
        .route(
            "/events/capability_report",
            get(app::event_capability_report),
        )
        .route("/sessions/create", post(session::session_create))
        .route("/sessions/get/:session_key", get(session::session_get))
        .route("/sessions/get", get(session::session_get_by_query))
        .route("/sessions/find", get(session::session_find))
        .route("/sessions/list", get(session::session_list))
        .route("/sessions/snapshot", get(session::session_export_snapshot))
        .route(
            "/sessions/snapshot/import",
            post(session::session_import_snapshot),
        )
        .route(
            "/sessions/status/:session_key",
            get(session::session_status),
        )
        .route("/sessions/status", get(session::session_status_by_query))
        .route("/sessions/close", post(session::session_close_by_body))
        .route("/sessions/close/:session_key", post(session::session_close))
        .route("/sessions/extend", post(session::session_extend_by_body))
        .route(
            "/sessions/extend/:session_key",
            post(session::session_extend),
        )
        .route(
            "/sessions/bind_service",
            post(session::session_bind_service_by_body),
        )
        .route(
            "/sessions/bind_service/:session_key",
            post(session::session_bind_service),
        )
        .route(
            "/sessions/unbind_service",
            post(session::session_unbind_service_by_body),
        )
        .route(
            "/sessions/unbind_service/:session_key",
            post(session::session_unbind_service),
        )
        .route(
            "/sessions/list_services",
            get(session::session_list_services_by_query),
        )
        .route(
            "/sessions/list_services/:session_key",
            get(session::session_list_services),
        )
        .route(
            "/sessions/list_tools",
            get(session::session_list_tools_by_query),
        )
        .route(
            "/sessions/list_tools/:session_key",
            get(session::session_list_tools),
        )
        .route(
            "/sessions/call_tool",
            post(session::session_call_tool_by_body),
        )
        .route(
            "/sessions/call_tool/:session_key",
            post(session::session_call_tool),
        )
        .route(
            "/sessions/state/list",
            get(session::session_list_state_by_query),
        )
        .route(
            "/sessions/state/list/:session_key",
            get(session::session_list_state),
        )
        .route(
            "/sessions/state/value",
            get(session::session_get_state_value),
        )
        .route(
            "/sessions/state/set",
            post(session::session_set_state_by_body),
        )
        .route(
            "/sessions/state/set/:session_key",
            post(session::session_set_state),
        )
        .route(
            "/sessions/state/delete",
            post(session::session_delete_state_by_body),
        )
        .route(
            "/sessions/state/delete/:session_key",
            post(session::session_delete_state),
        )
        .route(
            "/sessions/state/clear",
            post(session::session_clear_state_by_body),
        )
        .route(
            "/sessions/state/clear/:session_key",
            post(session::session_clear_state),
        )
        .route("/scopes/store/instances", get(service::store_list_services))
        .route(
            "/scopes/agents/:agent_id/instances",
            get(service::agent_list_services),
        )
        .route(
            "/services/:service_name",
            post(service::add_service_definition)
                .put(service::update_service_definition)
                .delete(service::remove_service_definition),
        )
        .route(
            "/services/:service_name/scopes/store",
            put(service::declare_store_scope).delete(service::remove_store_scope),
        )
        .route(
            "/services/:service_name/scopes/agents/:agent_id",
            put(service::declare_agent_scope).delete(service::remove_agent_scope),
        )
        .route(
            "/instances/:instance_id/connect",
            post(service::store_connect_service),
        )
        .route("/instances/:instance_id/auth", get(auth::store_auth_status))
        .route(
            "/instances/:instance_id/auth/start",
            post(auth::store_auth_start),
        )
        .route(
            "/instances/:instance_id/auth/callback",
            get(auth::store_auth_callback_get).post(auth::store_auth_callback_post),
        )
        .route(
            "/instances/:instance_id/auth/refresh",
            post(auth::store_auth_refresh),
        )
        .route(
            "/instances/:instance_id/auth/logout",
            post(auth::store_auth_logout),
        )
        .route(
            "/instances/:instance_id/auth/client-secret",
            post(auth::store_auth_save_client_secret),
        )
        .route(
            "/instances/:instance_id/auth/private-key",
            post(auth::store_auth_save_private_key),
        )
        .route(
            "/instances/:instance_id/auth/scope-upgrade",
            post(auth::store_auth_scope_upgrade),
        )
        .route(
            "/instances/:instance_id/disconnect",
            post(service::store_disconnect_service),
        )
        .route(
            "/instances/:instance_id/restart",
            post(service::store_restart_service),
        )
        .route(
            "/instances/:instance_id/wait",
            get(service::store_wait_service),
        )
        .route(
            "/instances/:instance_id/tools",
            get(service::store_list_tools),
        )
        .route(
            "/instances/:instance_id/tool-policy",
            get(service::store_get_tool_policy)
                .put(service::store_set_tool_policy)
                .delete(service::store_clear_tool_policy),
        )
        .route(
            "/instances/:instance_id/call",
            post(service::store_call_tool),
        )
        .route("/tool_transforms", get(service::store_list_tool_transforms))
        .route(
            "/instances/:instance_id/tool_transforms/:tool_name",
            get(service::store_get_tool_transform_by_path)
                .put(service::store_set_tool_transform_by_path)
                .delete(service::store_delete_tool_transform_by_path),
        )
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
        .route(
            "/instances/:instance_id/resources",
            get(service::store_list_resources),
        )
        .route(
            "/instances/:instance_id/resource_templates",
            get(service::store_list_resource_templates),
        )
        .route(
            "/instances/:instance_id/read_resource",
            get(service::store_read_resource),
        )
        .route(
            "/instances/:instance_id/prompts",
            get(service::store_list_prompts),
        )
        .route(
            "/instances/:instance_id/get_prompt",
            post(service::store_get_prompt),
        )
        .route(
            "/instances/:instance_id/completions",
            post(service::store_complete_argument),
        )
        .route(
            "/instances/:instance_id/resources/subscribe",
            post(service::store_subscribe_resource),
        )
        .route(
            "/instances/:instance_id/resources/unsubscribe",
            post(service::store_unsubscribe_resource),
        )
        .route(
            "/instances/:instance_id/logging/level",
            post(service::store_set_logging_level),
        )
        .route(
            "/instances/:instance_id/check",
            get(service::store_check_service),
        )
        .route("/instances/:instance_id", get(service::store_service_info))
        .route(
            "/instances/:instance_id/state",
            get(service::store_service_state),
        )
        .route("/config", get(service::store_show_config))
        .route(
            "/client-config/inspect",
            post(client::client_config_inspect),
        )
        .route("/client-config/plan", post(client::client_config_plan))
        .route("/client-config/apply", post(client::client_config_apply))
        .route("/client-config/undo", post(client::client_config_undo))
        .route("/client-config/import", post(client::client_config_import))
        .route("/aggregate/launch", get(client::aggregate_launch))
        .route("/config/reset", post(service::store_reset_config))
        .route(
            "/scopes/agents/:agent_id/config",
            get(service::agent_show_config),
        )
        .route(
            "/scopes/agents/:agent_id/reset",
            post(service::agent_reset_config),
        )
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
