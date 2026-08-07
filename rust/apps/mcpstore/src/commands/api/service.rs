use super::*;

use axum::extract::{Path, Query, State};
use axum::Json;
use mcpstore::config_formats::ConfigFormat;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    envelope::{success, ApiError, ApiResult},
    ApiState,
};

// ===== 请求体/查询结构 =====

/// `GET /services/{name}/tools/list?scope=&filter=` —— 作用域 + 工具过滤。
#[derive(Deserialize)]
pub(super) struct ServiceToolsListQuery {
    scope: Option<String>,
    agent_id: Option<String>,
    filter: Option<String>,
}

/// `GET /services/{name}/wait?scope=&timeout=`。
#[derive(Deserialize)]
pub(super) struct ServiceWaitQuery {
    scope: Option<String>,
    agent_id: Option<String>,
    timeout: Option<u64>,
}

/// `GET /services/{name}/resources/read?scope=&uri=`。
#[derive(Deserialize)]
pub(super) struct ServiceReadResourceQuery {
    scope: Option<String>,
    agent_id: Option<String>,
    uri: String,
}

/// `GET /tools/list?service_name=&scope=&filter=` —— 顶层工具列表。
#[derive(Deserialize)]
pub(super) struct ToolsListQuery {
    service_name: String,
    scope: Option<String>,
    agent_id: Option<String>,
    filter: Option<String>,
}

// —— 工具策略 / 配置查询结构 ——
#[derive(Deserialize)]
pub(super) struct ToolVisibilityRequest {
    available_tools: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct ShowConfigQuery {
    format: Option<String>,
    service_name: Option<String>,
    scope: Option<String>,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ResourceSubscriptionRequest {
    uri: String,
}

// ===== 工具过滤解析 =====

fn parse_tool_filter(value: &str) -> ApiResult<mcpstore::ToolVisibilityFilter> {
    match value {
        "all" => Ok(mcpstore::ToolVisibilityFilter::All),
        "available" => Ok(mcpstore::ToolVisibilityFilter::Available),
        "removed" => Ok(mcpstore::ToolVisibilityFilter::Removed),
        other => Err(ApiError::invalid_parameter(
            format!("不支持的工具过滤器: {other}"),
            Some("filter"),
        )),
    }
}

fn tool_filter_label(filter: mcpstore::ToolVisibilityFilter) -> &'static str {
    match filter {
        mcpstore::ToolVisibilityFilter::All => "all",
        mcpstore::ToolVisibilityFilter::Available => "available",
        mcpstore::ToolVisibilityFilter::Removed => "removed",
    }
}

fn parse_config_format(value: Option<&str>) -> Result<ConfigFormat, ApiError> {
    value
        .unwrap_or("native")
        .parse()
        .map_err(ApiError::from_store)
}

// ===== 列表类 =====

/// `GET /services/list?scope=store|agent&agent_id=` —— 合并 store/agent 作用域的服务列表。
pub(super) async fn service_list_services(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let services = state
        .store
        .list_services_scoped(&scope)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

/// `GET /scopes/list` —— 作用域注册表（root + store + 各 agent），每项带服务数。
pub(super) async fn scopes_list(State(state): State<Arc<ApiState>>) -> ApiResult {
    let scopes = state
        .store
        .list_scopes()
        .await
        .map_err(ApiError::from_store)?;
    let total = scopes.len();
    Ok(success(
        "作用域列表获取成功",
        json!({ "scopes": scopes, "total": total }),
    ))
}

/// `GET /agents/list` —— agent 列表（保留）。
pub(super) async fn list_agents(State(state): State<Arc<ApiState>>) -> ApiResult {
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

// ===== 服务实例：信息 / 状态 / 生命周期（服务名 + scope 寻址）=====

pub(super) async fn service_info(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let service = state
        .store
        .service_info_scoped(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务信息获取成功", service))
}

pub(super) async fn service_state(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let service_state = state
        .store
        .service_state(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务状态获取成功", service_state))
}

pub(super) async fn service_connect(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    state
        .store
        .connect_service(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务连接成功", json!({ "status": "ok" })))
}

pub(super) async fn service_disconnect(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    state
        .store
        .disconnect_service(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务断开成功", json!({ "status": "ok" })))
}

pub(super) async fn service_restart(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    state
        .store
        .restart_service(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务重启成功", json!({ "status": "ok" })))
}

pub(super) async fn service_wait(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ServiceWaitQuery>,
) -> ApiResult {
    let scope = parse_scope_ref(query.scope.as_deref(), query.agent_id.as_deref())?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let timeout = query.timeout.unwrap_or(10);
    let status = state
        .store
        .wait_instance_ready(instance_id, std::time::Duration::from_secs(timeout))
        .await
        .map_err(ApiError::from_store)?;
    let status = serde_json::to_value(status)
        .map_err(|error| ApiError::invalid_request(format!("服务状态序列化失败: {error}")))?;
    Ok(success("服务等待完成", status))
}

pub(super) async fn service_check(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let result = state
        .store
        .health_check(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务检查完成", json!(result)))
}

// ===== 工具 =====

/// `GET /services/{name}/tools/list?scope=&filter=`
pub(super) async fn service_list_tools(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ServiceToolsListQuery>,
) -> ApiResult {
    let scope = parse_scope_ref(query.scope.as_deref(), query.agent_id.as_deref())?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let filter = parse_tool_filter(query.filter.as_deref().unwrap_or("available"))?;
    let filter_name = tool_filter_label(filter);
    let tools = state
        .store
        .list_tools_for_instance_with_filter(instance_id, filter)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具列表获取成功",
        json!({ "filter": filter_name, "tools": tools, "total": tools.len() }),
    ))
}

/// `GET /tools/list?service_name=&scope=&filter=` —— 顶层入口，等价于上面的服务嵌套形态。
pub(super) async fn tools_list(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ToolsListQuery>,
) -> ApiResult {
    let scope = parse_scope_ref(query.scope.as_deref(), query.agent_id.as_deref())?;
    let service_name = query.service_name.trim();
    if service_name.is_empty() {
        return Err(ApiError::missing_parameter("service_name"));
    }
    let instance_id = resolve_instance(&state, service_name, &scope).await?;
    let filter = parse_tool_filter(query.filter.as_deref().unwrap_or("available"))?;
    let filter_name = tool_filter_label(filter);
    let tools = state
        .store
        .list_tools_for_instance_with_filter(instance_id, filter)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具列表获取成功",
        json!({
            "service_name": service_name,
            "filter": filter_name,
            "tools": tools,
            "total": tools.len(),
        }),
    ))
}

/// `POST /services/{name}/tools/call` —— body: `{ tool_name, args? }`
pub(super) async fn service_call_tool(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let tool_name = extract_tool_name(&payload)?;
    let args = extract_tool_args(&payload)?;
    let result = state
        .store
        .call_tool(instance_id, &tool_name, args)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具调用完成",
        serde_json::to_value(result).unwrap_or(Value::Null),
    ))
}

/// `POST /tools/call` —— body: `{ service_name|service, scope?, agent_id?, tool_name, args? }`
pub(super) async fn tools_call(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let service_name = payload
        .get("service_name")
        .and_then(Value::as_str)
        .or_else(|| payload.get("service").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::missing_parameter("service_name"))?;
    let scope = parse_scope_ref(
        payload.get("scope").and_then(Value::as_str),
        payload.get("agent_id").and_then(Value::as_str),
    )?;
    let instance_id = resolve_instance(&state, service_name, &scope).await?;
    let tool_name = extract_tool_name(&payload)?;
    let args = extract_tool_args(&payload)?;
    let result = state
        .store
        .call_tool(instance_id, &tool_name, args)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具调用完成",
        serde_json::to_value(result).unwrap_or(Value::Null),
    ))
}

// ===== 资源 / Prompt =====

/// `GET /services/{name}/resources/list?scope=`
pub(super) async fn service_list_resources(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let resources = state
        .store
        .list_resources_for_instance(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "资源列表获取成功",
        json!({ "resources": resources, "total": resources.len() }),
    ))
}

/// `GET /services/{name}/resources/templates?scope=`
pub(super) async fn service_list_resource_templates(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let templates = state
        .store
        .list_resource_templates_for_instance(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "资源模板列表获取成功",
        json!({ "resource_templates": templates, "total": templates.len() }),
    ))
}

/// `GET /services/{name}/resources/read?scope=&uri=`
pub(super) async fn service_read_resource(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ServiceReadResourceQuery>,
) -> ApiResult {
    let scope = parse_scope_ref(query.scope.as_deref(), query.agent_id.as_deref())?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let uri = query.uri.trim();
    if uri.is_empty() {
        return Err(ApiError::missing_parameter("uri"));
    }
    let result = state
        .store
        .read_resource_scoped(instance_id, uri)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("资源读取成功", result))
}

/// `GET /services/{name}/prompts/list?scope=`
pub(super) async fn service_list_prompts(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let prompts = state
        .store
        .list_prompts_for_instance(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Prompt 列表获取成功",
        json!({ "prompts": prompts, "total": prompts.len() }),
    ))
}

/// `POST /services/{name}/prompts/get` —— body: `{ prompt_name, args? }`
pub(super) async fn service_get_prompt(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let prompt_name = extract_prompt_name(&payload)?;
    let args = extract_prompt_args(&payload)?;
    let result = state
        .store
        .get_prompt_scoped(instance_id, &prompt_name, args)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Prompt 获取成功", result))
}

// ===== 服务定义（根级 CRUD，保留）=====

pub(super) async fn add_service_definition(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let config: ServerConfig = serde_json::from_value(payload)
        .map_err(|error| ApiError::invalid_request(format!("服务配置无效: {error}")))?;
    state
        .store
        .add_service(&service_name, config)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务定义添加成功", json!({ "status": "ok" })))
}

pub(super) async fn update_service_definition(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Json(payload): Json<Value>,
) -> ApiResult {
    if payload
        .as_object()
        .is_some_and(|config| config.contains_key("_mcpstore"))
    {
        return Err(ApiError::invalid_request(
            "基础配置更新不能包含 _mcpstore；请使用作用域接口修改 scope",
        ));
    }
    let config: ServerConfig = serde_json::from_value(payload)
        .map_err(|error| ApiError::invalid_request(format!("服务配置无效: {error}")))?;
    state
        .store
        .update_service(&service_name, config)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务定义更新成功", json!({ "status": "ok" })))
}

pub(super) async fn remove_service_definition(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .remove_service(&service_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务定义删除成功", json!({ "status": "ok" })))
}

// ===== Scope 声明（保留）=====

pub(super) async fn declare_store_scope(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Json(descriptor): Json<ScopeDescriptor>,
) -> ApiResult {
    state
        .store
        .declare_service_scope(&service_name, &ScopeRef::Store, descriptor)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Store 作用域已声明", json!({ "status": "ok" })))
}

pub(super) async fn remove_store_scope(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
) -> ApiResult {
    state
        .store
        .remove_service_scope(&service_name, &ScopeRef::Store)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Store 作用域已删除", json!({ "status": "ok" })))
}

pub(super) async fn declare_agent_scope(
    State(state): State<Arc<ApiState>>,
    Path((service_name, agent_id)): Path<(String, String)>,
    Json(descriptor): Json<ScopeDescriptor>,
) -> ApiResult {
    state
        .store
        .declare_service_scope(&service_name, &ScopeRef::Agent { agent_id }, descriptor)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 作用域已声明", json!({ "status": "ok" })))
}

pub(super) async fn remove_agent_scope(
    State(state): State<Arc<ApiState>>,
    Path((service_name, agent_id)): Path<(String, String)>,
) -> ApiResult {
    state
        .store
        .remove_service_scope(&service_name, &ScopeRef::Agent { agent_id })
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 作用域已删除", json!({ "status": "ok" })))
}

// ===== 工具策略（服务名 + scope 寻址）=====

pub(super) async fn service_get_tool_policy(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let policy = state
        .store
        .get_context_tool_visibility(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具策略获取成功", json!({ "policy": policy })))
}

pub(super) async fn service_set_tool_policy(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<ToolVisibilityRequest>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let policy = state
        .store
        .set_context_tool_visibility(instance_id, payload.available_tools)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具策略更新成功", json!({ "policy": policy })))
}

pub(super) async fn service_clear_tool_policy(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    state
        .store
        .clear_context_tool_visibility(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具策略已清除", json!({ "policy": null })))
}

// ===== 工具转换规则（全局列表 + 按服务名+scope 管理）=====

pub(super) async fn store_list_tool_transforms(State(state): State<Arc<ApiState>>) -> ApiResult {
    let transforms = state
        .store
        .list_tool_transforms()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具转换规则列表获取成功",
        json!({ "transforms": transforms, "total": transforms.len() }),
    ))
}

pub(super) async fn service_get_tool_transform(
    State(state): State<Arc<ApiState>>,
    Path((service_name, tool_name)): Path<(String, String)>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let transform = state
        .store
        .get_tool_transform(instance_id, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具转换规则获取成功", json!({ "transform": transform })))
}

pub(super) async fn service_set_tool_transform(
    State(state): State<Arc<ApiState>>,
    Path((service_name, tool_name)): Path<(String, String)>,
    Query(query): Query<ScopeQuery>,
    Json(transform): Json<ToolTransformPatch>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let transform = state
        .store
        .set_tool_transform(instance_id, &tool_name, transform)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具转换规则设置成功",
        json!({ "transform": transform }),
    ))
}

pub(super) async fn service_delete_tool_transform(
    State(state): State<Arc<ApiState>>,
    Path((service_name, tool_name)): Path<(String, String)>,
    Query(query): Query<ScopeQuery>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    state
        .store
        .delete_tool_transform(instance_id, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具转换规则删除成功", json!({ "status": "ok" })))
}

// ===== 参数补全（服务名 + scope 寻址）=====

pub(super) async fn service_complete_argument(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<McpCompletionRequest>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let completion = state
        .store
        .complete_mcp_argument(instance_id, payload)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("参数补全成功", json!(completion)))
}

// ===== 资源订阅（服务名 + scope 寻址）=====

pub(super) async fn service_subscribe_resource(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<ResourceSubscriptionRequest>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let uri = payload.uri.trim();
    if uri.is_empty() {
        return Err(ApiError::invalid_parameter(
            "资源 URI 不能为空",
            Some("uri"),
        ));
    }
    state
        .store
        .subscribe_resource_updates(instance_id, uri)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("资源更新订阅成功", json!({ "uri": uri })))
}

pub(super) async fn service_unsubscribe_resource(
    State(state): State<Arc<ApiState>>,
    Path(service_name): Path<String>,
    Query(query): Query<ScopeQuery>,
    Json(payload): Json<ResourceSubscriptionRequest>,
) -> ApiResult {
    let scope = query.into_scope_ref()?;
    let instance_id = resolve_instance(&state, &service_name, &scope).await?;
    let uri = payload.uri.trim();
    if uri.is_empty() {
        return Err(ApiError::invalid_parameter(
            "资源 URI 不能为空",
            Some("uri"),
        ));
    }
    state
        .store
        .unsubscribe_resource_updates(instance_id, uri)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("资源更新订阅已取消", json!({ "uri": uri })))
}

// ===== 配置查看 / 重置（保留）=====

pub(super) async fn store_show_config(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ShowConfigQuery>,
) -> ApiResult {
    let format = parse_config_format(query.format.as_deref())?;
    let config = if format == ConfigFormat::Native {
        state.store.show_config().await
    } else {
        let service_name = query
            .service_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::missing_parameter("service_name"))?;
        let scope = parse_scope_ref(query.scope.as_deref(), query.agent_id.as_deref())?;
        let instance_id = resolve_instance(&state, service_name, &scope).await?;
        state
            .store
            .export_instance_config(instance_id, format)
            .await
    }
    .map_err(ApiError::from_store)?;
    Ok(success("配置获取成功", config))
}

pub(super) async fn agent_show_config(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
    Query(query): Query<ShowConfigQuery>,
) -> ApiResult {
    let format = parse_config_format(query.format.as_deref())?;
    let scope = ScopeRef::Agent { agent_id };
    let config = if format == ConfigFormat::Native {
        state.store.show_scope_config(&scope).await
    } else {
        let service_name = query
            .service_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::missing_parameter("service_name"))?;
        let instance_id = resolve_instance(&state, service_name, &scope).await?;
        state
            .store
            .export_instance_config(instance_id, format)
            .await
    }
    .map_err(ApiError::from_store)?;
    Ok(success("Agent 配置获取成功", config))
}

pub(super) async fn agent_reset_config(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
) -> ApiResult {
    state
        .store
        .reset_scope(&ScopeRef::Agent { agent_id })
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Agent 配置重置成功", json!({ "status": "ok" })))
}

pub(super) async fn store_reset_config(State(state): State<Arc<ApiState>>) -> ApiResult {
    state
        .store
        .reset_config()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("配置重置成功", json!({ "status": "ok" })))
}
