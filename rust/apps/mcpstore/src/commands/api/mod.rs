use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use clap::Args;
use mcpstore::{perspective::GLOBAL_AGENT_STORE, MCPStore};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    store_args::{build_store, StoreSourceArgs},
    BoxErr,
};

mod envelope;
mod parse;

use envelope::{success, ApiError, ApiResult};
use parse::{
    cache_storage_label, ensure_json_object, extract_prompt_args, extract_prompt_name,
    extract_resource_uri, extract_tool_args, extract_tool_name, normalize_prefix,
    parse_cache_storage, parse_named_service_payload, parse_positive_u64, parse_positive_usize,
};

#[derive(Args)]
pub struct ApiArgs {
    #[arg(long, default_value_t = 18200, help = "API 服务端口")]
    pub port: u16,
    #[arg(long, default_value = "127.0.0.1", help = "绑定地址")]
    pub host: String,
    #[arg(long, default_value = "", help = "URL 前缀，例如 /mcp")]
    pub url_prefix: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Clone)]
struct ApiState {
    store: Arc<MCPStore>,
}

#[derive(Deserialize)]
struct CacheSwitchRequest {
    backend: String,
    redis_url: Option<String>,
    namespace: Option<String>,
}

pub async fn run(args: ApiArgs) -> Result<(), BoxErr> {
    let store = build_store(&args.store)?;
    store.load_from_source().await?;

    let prefix = normalize_prefix(&args.url_prefix);
    let state = Arc::new(ApiState {
        store: Arc::new(store),
    });
    if !state.store.is_db_source() {
        spawn_control_request_worker(state.store.clone());
    }
    let app = router(state, &prefix);

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

fn spawn_control_request_worker(store: Arc<MCPStore>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            match store.process_control_requests().await {
                Ok(processed) if processed > 0 => {
                    tracing::info!("[API] Processed {processed} queued control request(s)");
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!("[API] Control request processing failed: {error}");
                }
            }
        }
    });
}

fn router(state: Arc<ApiState>, prefix: &str) -> Router {
    let base = Router::new()
        .route("/health", get(health))
        .route("/agents/list", get(list_agents))
        .route("/events/history", get(event_history))
        .route("/events/capability_report", get(event_capability_report))
        .route("/for_store/list_services", get(store_list_services))
        .route("/for_store/add_service", post(store_add_service))
        .route(
            "/for_store/update_service/:service_name",
            post(store_update_service),
        )
        .route(
            "/for_store/remove_service/:service_name",
            post(store_remove_service),
        )
        .route(
            "/for_store/connect_service/:service_name",
            post(store_connect_service),
        )
        .route(
            "/for_store/disconnect_service/:service_name",
            post(store_disconnect_service),
        )
        .route(
            "/for_store/restart_service/:service_name",
            post(store_restart_service),
        )
        .route(
            "/for_store/wait_service/:service_name",
            get(store_wait_service),
        )
        .route("/for_store/list_tools", get(store_list_tools))
        .route("/for_store/call_tool", post(store_call_tool))
        .route("/for_store/list_resources", get(store_list_resources))
        .route(
            "/for_store/list_resource_templates",
            get(store_list_resource_templates),
        )
        .route("/for_store/read_resource", get(store_read_resource))
        .route("/for_store/list_prompts", get(store_list_prompts))
        .route("/for_store/get_prompt", post(store_get_prompt))
        .route("/for_store/check_services", get(store_check_services))
        .route(
            "/for_store/service_info/:service_name",
            get(store_service_info),
        )
        .route(
            "/for_store/service_status/:service_name",
            get(store_service_status),
        )
        .route("/for_store/show_config", get(store_show_config))
        .route("/for_store/reset_config", post(store_reset_config))
        .route(
            "/for_agent/:agent_id/list_services",
            get(agent_list_services),
        )
        .route("/for_agent/:agent_id/add_service", post(agent_add_service))
        .route(
            "/for_agent/:agent_id/update_service/:service_name",
            post(agent_update_service),
        )
        .route(
            "/for_agent/:agent_id/remove_service/:service_name",
            post(agent_remove_service),
        )
        .route(
            "/for_agent/:agent_id/connect_service/:service_name",
            post(agent_connect_service),
        )
        .route(
            "/for_agent/:agent_id/disconnect_service/:service_name",
            post(agent_disconnect_service),
        )
        .route(
            "/for_agent/:agent_id/restart_service/:service_name",
            post(agent_restart_service),
        )
        .route(
            "/for_agent/:agent_id/wait_service/:service_name",
            get(agent_wait_service),
        )
        .route(
            "/for_agent/:agent_id/assign_service/:service_name",
            post(agent_assign_service),
        )
        .route(
            "/for_agent/:agent_id/unassign_service/:service_name",
            post(agent_unassign_service),
        )
        .route("/for_agent/:agent_id/list_tools", get(agent_list_tools))
        .route("/for_agent/:agent_id/call_tool", post(agent_call_tool))
        .route(
            "/for_agent/:agent_id/list_resources",
            get(agent_list_resources),
        )
        .route(
            "/for_agent/:agent_id/list_resource_templates",
            get(agent_list_resource_templates),
        )
        .route(
            "/for_agent/:agent_id/read_resource",
            get(agent_read_resource),
        )
        .route("/for_agent/:agent_id/list_prompts", get(agent_list_prompts))
        .route("/for_agent/:agent_id/get_prompt", post(agent_get_prompt))
        .route(
            "/for_agent/:agent_id/service_info/:service_name",
            get(agent_service_info),
        )
        .route(
            "/for_agent/:agent_id/service_status/:service_name",
            get(agent_service_status),
        )
        .route("/for_agent/:agent_id/show_config", get(agent_show_config))
        .route("/cache/health", get(cache_health))
        .route("/cache/inspect", get(cache_inspect))
        .route("/cache/switch", post(cache_switch))
        .with_state(state);

    if prefix.is_empty() {
        base
    } else {
        Router::new().nest(prefix, base)
    }
}

async fn health(State(state): State<Arc<ApiState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "backend": cache_storage_label(state.store.current_cache_storage().await),
    }))
}

async fn list_agents(State(state): State<Arc<ApiState>>) -> ApiResult {
    let agents = state
        .store
        .list_agents()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 列表获取成功",
        json!({ "agents": agents, "total": agents.len() }),
    ))
}

async fn event_history(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let count = params
        .get("count")
        .map(String::as_str)
        .map(parse_positive_usize)
        .transpose()?
        .unwrap_or(100);
    let events = state.store.event_history(count).await;
    Ok(success(
        "事件历史获取成功",
        json!({ "events": events, "total": events.len() }),
    ))
}

async fn event_capability_report(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state.store.event_capability_report().await;
    Ok(success("事件能力报告获取成功", report))
}

async fn store_list_services(State(state): State<Arc<ApiState>>) -> ApiResult {
    let services = state
        .store
        .list_services_scoped(None)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

async fn store_add_service(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Value>,
) -> ApiResult {
    for (name, config) in parse_named_service_payload(payload)? {
        state
            .store
            .add_service(&name, config)
            .await
            .map_err(ApiError::from_store)?;
    }
    Ok(success("服务添加成功", json!({ "status": "ok" })))
}

async fn store_update_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let updates = ensure_json_object(payload, "payload")?;
    state
        .store
        .patch_service(&service_name, updates)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务更新成功", json!({ "status": "ok" })))
}

async fn store_remove_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .remove_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务删除成功", json!({ "status": "ok" })))
}

async fn store_connect_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .connect_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务连接成功", json!({ "status": "ok" })))
}

async fn store_disconnect_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .disconnect_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务断开成功", json!({ "status": "ok" })))
}

async fn store_restart_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .restart_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务重启成功", json!({ "status": "ok" })))
}

async fn store_wait_service(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let timeout = params
        .get("timeout")
        .map(String::as_str)
        .map(parse_positive_u64)
        .transpose()?
        .unwrap_or(10);
    let status = state
        .store
        .wait_service_ready(&service_name, timeout)
        .await
        .map_err(ApiError::from_store)?;
    let status = serde_json::to_value(status)
        .map_err(|error| ApiError::invalid_request(format!("服务状态序列化失败: {error}")))?;
    Ok(success("服务等待完成", status))
}

async fn store_list_tools(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let tools = state
        .store
        .list_tools_scoped(None, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具列表获取成功",
        json!({ "tools": tools, "total": tools.len() }),
    ))
}

async fn store_call_tool(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let tool_name = extract_tool_name(&payload)?;
    let args = extract_tool_args(&payload)?;
    let resolution = state
        .store
        .resolve_tool_for_agent(GLOBAL_AGENT_STORE, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    let result = state
        .store
        .call_tool(
            &resolution.global_service_name,
            &resolution.canonical_tool_name,
            args,
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具调用完成",
        serde_json::to_value(result).unwrap_or(Value::Null),
    ))
}

async fn store_list_resources(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let resources = state
        .store
        .list_resources_scoped(None, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "资源列表获取成功",
        json!({ "resources": resources, "total": resources.len() }),
    ))
}

async fn store_list_resource_templates(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let templates = state
        .store
        .list_resource_templates_scoped(None, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "资源模板列表获取成功",
        json!({ "resource_templates": templates, "total": templates.len() }),
    ))
}

async fn store_read_resource(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let uri = extract_resource_uri(&params)?;
    let result = state
        .store
        .read_resource_scoped(None, &uri, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("资源读取成功", result))
}

async fn store_list_prompts(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let prompts = state
        .store
        .list_prompts_scoped(None, params.get("service_name").map(String::as_str))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Prompt 列表获取成功",
        json!({ "prompts": prompts, "total": prompts.len() }),
    ))
}

async fn store_get_prompt(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let prompt_name = extract_prompt_name(&payload)?;
    let args = extract_prompt_args(&payload)?;
    let service_name = payload.get("service_name").and_then(Value::as_str);
    let result = state
        .store
        .get_prompt_scoped(None, &prompt_name, args, service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Prompt 获取成功", result))
}

async fn store_check_services(State(state): State<Arc<ApiState>>) -> ApiResult {
    let result = state
        .store
        .check_services_scoped(None)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务检查完成", result))
}

async fn store_service_info(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    let service = state
        .store
        .service_info_scoped(None, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务信息获取成功", service))
}

async fn store_service_status(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    let status = state
        .store
        .service_status_scoped(None, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务状态获取成功", status))
}

async fn store_show_config(State(state): State<Arc<ApiState>>) -> ApiResult {
    let config = state
        .store
        .show_config()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("配置获取成功", config))
}

async fn store_reset_config(State(state): State<Arc<ApiState>>) -> ApiResult {
    state
        .store
        .reset_config()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("配置重置成功", json!({ "status": "ok" })))
}

async fn agent_list_services(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
) -> ApiResult {
    let services = state
        .store
        .list_services_scoped(Some(&agent_id))
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

async fn agent_add_service(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    for (name, config) in parse_named_service_payload(payload)? {
        state
            .store
            .add_service_for_agent(&agent_id, &name, config)
            .await
            .map_err(ApiError::from_store)?;
    }
    Ok(success("Agent 服务添加成功", json!({ "status": "ok" })))
}

async fn agent_update_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    let updates = ensure_json_object(payload, "payload")?;
    state
        .store
        .patch_service(&global_service_name, updates)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务更新成功", json!({ "status": "ok" })))
}

async fn agent_remove_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    state
        .store
        .remove_service(&global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务删除成功", json!({ "status": "ok" })))
}

async fn agent_connect_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    state
        .store
        .connect_service(&global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务连接成功", json!({ "status": "ok" })))
}

async fn agent_disconnect_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    state
        .store
        .disconnect_service(&global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务断开成功", json!({ "status": "ok" })))
}

async fn agent_restart_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    state
        .store
        .restart_service(&global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务重启成功", json!({ "status": "ok" })))
}

async fn agent_wait_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    let timeout = params
        .get("timeout")
        .map(String::as_str)
        .map(parse_positive_u64)
        .transpose()?
        .unwrap_or(10);
    let status = state
        .store
        .wait_service_ready(&global_service_name, timeout)
        .await
        .map_err(ApiError::from_store)?;
    let status = serde_json::to_value(status)
        .map_err(|error| ApiError::invalid_request(format!("服务状态序列化失败: {error}")))?;
    Ok(success("Agent 服务等待完成", status))
}

async fn agent_assign_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    state
        .store
        .assign_service_to_agent(&agent_id, &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务授权成功", json!({ "status": "ok" })))
}

async fn agent_unassign_service(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let global_service_name = state
        .store
        .resolve_service_name_for_agent(&agent_id, &service_name)
        .await
        .unwrap_or(service_name);
    state
        .store
        .unassign_service_from_agent(&agent_id, &global_service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务解除授权成功", json!({ "status": "ok" })))
}

async fn agent_list_tools(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let tools = state
        .store
        .list_tools_scoped(
            Some(&agent_id),
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 工具列表获取成功",
        json!({ "tools": tools, "total": tools.len() }),
    ))
}

async fn agent_call_tool(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let tool_name = extract_tool_name(&payload)?;
    let args = extract_tool_args(&payload)?;
    let resolution = state
        .store
        .resolve_tool_for_agent(&agent_id, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    let result = state
        .store
        .call_tool(
            &resolution.global_service_name,
            &resolution.canonical_tool_name,
            args,
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 工具调用完成",
        serde_json::to_value(result).unwrap_or(Value::Null),
    ))
}

async fn agent_list_resources(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let resources = state
        .store
        .list_resources_scoped(
            Some(&agent_id),
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 资源列表获取成功",
        json!({ "resources": resources, "total": resources.len() }),
    ))
}

async fn agent_list_resource_templates(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let templates = state
        .store
        .list_resource_templates_scoped(
            Some(&agent_id),
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 资源模板列表获取成功",
        json!({ "resource_templates": templates, "total": templates.len() }),
    ))
}

async fn agent_read_resource(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let uri = extract_resource_uri(&params)?;
    let result = state
        .store
        .read_resource_scoped(
            Some(&agent_id),
            &uri,
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 资源读取成功", result))
}

async fn agent_list_prompts(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let prompts = state
        .store
        .list_prompts_scoped(
            Some(&agent_id),
            params.get("service_name").map(String::as_str),
        )
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent Prompt 列表获取成功",
        json!({ "prompts": prompts, "total": prompts.len() }),
    ))
}

async fn agent_get_prompt(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let prompt_name = extract_prompt_name(&payload)?;
    let args = extract_prompt_args(&payload)?;
    let service_name = payload.get("service_name").and_then(Value::as_str);
    let result = state
        .store
        .get_prompt_scoped(Some(&agent_id), &prompt_name, args, service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent Prompt 获取成功", result))
}

async fn agent_service_info(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let service = state
        .store
        .service_info_scoped(Some(&agent_id), &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务信息获取成功", service))
}

async fn agent_service_status(
    State(state): State<Arc<ApiState>>,
    Path((agent_id, service_name)): Path<(String, String)>,
) -> ApiResult {
    let status = state
        .store
        .service_status_scoped(Some(&agent_id), &service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 服务状态获取成功", status))
}

async fn agent_show_config(
    State(state): State<Arc<ApiState>>,
    Path(_agent_id): Path<String>,
) -> ApiResult {
    let config = state
        .store
        .show_config()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 配置获取成功", config))
}

async fn cache_inspect(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .cache_inspect()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("缓存视图获取成功", report))
}

async fn cache_health(State(state): State<Arc<ApiState>>) -> ApiResult {
    let report = state
        .store
        .cache_health_check()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("缓存健康检查成功", report))
}

async fn cache_switch(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<CacheSwitchRequest>,
) -> ApiResult {
    let cache_storage = parse_cache_storage(&payload.backend)?;
    let snapshot = state
        .store
        .switch_cache_storage(cache_storage, payload.redis_url, payload.namespace)
        .await
        .map_err(ApiError::from_store)?;
    let snapshot = serde_json::to_value(snapshot)
        .map_err(|error| ApiError::invalid_request(format!("缓存切换结果序列化失败: {error}")))?;
    Ok(success("缓存后端切换成功", snapshot))
}
