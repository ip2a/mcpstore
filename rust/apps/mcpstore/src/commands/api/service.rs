use std::sync::Arc;

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

#[derive(Deserialize)]
pub(super) struct ToolListQuery {
    filter: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ToolVisibilityRequest {
    available_tools: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct ShowConfigQuery {
    format: Option<String>,
    instance_id: Option<InstanceId>,
}

#[derive(Deserialize)]
pub(super) struct ResourceSubscriptionRequest {
    uri: String,
}

#[derive(Deserialize)]
pub(super) struct LoggingLevelRequest {
    level: McpLoggingLevel,
}

pub(super) async fn store_list_services(State(state): State<Arc<ApiState>>) -> ApiResult {
    let services = state
        .store
        .list_services_scoped(&ScopeRef::Store)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
}

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

fn parse_config_format(value: Option<&str>) -> Result<ConfigFormat, ApiError> {
    value
        .unwrap_or("native")
        .parse()
        .map_err(ApiError::from_store)
}

pub(super) async fn store_connect_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .connect_service(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务连接成功", json!({ "status": "ok" })))
}

pub(super) async fn store_disconnect_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .disconnect_service(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务断开成功", json!({ "status": "ok" })))
}

pub(super) async fn store_restart_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .restart_service(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务重启成功", json!({ "status": "ok" })))
}

pub(super) async fn store_wait_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
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
        .wait_instance_ready(instance_id, timeout)
        .await
        .map_err(ApiError::from_store)?;
    let status = serde_json::to_value(status)
        .map_err(|error| ApiError::invalid_request(format!("服务状态序列化失败: {error}")))?;
    Ok(success("服务等待完成", status))
}

pub(super) async fn store_list_tools(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Query(query): Query<ToolListQuery>,
) -> ApiResult {
    let filter = match query.filter.as_deref().unwrap_or("available") {
        "all" => mcpstore::ToolVisibilityFilter::All,
        "available" => mcpstore::ToolVisibilityFilter::Available,
        "removed" => mcpstore::ToolVisibilityFilter::Removed,
        value => {
            return Err(ApiError::invalid_parameter(
                format!("不支持的工具过滤器: {value}"),
                Some("filter"),
            ));
        }
    };
    let filter_name = match filter {
        mcpstore::ToolVisibilityFilter::All => "all",
        mcpstore::ToolVisibilityFilter::Available => "available",
        mcpstore::ToolVisibilityFilter::Removed => "removed",
    };
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

pub(super) async fn store_get_tool_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let policy = state
        .store
        .get_context_tool_visibility(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具策略获取成功", json!({ "policy": policy })))
}

pub(super) async fn store_set_tool_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<ToolVisibilityRequest>,
) -> ApiResult {
    let policy = state
        .store
        .set_context_tool_visibility(instance_id, payload.available_tools)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具策略更新成功", json!({ "policy": policy })))
}

pub(super) async fn store_clear_tool_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    state
        .store
        .clear_context_tool_visibility(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具策略已清除", json!({ "policy": null })))
}

pub(super) async fn store_call_tool(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<Value>,
) -> ApiResult {
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

pub(super) async fn store_get_tool_transform_by_path(
    State(state): State<Arc<ApiState>>,
    Path((instance_id, tool_name)): Path<(InstanceId, String)>,
) -> ApiResult {
    let transform = state
        .store
        .get_tool_transform(instance_id, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "工具转换规则获取成功",
        json!({ "transform": transform }),
    ))
}

pub(super) async fn store_set_tool_transform_by_path(
    State(state): State<Arc<ApiState>>,
    Path((instance_id, tool_name)): Path<(InstanceId, String)>,
    Json(transform): Json<ToolTransformPatch>,
) -> ApiResult {
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

pub(super) async fn store_delete_tool_transform_by_path(
    State(state): State<Arc<ApiState>>,
    Path((instance_id, tool_name)): Path<(InstanceId, String)>,
) -> ApiResult {
    state
        .store
        .delete_tool_transform(instance_id, &tool_name)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("工具转换规则删除成功", json!({ "status": "ok" })))
}

pub(super) async fn store_list_resources(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
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

pub(super) async fn store_list_resource_templates(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
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

pub(super) async fn store_read_resource(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let uri = extract_resource_uri(&params)?;
    let result = state
        .store
        .read_resource_scoped(instance_id, &uri)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("资源读取成功", result))
}

pub(super) async fn store_list_prompts(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
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

pub(super) async fn store_get_prompt(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<Value>,
) -> ApiResult {
    let prompt_name = extract_prompt_name(&payload)?;
    let args = extract_prompt_args(&payload)?;
    let result = state
        .store
        .get_prompt_scoped(instance_id, &prompt_name, args)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("Prompt 获取成功", result))
}

pub(super) async fn store_complete_argument(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<McpCompletionRequest>,
) -> ApiResult {
    let completion = state
        .store
        .complete_mcp_argument(instance_id, payload)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("参数补全成功", json!(completion)))
}

pub(super) async fn store_subscribe_resource(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<ResourceSubscriptionRequest>,
) -> ApiResult {
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

pub(super) async fn store_unsubscribe_resource(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<ResourceSubscriptionRequest>,
) -> ApiResult {
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

pub(super) async fn store_set_logging_level(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
    Json(payload): Json<LoggingLevelRequest>,
) -> ApiResult {
    state
        .store
        .set_mcp_logging_level(instance_id, payload.level)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "远端日志级别设置成功",
        json!({ "level": payload.level }),
    ))
}

pub(super) async fn store_check_service(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let result = state
        .store
        .health_check(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务检查完成", json!(result)))
}

pub(super) async fn store_service_info(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let service = state
        .store
        .service_info_scoped(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务信息获取成功", service))
}

pub(super) async fn store_service_state(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<InstanceId>,
) -> ApiResult {
    let service_state = state
        .store
        .service_state(instance_id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("服务状态获取成功", service_state))
}

pub(super) async fn store_show_config(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ShowConfigQuery>,
) -> ApiResult {
    let format = parse_config_format(query.format.as_deref())?;
    let config = if format == ConfigFormat::Native {
        state.store.show_config().await
    } else {
        let instance_id = query
            .instance_id
            .ok_or_else(|| ApiError::missing_parameter("instance_id"))?;
        state
            .store
            .export_instance_config(instance_id, format)
            .await
    }
    .map_err(ApiError::from_store)?;
    Ok(success("配置获取成功", config))
}

pub(super) async fn agent_list_services(
    State(state): State<Arc<ApiState>>,
    Path(agent_id): Path<String>,
) -> ApiResult {
    let services = state
        .store
        .list_services_scoped(&ScopeRef::Agent {
            agent_id: agent_id.clone(),
        })
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "Agent 服务列表获取成功",
        json!({ "services": services, "total": services.len() }),
    ))
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
        let instance_id = query
            .instance_id
            .ok_or_else(|| ApiError::missing_parameter("instance_id"))?;
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

pub(super) async fn store_reset_config(State(state): State<Arc<ApiState>>) -> ApiResult {
    state
        .store
        .reset_config()
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("配置重置成功", json!({ "status": "ok" })))
}
