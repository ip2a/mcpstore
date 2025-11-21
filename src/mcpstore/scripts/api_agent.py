"""
MCPStore API - Agent-level routes
Contains all Agent-level API endpoints
"""

import logging
from typing import Dict, Any, Union, List, Optional

from fastapi import APIRouter, HTTPException, Depends, Request, Query

from mcpstore import MCPStore
from mcpstore.core.models import ResponseBuilder, ErrorCode, timed_response
from mcpstore.core.models.common import APIResponse  # 保留用于 response_model
from .api_decorators import handle_exceptions, get_store, validate_agent_id
from .api_models import (
    ToolExecutionRecordResponse, ToolRecordsResponse, ToolRecordsSummaryResponse,
    SimpleToolExecutionRequest, create_enhanced_pagination_info
)

# Create Agent-level router
agent_router = APIRouter()

logger = logging.getLogger(__name__)

# === Agent-level operations ===
@agent_router.post("/for_agent/{agent_id}/add_service", response_model=APIResponse)
@timed_response
async def agent_add_service(
    agent_id: str,
    payload: Union[List[str], Dict[str, Any]]
):
    """Agent级别添加服务"""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    
    # 调用 add_service 后手动聚合详情
    try:
        await context.add_service_async(payload)
        
        # 聚合详细信息
        services = context.list_services()
        tools = context.list_tools()
        
        result = {
            "success": True,
            "message": f"Service added successfully for agent '{agent_id}'",
            "added_services": [s.get("name") if isinstance(s, dict) else getattr(s, "name", "unknown") for s in services],
            "total_services": len(services),
            "total_tools": len(tools)
        }
        
        return ResponseBuilder.success(
            message=result["message"],
            data=result
        )
    except Exception as e:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_INITIALIZATION_FAILED,
            message=f"Service operation failed for agent '{agent_id}': {str(e)}",
            details={"error": str(e)}
        )

@agent_router.get("/for_agent/{agent_id}/list_services", response_model=APIResponse)
@timed_response
async def agent_list_services(
    agent_id: str,
    # 分页参数 (可选)
    page: Optional[int] = Query(None, ge=1, description="页码，从 1 开始。省略时不分页。"),
    limit: Optional[int] = Query(None, ge=1, le=1000, description="每页数量。省略时不分页。"),
    # 过滤参数 (可选)
    status: Optional[str] = Query(None, description="按状态过滤 (如: healthy, initializing, error)"),
    search: Optional[str] = Query(None, description="按服务名称搜索 (模糊匹配)"),
    service_type: Optional[str] = Query(None, description="按服务类型过滤 (如: sse, stdio)"),
    # 排序参数 (可选)
    sort_by: Optional[str] = Query(None, description="排序字段 (name, status, type, tools_count)"),
    sort_order: Optional[str] = Query(None, description="排序方向 (asc, desc)")
):
    """
    Agent级别获取服务列表 (支持分页/过滤/排序)

    特性:
    - 所有参数均为可选，不提供任何参数时返回全部数据
    - 支持按状态、名称、类型过滤
    - 支持按多个字段排序
    - 统一返回格式，始终包含 pagination 字段

    示例:
    - 获取全部: GET /for_agent/agent1/list_services
    - 分页: GET /for_agent/agent1/list_services?page=1&limit=10
    - 过滤: GET /for_agent/agent1/list_services?status=healthy&service_type=sse
    - 搜索: GET /for_agent/agent1/list_services?search=weather
    - 排序: GET /for_agent/agent1/list_services?sort_by=name&sort_order=asc
    - 组合: GET /for_agent/agent1/list_services?status=healthy&page=1&limit=10&sort_by=tools_count&sort_order=desc
    """
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)

    # 1. 获取所有服务
    all_services = await context.list_services_async()

    # 2. 构造完整的服务数据
    services_data = []
    for service in all_services:
        service_data = {
            "name": service.name,
            "url": service.url or "",
            "command": service.command or "",
            "args": service.args or [],
            "env": service.env or {},
            "working_dir": service.working_dir or "",
            "package_name": service.package_name or "",
            "keep_alive": service.keep_alive,
            "type": service.transport_type.value if service.transport_type else 'unknown',
            "status": service.status.value if hasattr(service.status, 'value') else str(service.status),
            "tools_count": getattr(service, 'tool_count', 0),
            "client_id": service.client_id or "",
            "config": service.config or {}
        }
        services_data.append(service_data)

    # 3. 应用过滤
    filtered = services_data
    applied_filters = {}

    if status:
        filtered = [s for s in filtered if s.get("status", "").lower() == status.lower()]
        applied_filters["status"] = status

    if search:
        search_lower = search.lower()
        filtered = [s for s in filtered if search_lower in s.get("name", "").lower()]
        applied_filters["search"] = search

    if service_type:
        filtered = [s for s in filtered if s.get("type", "").lower() == service_type.lower()]
        applied_filters["service_type"] = service_type

    # 4. 应用排序
    applied_sort = {}
    if sort_by:
        reverse = (sort_order == "desc")
        if sort_by == "name":
            filtered.sort(key=lambda s: s.get("name", ""), reverse=reverse)
        elif sort_by == "status":
            filtered.sort(key=lambda s: s.get("status", ""), reverse=reverse)
        elif sort_by == "type":
            filtered.sort(key=lambda s: s.get("type", ""), reverse=reverse)
        elif sort_by == "tools_count":
            filtered.sort(key=lambda s: s.get("tools_count", 0), reverse=reverse)

        applied_sort = {"by": sort_by, "order": sort_order or "asc"}

    filtered_count = len(filtered)

    # 5. 应用分页
    if page is not None or limit is not None:
        # 有分页参数时才进行分页
        page = page or 1
        limit = limit or 20
        start = (page - 1) * limit
        paginated = filtered[start:start + limit]
    else:
        # 无分页参数时返回全部
        paginated = filtered

    # 6. 构造统一响应格式 (始终包含 pagination 字段)
    pagination = create_enhanced_pagination_info(page, limit, filtered_count)

    response_data = {
        "services": paginated,
        "pagination": pagination.dict()
    }

    # 添加过滤和排序信息（如果应用了）
    if applied_filters:
        response_data["filters"] = applied_filters
    if applied_sort:
        response_data["sort"] = applied_sort

    return ResponseBuilder.success(
        message=f"Retrieved {len(paginated)} of {filtered_count} services for agent '{agent_id}'",
        data=response_data
    )

@agent_router.get("/for_agent/{agent_id}/summary", response_model=APIResponse)
@timed_response
async def agent_summary(agent_id: str):
    """返回 Agent 级统计摘要（对象化入口封装）。"""
    validate_agent_id(agent_id)
    store = get_store()
    proxy = store.for_agent_proxy(agent_id)
    stats = proxy.get_stats()
    return ResponseBuilder.success(
        message=f"Agent '{agent_id}' summary returned",
        data=stats
    )

@agent_router.post("/for_agent/{agent_id}/reset_service", response_model=APIResponse)
@timed_response
async def agent_reset_service(agent_id: str, request: Request):
    """Agent级别重置服务状态"""
    validate_agent_id(agent_id)
    body = await request.json()
    
    store = get_store()
    context = store.for_agent(agent_id)
    
    # 提取参数
    identifier = body.get("identifier")
    client_id = body.get("client_id")
    service_name = body.get("service_name")
    
    used_identifier = service_name or identifier or client_id
    
    if not used_identifier:
        return ResponseBuilder.error(
            code=ErrorCode.VALIDATION_ERROR,
            message="Missing service identifier",
            field="service_name"
        )
    
    # 调用 init_service 方法重置状态
    await context.init_service_async(
        client_id_or_service_name=identifier,
        client_id=client_id,
        service_name=service_name
    )
    
    return ResponseBuilder.success(
        message=f"Service '{used_identifier}' reset successfully for agent '{agent_id}'",
        data={"service_name": used_identifier, "agent_id": agent_id, "status": "initializing"}
    )

@agent_router.get("/for_agent/{agent_id}/list_tools", response_model=APIResponse)
@timed_response
async def agent_list_tools(
    agent_id: str,
    # 分页参数 (可选)
    page: Optional[int] = Query(None, ge=1, description="页码，从 1 开始。省略时不分页。"),
    limit: Optional[int] = Query(None, ge=1, le=1000, description="每页数量。省略时不分页。"),
    # 过滤参数 (可选)
    search: Optional[str] = Query(None, description="按工具名称或描述搜索 (模糊匹配)"),
    service_name: Optional[str] = Query(None, description="按服务名称过滤 (精确匹配)"),
    # 排序参数 (可选)
    sort_by: Optional[str] = Query(None, description="排序字段 (name, service)"),
    sort_order: Optional[str] = Query(None, description="排序方向 (asc, desc)")
):
    """
    Agent级别获取工具列表 (支持分页/过滤/排序)

    特性:
    - 所有参数均为可选，不提供任何参数时返回全部数据
    - 支持按工具名称、描述、服务名过滤
    - 支持按名称、服务排序
    - 统一返回格式，始终包含 pagination 字段

    示例:
    - 获取全部: GET /for_agent/agent1/list_tools
    - 分页: GET /for_agent/agent1/list_tools?page=1&limit=20
    - 搜索: GET /for_agent/agent1/list_tools?search=read
    - 按服务: GET /for_agent/agent1/list_tools?service_name=filesystem
    - 排序: GET /for_agent/agent1/list_tools?sort_by=name&sort_order=asc
    - 组合: GET /for_agent/agent1/list_tools?service_name=filesystem&page=1&limit=10&sort_by=name
    """
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)

    # 1. 获取所有工具
    all_tools = context.list_tools()

    # 2. 构造工具数据
    tools_data = [
        {
            "name": tool.name,
            "service": getattr(tool, 'service_name', 'unknown'),
            "description": tool.description or ""
        }
        for tool in all_tools
    ]

    # 3. 应用过滤
    filtered = tools_data
    applied_filters = {}

    if search:
        search_lower = search.lower()
        filtered = [
            t for t in filtered
            if search_lower in t.get("name", "").lower() or search_lower in t.get("description", "").lower()
        ]
        applied_filters["search"] = search

    if service_name:
        filtered = [t for t in filtered if t.get("service", "") == service_name]
        applied_filters["service_name"] = service_name

    # 4. 应用排序
    applied_sort = {}
    if sort_by:
        reverse = (sort_order == "desc")
        if sort_by == "name":
            filtered.sort(key=lambda t: t.get("name", ""), reverse=reverse)
        elif sort_by == "service":
            filtered.sort(key=lambda t: t.get("service", ""), reverse=reverse)

        applied_sort = {"by": sort_by, "order": sort_order or "asc"}

    filtered_count = len(filtered)

    # 5. 应用分页
    if page is not None or limit is not None:
        # 有分页参数时才进行分页
        page = page or 1
        limit = limit or 20
        start = (page - 1) * limit
        paginated = filtered[start:start + limit]
    else:
        # 无分页参数时返回全部
        paginated = filtered

    # 6. 构造统一响应格式 (始终包含 pagination 字段)
    pagination = create_enhanced_pagination_info(page, limit, filtered_count)

    response_data = {
        "tools": paginated,
        "pagination": pagination.dict()
    }

    # 添加过滤和排序信息（如果应用了）
    if applied_filters:
        response_data["filters"] = applied_filters
    if applied_sort:
        response_data["sort"] = applied_sort

    return ResponseBuilder.success(
        message=f"Retrieved {len(paginated)} of {filtered_count} tools for agent '{agent_id}'",
        data=response_data
    )

@agent_router.get("/for_agent/{agent_id}/check_services", response_model=APIResponse)
@timed_response
async def agent_check_services(agent_id: str):
    """Agent级别批量健康检查"""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    health_status = await context.check_services_async()
    
    return ResponseBuilder.success(
        message=f"Health check completed for agent '{agent_id}'",
        data=health_status
    )

@agent_router.post("/for_agent/{agent_id}/call_tool", response_model=APIResponse)
@timed_response
async def agent_call_tool(agent_id: str, request: SimpleToolExecutionRequest):
    """Agent级别工具执行"""
    validate_agent_id(agent_id)
    
    store = get_store()
    context = store.for_agent(agent_id)
    result = await context.call_tool_async(request.tool_name, request.args)
    
    return ResponseBuilder.success(
        message=f"Tool '{request.tool_name}' executed successfully for agent '{agent_id}'",
        data=result
    )

@agent_router.put("/for_agent/{agent_id}/update_service/{service_name}", response_model=APIResponse)
@timed_response
async def agent_update_service(agent_id: str, service_name: str, request: Request):
    """Agent级别更新服务配置"""
    validate_agent_id(agent_id)
    body = await request.json()
    
    store = get_store()
    context = store.for_agent(agent_id)
    result = await context.update_service_async(service_name, body)
    
    if not result:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"Failed to update service '{service_name}' for agent '{agent_id}'",
            field="service_name"
        )
    
    return ResponseBuilder.success(
        message=f"Service '{service_name}' updated for agent '{agent_id}'",
        data={"service_name": service_name, "agent_id": agent_id}
    )

@agent_router.delete("/for_agent/{agent_id}/delete_service/{service_name}", response_model=APIResponse)
@timed_response
async def agent_delete_service(agent_id: str, service_name: str):
    """Agent级别删除服务"""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    result = await context.delete_service_async(service_name)
    
    if not result:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"Failed to delete service '{service_name}' for agent '{agent_id}'",
            field="service_name"
        )
    
    return ResponseBuilder.success(
        message=f"Service '{service_name}' deleted for agent '{agent_id}'",
        data={"service_name": service_name, "agent_id": agent_id}
    )

@agent_router.post("/for_agent/{agent_id}/disconnect_service", response_model=APIResponse)
@timed_response
async def agent_disconnect_service(agent_id: str, request: Request):
    """Agent 级别断开服务（生命周期断链，不修改配置）

    Body 示例：
    {
      "service_name": "localName",  # Agent 本地名
      "reason": "user_requested"
    }
    """
    validate_agent_id(agent_id)
    body = await request.json()
    local_name = body.get("service_name") or body.get("name")
    reason = body.get("reason", "user_requested")

    if not local_name:
        return ResponseBuilder.error(
            code=ErrorCode.VALIDATION_ERROR,
            message="Missing service_name",
            field="service_name"
        )

    store = get_store()
    context = store.for_agent(agent_id)

    try:
        ok = await context.disconnect_service_async(local_name, reason=reason)
        if ok:
            return ResponseBuilder.success(
                message=f"Service '{local_name}' disconnected for agent '{agent_id}'",
                data={"agent_id": agent_id, "service_name": local_name, "status": "disconnected"}
            )
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_OPERATION_FAILED,
            message=f"Failed to disconnect service '{local_name}' for agent '{agent_id}'",
            details={"agent_id": agent_id, "service_name": local_name}
        )
    except Exception as e:
        return ResponseBuilder.error(
            code=ErrorCode.INTERNAL_ERROR,
            message=f"Failed to disconnect service '{local_name}' for agent '{agent_id}': {e}",
            details={"agent_id": agent_id, "service_name": local_name}
        )

@agent_router.get("/for_agent/{agent_id}/show_mcpconfig", response_model=APIResponse)
@timed_response
async def agent_show_mcpconfig(agent_id: str):
    """Agent级别获取MCP配置"""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    config = context.show_mcpconfig()
    
    return ResponseBuilder.success(
        message=f"MCP configuration retrieved for agent '{agent_id}'",
        data=config
    )

@agent_router.get("/for_agent/{agent_id}/show_config", response_model=APIResponse)
@timed_response
async def agent_show_config(agent_id: str):
    """Agent级别显示配置信息"""
    validate_agent_id(agent_id)
    store = get_store()
    config_data = await store.for_agent(agent_id).show_config_async()
    
    # 检查是否有错误
    if "error" in config_data:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=config_data["error"],
            details=config_data
        )
    
    return ResponseBuilder.success(
        message=f"Retrieved configuration for agent '{agent_id}'",
        data=config_data
    )

@agent_router.delete("/for_agent/{agent_id}/delete_config/{client_id_or_service_name}", response_model=APIResponse)
@timed_response
async def agent_delete_config(agent_id: str, client_id_or_service_name: str):
    """Agent级别删除服务配置"""
    validate_agent_id(agent_id)
    store = get_store()
    result = await store.for_agent(agent_id).delete_config_async(client_id_or_service_name)
    
    if result.get("success"):
        return ResponseBuilder.success(
            message=result.get("message", "Configuration deleted successfully"),
            data=result
        )
    else:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=result.get("error", "Failed to delete configuration"),
            details=result
        )

@agent_router.put("/for_agent/{agent_id}/update_config/{client_id_or_service_name}", response_model=APIResponse)
@timed_response
async def agent_update_config(agent_id: str, client_id_or_service_name: str, new_config: dict):
    """Agent级别更新服务配置"""
    validate_agent_id(agent_id)
    store = get_store()
    result = await store.for_agent(agent_id).update_config_async(client_id_or_service_name, new_config)
    
    if result.get("success"):
        return ResponseBuilder.success(
            message=result.get("message", "Configuration updated successfully"),
            data=result
        )
    else:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=result.get("error", "Failed to update configuration"),
            details=result
        )

@agent_router.post("/for_agent/{agent_id}/reset_config", response_model=APIResponse)
@timed_response
async def agent_reset_config(agent_id: str):
    """Agent级别重置配置"""
    validate_agent_id(agent_id)
    store = get_store()
    success = await store.for_agent(agent_id).reset_config_async()
    
    if not success:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=f"Failed to reset agent '{agent_id}' configuration",
            field="agent_id"
        )
    
    return ResponseBuilder.success(
        message=f"Agent '{agent_id}' configuration reset successfully",
        data={"agent_id": agent_id, "reset": True}
    )

# === Agent 级别统计和监控 ===

@agent_router.get("/for_agent/{agent_id}/tool_records", response_model=APIResponse)
@timed_response
async def get_agent_tool_records(agent_id: str, limit: int = 50):
    """获取Agent级别的工具执行记录"""
    validate_agent_id(agent_id)
    store = get_store()
    records_data = await store.for_agent(agent_id).get_tool_records_async(limit)
    
    return ResponseBuilder.success(
        message=f"Retrieved {len(records_data.get('executions', []))} tool execution records for agent '{agent_id}'",
        data=records_data
    )

# === 向后兼容性路由 ===

@agent_router.post("/for_agent/{agent_id}/use_tool", response_model=APIResponse)
async def agent_use_tool(agent_id: str, request: SimpleToolExecutionRequest):
    """Agent级别工具执行 - 向后兼容别名
    
    推荐使用 /for_agent/{agent_id}/call_tool 接口
    """
    return await agent_call_tool(agent_id, request)

@agent_router.post("/for_agent/{agent_id}/wait_service", response_model=APIResponse)
@timed_response
async def agent_wait_service(agent_id: str, request: Request):
    """Agent级别等待服务达到指定状态"""
    body = await request.json()
    
    # 提取参数
    client_id_or_service_name = body.get("client_id_or_service_name")
    if not client_id_or_service_name:
        return ResponseBuilder.error(
            code=ErrorCode.VALIDATION_ERROR,
            message="Missing required parameter: client_id_or_service_name",
            field="client_id_or_service_name"
        )
    
    status = body.get("status", "healthy")
    timeout = body.get("timeout", 10.0)
    raise_on_timeout = body.get("raise_on_timeout", False)
    
    # 调用 SDK
    store = get_store()
    context = store.for_agent(agent_id)
    
    result = await context.wait_service_async(
        client_id_or_service_name=client_id_or_service_name,
        status=status,
        timeout=timeout,
        raise_on_timeout=raise_on_timeout
    )
    
    return ResponseBuilder.success(
        message=f"Service wait {'completed' if result else 'timeout'} for agent '{agent_id}'",
        data={
            "agent_id": agent_id,
            "service": client_id_or_service_name,
            "result": result
        }
    )

@agent_router.post("/for_agent/{agent_id}/restart_service", response_model=APIResponse)
@timed_response
async def agent_restart_service(agent_id: str, request: Request):
    """Agent级别重启服务"""
    body = await request.json()
    
    # 提取参数
    service_name = body.get("service_name")
    if not service_name:
        return ResponseBuilder.error(
            code=ErrorCode.VALIDATION_ERROR,
            message="Missing required parameter: service_name",
            field="service_name"
        )
    
    # 调用 SDK
    store = get_store()
    context = store.for_agent(agent_id)
    
    result = await context.restart_service_async(service_name)
    
    if not result:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_OPERATION_FAILED,
            message=f"Failed to restart service '{service_name}' for agent '{agent_id}'",
            field="service_name"
        )
    
    return ResponseBuilder.success(
        message=f"Service '{service_name}' restarted for agent '{agent_id}'",
        data={"agent_id": agent_id, "service_name": service_name, "restarted": True}
    )


# === Agent 级别服务详情相关 API ===

@agent_router.get("/for_agent/{agent_id}/service_info/{service_name}", response_model=APIResponse)
@timed_response
async def agent_get_service_info_detailed(agent_id: str, service_name: str):
    """Agent级别获取服务详细信息"""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    
    # 使用 SDK 获取服务信息
    info = context.get_service_info(service_name)
    if not info or not getattr(info, 'success', False):
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=getattr(info, 'message', f"Service '{service_name}' not found for agent '{agent_id}'"),
            field="service_name"
        )
    
    # 简化返回结构
    service = getattr(info, 'service', None)
    service_info = {
        "name": service.name,
        "status": service.status.value if hasattr(service.status, 'value') else str(service.status),
        "type": service.transport_type.value if service.transport_type else 'unknown',
        "tools_count": getattr(service, 'tool_count', 0)
    }
    
    return ResponseBuilder.success(
        message=f"Service info retrieved for '{service_name}' in agent '{agent_id}'",
        data=service_info
    )

@agent_router.get("/for_agent/{agent_id}/service_status/{service_name}", response_model=APIResponse)
@timed_response
async def agent_get_service_status(agent_id: str, service_name: str):
    """Agent级别获取服务状态"""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    
    # 查找服务
    service = None
    all_services = await context.list_services_async()
    for s in all_services:
        if s.name == service_name:
            service = s
            break
    
    if not service:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"Service '{service_name}' not found for agent '{agent_id}'",
            field="service_name"
        )
    
    # 简化状态信息
    status_info = {
        "name": service.name,
        "status": service.status.value if hasattr(service.status, 'value') else str(service.status),
        "is_active": getattr(service, 'state_metadata', None) is not None
    }
    
    return ResponseBuilder.success(
        message=f"Service status retrieved for '{service_name}' in agent '{agent_id}'",
        data=status_info
    )

