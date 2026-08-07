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
    client_config::{
        apply_config_change, import_selected_services, inspect_client_config, plan_add_entries,
        ClientConfigInspection, ClientEntryKind, ClientEntryPlan, ClientEntrySpec,
        ClientEntryStatus, ClientKind, ConfigChangeReceipt,
    },
    config::ScopeDescriptor,
    AuthFlow, InstanceId, MCPStore, McpCompletionRequest, OpenApiBundleOptions,
    OpenApiImportOptions, OpenApiRefCachePolicy, ScopeRef, ScopeView, ServerConfig,
    ToolOverridePatch,
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
    extract_prompt_args, extract_prompt_name, extract_tool_args, extract_tool_name, normalize_prefix,
    parse_scope_ref, ScopeQuery,
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
    client_changes: Arc<Mutex<HashMap<String, ConfigChangeReceipt>>>,
    aggregate_process: Arc<Mutex<Option<AggregateProcess>>>,
}

struct AggregateProcess {
    child: Child,
    descriptor: McpServerLaunchDescriptor,
}

impl Drop for AggregateProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// 把 `(service_name, scope)` 解析成 instance_id；服务未在该 scope 声明时返回 404。
/// 新版 URL 以「服务名 + 作用域」寻址，instance_id 只在 API 层内部流转，不再暴露给用户。
async fn resolve_instance(
    state: &ApiState,
    service_name: &str,
    scope: &ScopeRef,
) -> ApiResult<InstanceId> {
    state
        .store
        .instance_id_for_scope(service_name, scope)
        .await
        .map_err(|error| match error {
            mcpstore::StoreError::Other(_) => ApiError::not_found(
                "SERVICE_SCOPE_NOT_FOUND",
                format!("服务 {service_name} 未在该作用域声明"),
                Some("service_name"),
                Some(json!({
                    "service_name": service_name,
                    "scope": serde_json::to_value(scope).unwrap_or(serde_json::Value::Null),
                })),
            ),
            other => ApiError::from_store(other),
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
        client_changes: Arc::new(Mutex::new(HashMap::new())),
        aggregate_process: Arc::new(Mutex::new(None)),
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
        // ===== app：应用配置 / 元信息 / 历史（app 专用，非 core）=====
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
        // ===== services：服务定义（根级 CRUD）+ 服务信息 =====
        .route("/services/list", get(service::service_list_services))
        .route(
            "/services/:service_name",
            get(service::service_info)
                .post(service::add_service_definition)
                .put(service::update_service_definition)
                .delete(service::remove_service_definition),
        )
        // ===== services：生命周期 / 状态（服务名 + scope 寻址）=====
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
        // ===== tools：服务嵌套形态 + 顶层形态 =====
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
        // ===== resources / prompts（服务名 + scope 寻址）=====
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
        // ===== services：scope 声明 =====
        .route(
            "/services/:service_name/scopes/store",
            put(service::declare_store_scope).delete(service::remove_store_scope),
        )
        .route(
            "/services/:service_name/scopes/agents/:agent_id",
            put(service::declare_agent_scope).delete(service::remove_agent_scope),
        )
        // ===== sessions（已折叠为单形态：session_key 走 query/body）=====
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
        .route("/sessions/bind_service", post(session::session_bind_service))
        .route(
            "/sessions/unbind_service",
            post(session::session_unbind_service),
        )
        .route("/sessions/list_services", get(session::session_list_services))
        .route("/sessions/list_tools", get(session::session_list_tools))
        .route("/sessions/call_tool", post(session::session_call_tool))
        .route("/sessions/state/list", get(session::session_list_state))
        .route("/sessions/state/value", get(session::session_get_state_value))
        .route("/sessions/state/set", post(session::session_set_state))
        .route("/sessions/state/delete", post(session::session_delete_state))
        .route("/sessions/state/clear", post(session::session_clear_state))
        // ===== 工具策略 / 转换规则 / 补全 / 资源订阅（服务名 + scope 寻址）=====
        .route(
            "/services/:service_name/tool-policy",
            get(service::service_get_tool_policy)
                .put(service::service_set_tool_policy)
                .delete(service::service_clear_tool_policy),
        )
        .route("/tool_transforms", get(service::store_list_tool_overrides))
        .route(
            "/services/:service_name/tool_transforms/:tool_name",
            get(service::service_get_tool_override)
                .put(service::service_set_tool_override)
                .delete(service::service_delete_tool_override),
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
        // ===== OAuth 认证（服务名 + scope 寻址）=====
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
        // ===== OpenAPI 导入（保留）=====
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
        // ===== 配置 / 编程助手 / 聚合 / 缓存（app 专用，非 core）=====
        .route("/config", get(service::store_show_config))
        .route("/config/reset", post(service::store_reset_config))
        .route(
            "/client-config/inspect",
            post(client::client_config_inspect),
        )
        .route("/client-config/plan", post(client::client_config_plan))
        .route("/client-config/apply", post(client::client_config_apply))
        .route("/client-config/undo", post(client::client_config_undo))
        .route("/client-config/import", post(client::client_config_import))
        .route("/aggregate/launch", get(client::aggregate_launch))
        .route("/aggregate/status", get(client::aggregate_status))
        .route("/aggregate/start", post(client::aggregate_start))
        .route("/aggregate/stop", post(client::aggregate_stop))
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
