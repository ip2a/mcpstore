"""
MCPStore API - Store-level routes
Contains all Store-level API endpoints
"""

from typing import Optional, Dict, Any, Union

from fastapi import APIRouter, Depends, Request, Query

from mcpstore import MCPStore
from mcpstore.core.models import ResponseBuilder, ErrorCode, timed_response
from mcpstore.core.models.common import APIResponse  # 保留用于 response_model
from .api_decorators import handle_exceptions, get_store
from .api_models import (
    ToolExecutionRecordResponse, ToolRecordsResponse, ToolRecordsSummaryResponse,
    SimpleToolExecutionRequest
)
from .api_service_utils import (
    ServiceOperationHelper
)

# Create Store-level router
store_router = APIRouter()

# === Store-level operations ===

# Note: sync_services 接口已删除（v0.6.0）
# 原因：文件监听机制已自动化配置同步，无需手动触发
# 迁移：直接修改 mcp.json 文件，系统将在1秒内自动同步

@store_router.get("/for_store/sync_status", response_model=APIResponse)
@timed_response
async def store_sync_status():
    """获取同步状态信息"""
    store = get_store()
    
    if hasattr(store.orchestrator, 'sync_manager') and store.orchestrator.sync_manager:
        status = store.orchestrator.sync_manager.get_sync_status()
        return ResponseBuilder.success(
            message="Sync status retrieved",
            data=status
        )
    else:
        return ResponseBuilder.success(
            message="Sync manager not available",
            data={
                "is_running": False,
                "reason": "sync_manager_not_initialized"
            }
        )

@store_router.post("/for_store/add_service", response_model=APIResponse)
@timed_response
async def store_add_service(
    payload: Optional[Dict[str, Any]] = None
):
    """Store 级别添加服务
    
    支持三种模式:
    1. 空参数注册: 注册所有 mcp.json 中的服务
    2. URL方式添加服务
    3. 命令方式添加服务(本地服务)
    
    """
    store = get_store()
    
    # 添加服务
    if payload is None:
        # 空参数：从 mcp.json 全量同步到缓存（统一同步管理器）
        sync_mgr = getattr(store.orchestrator, 'sync_manager', None)
        if not sync_mgr:
            return ResponseBuilder.error(
                code=ErrorCode.INTERNAL_ERROR,
                message="Sync manager not initialized"
            )
        await sync_mgr.sync_global_agent_store_from_mcp_json()
        context_result = True
        service_name = "all services"
    else:
        # 有参数：添加特定服务
        context_result = await store.for_store().add_service_async(payload)
        service_name = payload.get("name", "unknown")
    
    if not context_result:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_INITIALIZATION_FAILED,
            message="Service registration failed",
            details={"service_name": service_name}
        )
    
    # 返回成功，附带服务基本信息
    return ResponseBuilder.success(
        message=f"Service '{service_name}' added successfully",
        data={
            "service_name": service_name,
            "status": "initializing"
        }
    )

@store_router.get("/for_store/list_services", response_model=APIResponse)
@timed_response
async def store_list_services(
    # 分页参数（可选）
    page: Optional[int] = Query(None, ge=1, description="页码（从1开始），不传则返回全部"),
    limit: Optional[int] = Query(None, ge=1, le=1000, description="每页数量（1-1000），不传则返回全部"),

    # 过滤参数（可选）
    status: Optional[str] = Query(None, description="按状态过滤：active/ready/error/initializing"),
    search: Optional[str] = Query(None, description="搜索服务名称（模糊匹配）"),
    service_type: Optional[str] = Query(None, description="按类型过滤：sse/stdio"),

    # 排序参数（可选）
    sort_by: Optional[str] = Query(None, description="排序字段：name/status/tools_count"),
    sort_order: Optional[str] = Query(None, description="排序方向：asc/desc，默认 asc")
):
    """
    获取 Store 级别服务列表（增强版 - 统一响应格式）

    响应格式说明：
    - 始终返回包含 pagination 字段的统一格式
    - 不传分页参数时，limit 自动等于 total（返回全部数据）
    - 前端只需一套解析逻辑

    示例：

    1. 不传参数（返回全部）：
       GET /for_store/list_services
       → 返回全部服务，pagination.limit = pagination.total

    2. 使用分页：
       GET /for_store/list_services?page=1&limit=20
       → 返回第 1 页，每页 20 条

    3. 搜索：
       GET /for_store/list_services?search=weather
       → 返回名称包含 "weather" 的所有服务

    4. 过滤 + 分页：
       GET /for_store/list_services?status=error&page=1&limit=10
       → 返回错误状态的服务，第 1 页，每页 10 条

    5. 排序：
       GET /for_store/list_services?sort_by=status&sort_order=desc
       → 按状态降序排列，返回全部
    """
    from .api_models import (
        EnhancedPaginationInfo,
        ListFilterInfo,
        ListSortInfo,
        create_enhanced_pagination_info
    )

    store = get_store()
    context = store.for_store()

    # 1. 获取所有服务
    all_services = context.list_services()
    original_count = len(all_services)

    # 2. 应用过滤
    filtered_services = all_services

    if status:
        filtered_services = [
            s for s in filtered_services
            if s.get("status", "").lower() == status.lower()
        ]

    if search:
        search_lower = search.lower()
        filtered_services = [
            s for s in filtered_services
            if search_lower in s.get("name", "").lower()
        ]

    if service_type:
        filtered_services = [
            s for s in filtered_services
            if s.get("type", "") == service_type
        ]

    filtered_count = len(filtered_services)

    # 3. 应用排序
    if sort_by:
        reverse = (sort_order == "desc") if sort_order else False

        if sort_by == "name":
            filtered_services.sort(key=lambda s: s.get("name", ""), reverse=reverse)
        elif sort_by == "status":
            filtered_services.sort(key=lambda s: s.get("status", ""), reverse=reverse)
        elif sort_by == "tools_count":
            filtered_services.sort(key=lambda s: s.get("tools_count", 0) or 0, reverse=reverse)

    # 4. 应用分页（如果有）
    if page is not None or limit is not None:
        page = page or 1
        limit = limit or 20

        start = (page - 1) * limit
        end = start + limit
        paginated_services = filtered_services[start:end]
    else:
        # 不分页，返回全部
        paginated_services = filtered_services

    # 5. 构造服务数据
    def build_service_data(service) -> Dict[str, Any]:
        """构造单个服务的数据"""
        # service 已经是字典（从 StoreProxy.list_services 返回）
        # 如果是对象，转换为字典访问
        if isinstance(service, dict):
            # 直接使用字典键访问
            service_data = {
                "name": service.get("name", ""),
                "url": service.get("url", ""),
                "command": service.get("command", ""),
                "args": service.get("args", []),
                "env": service.get("env", {}),
                "working_dir": service.get("working_dir", ""),
                "package_name": service.get("package_name", ""),
                "keep_alive": service.get("keep_alive", False),
                "type": service.get("type", "unknown"),
                "status": service.get("status", "unknown"),
                "tools_count": service.get("tools_count", 0) or service.get("tool_count", 0) or 0,
                "last_check": None,
                "client_id": service.get("client_id", ""),
            }

            # 处理 state_metadata（如果存在）
            state_metadata = service.get("state_metadata")
            if state_metadata and isinstance(state_metadata, dict):
                last_ping_time = state_metadata.get("last_ping_time")
                if last_ping_time:
                    service_data["last_check"] = last_ping_time if isinstance(last_ping_time, str) else None
        else:
            # 对象访问方式（向后兼容）
            service_data = {
                "name": service.name,
                "url": service.url or "",
                "command": service.command or "",
                "args": service.args or [],
                "env": service.env or {},
                "working_dir": service.working_dir or "",
                "package_name": service.package_name or "",
                "keep_alive": service.keep_alive,
                "type": service.transport_type.value if service.transport_type else "unknown",
                "status": service.status.value if service.status else "unknown",
                "tools_count": service.tool_count or 0,
                "last_check": None,
                "client_id": service.client_id or "",
            }

            if service.state_metadata:
                service_data["last_check"] = (
                    service.state_metadata.last_ping_time.isoformat()
                    if service.state_metadata.last_ping_time else None
                )

        return service_data

    services_data = [build_service_data(s) for s in paginated_services]

    # 6. 创建统一的分页信息
    pagination = create_enhanced_pagination_info(
        page=page,
        limit=limit,
        filtered_count=filtered_count
    )

    # 7. 构造响应数据（统一格式）
    response_data = {
        "services": services_data,
        "pagination": pagination.model_dump()
    }

    # 添加过滤信息（如果有）
    if any([status, search, service_type]):
        response_data["filters"] = ListFilterInfo(
            status=status,
            search=search,
            service_type=service_type
        ).model_dump(exclude_none=True)

    # 添加排序信息（如果有）
    if sort_by:
        response_data["sort"] = ListSortInfo(
            by=sort_by,
            order=sort_order or "asc"
        ).model_dump()

    # 8. 返回统一格式的响应
    message_parts = [f"Retrieved {len(services_data)} services"]

    if filtered_count < original_count:
        message_parts.append(f"(filtered from {original_count})")

    if page is not None:
        message_parts.append(f"(page {pagination.page} of {pagination.total_pages})")

    return ResponseBuilder.success(
        message=" ".join(message_parts),
        data=response_data
    )

@store_router.post("/for_store/reset_service", response_model=APIResponse)
@timed_response
async def store_reset_service(request: Request):
    """Store 级别重置服务状态
    
    重置已存在服务的状态到 INITIALIZING，清除所有错误计数和历史记录
    """
    body = await request.json()
    
    store = get_store()
    context = store.for_store()
    
    # 提取参数
    identifier = body.get("identifier")
    client_id = body.get("client_id")
    service_name = body.get("service_name")
    
    # 确定使用的标识符
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
        message=f"Service '{used_identifier}' reset successfully",
        data={"service_name": used_identifier, "status": "initializing"}
    )

@store_router.get("/for_store/list_tools", response_model=APIResponse)
@timed_response
async def store_list_tools(
    # 分页参数（可选）
    page: Optional[int] = Query(None, ge=1, description="页码（从1开始），不传则返回全部"),
    limit: Optional[int] = Query(None, ge=1, le=1000, description="每页数量（1-1000），不传则返回全部"),

    # 过滤参数（可选）
    search: Optional[str] = Query(None, description="搜索工具名称或描述（模糊匹配）"),
    service_name: Optional[str] = Query(None, description="按服务名称过滤"),

    # 排序参数（可选）
    sort_by: Optional[str] = Query(None, description="排序字段：name/service"),
    sort_order: Optional[str] = Query(None, description="排序方向：asc/desc，默认 asc")
):
    """
    获取 Store 级别工具列表（增强版 - 统一响应格式）

    响应格式说明：
    - 始终返回包含 pagination 字段的统一格式
    - 不传分页参数时，limit 自动等于 total（返回全部数据）
    - 前端只需一套解析逻辑

    示例：

    1. 不传参数（返回全部）：
       GET /for_store/list_tools
       → 返回全部工具，pagination.limit = pagination.total

    2. 使用分页：
       GET /for_store/list_tools?page=1&limit=20
       → 返回第 1 页，每页 20 条

    3. 搜索：
       GET /for_store/list_tools?search=weather
       → 返回名称或描述包含 "weather" 的所有工具

    4. 按服务过滤：
       GET /for_store/list_tools?service_name=mcpstore-wiki
       → 返回指定服务的所有工具

    5. 排序：
       GET /for_store/list_tools?sort_by=name&sort_order=asc
       → 按名称升序排列，返回全部
    """
    from .api_models import (
        EnhancedPaginationInfo,
        ListFilterInfo,
        ListSortInfo,
        create_enhanced_pagination_info
    )

    store = get_store()
    context = store.for_store()

    # 1. 获取所有工具
    all_tools = context.list_tools()
    original_count = len(all_tools)

    # 2. 应用过滤
    filtered_tools = all_tools

    if search:
        search_lower = search.lower()
        filtered_tools = [
            t for t in filtered_tools
            if search_lower in (t.get("name", "") if isinstance(t, dict) else t.name).lower() or
               search_lower in (t.get("description", "") if isinstance(t, dict) else (t.description or "")).lower()
        ]

    if service_name:
        filtered_tools = [
            t for t in filtered_tools
            if (t.get('service_name', 'unknown') if isinstance(t, dict) else getattr(t, 'service_name', 'unknown')) == service_name
        ]

    filtered_count = len(filtered_tools)

    # 3. 应用排序
    if sort_by:
        reverse = (sort_order == "desc") if sort_order else False

        if sort_by == "name":
            filtered_tools.sort(key=lambda t: t.get("name", "") if isinstance(t, dict) else t.name, reverse=reverse)
        elif sort_by == "service":
            filtered_tools.sort(
                key=lambda t: t.get('service_name', 'unknown') if isinstance(t, dict) else getattr(t, 'service_name', 'unknown'),
                reverse=reverse
            )

    # 4. 应用分页（如果有）
    if page is not None or limit is not None:
        page = page or 1
        limit = limit or 20

        start = (page - 1) * limit
        end = start + limit
        paginated_tools = filtered_tools[start:end]
    else:
        # 不分页，返回全部
        paginated_tools = filtered_tools

    # 5. 构造工具数据
    def build_tool_data(tool) -> Dict[str, Any]:
        """构造单个工具的数据（兼容字典和对象）"""
        if isinstance(tool, dict):
            return {
                "name": tool.get("name", ""),
                "service": tool.get('service_name', 'unknown'),
                "description": tool.get("description", ""),
                "input_schema": tool.get("inputSchema", {}) or tool.get("input_schema", {})
            }
        else:
            return {
                "name": tool.name,
                "service": getattr(tool, 'service_name', 'unknown'),
                "description": tool.description or "",
                "input_schema": tool.inputSchema if hasattr(tool, 'inputSchema') else {}
            }

    tools_data = [build_tool_data(t) for t in paginated_tools]

    # 6. 创建统一的分页信息
    pagination = create_enhanced_pagination_info(
        page=page,
        limit=limit,
        filtered_count=filtered_count
    )

    # 7. 构造响应数据（统一格式）
    response_data = {
        "tools": tools_data,
        "pagination": pagination.model_dump()
    }

    # 添加过滤信息（如果有）
    if any([search, service_name]):
        response_data["filters"] = {
            "search": search,
            "service_name": service_name
        }
        # 移除 None 值
        response_data["filters"] = {k: v for k, v in response_data["filters"].items() if v is not None}

    # 添加排序信息（如果有）
    if sort_by:
        response_data["sort"] = ListSortInfo(
            by=sort_by,
            order=sort_order or "asc"
        ).model_dump()

    # 8. 返回统一格式的响应
    message_parts = [f"Retrieved {len(tools_data)} tools"]

    if filtered_count < original_count:
        message_parts.append(f"(filtered from {original_count})")

    if page is not None:
        message_parts.append(f"(page {pagination.page} of {pagination.total_pages})")

    return ResponseBuilder.success(
        message=" ".join(message_parts),
        data=response_data
    )

@store_router.get("/for_store/check_services", response_model=APIResponse)
@timed_response
async def store_check_services():
    """Store 级别批量健康检查"""
    store = get_store()
    context = store.for_store()
    health_status = context.check_services()
    
    return ResponseBuilder.success(
        message=f"Health check completed for {len(health_status.get('services', []))} services",
        data=health_status
    )

@store_router.get("/for_store/list_agents", response_model=APIResponse)
@timed_response
async def store_list_agents():
    """Store 级列出所有 Agents 概要信息（增强版，无分页）

    返回统一结构，包含 agents 明细与汇总 summary。
    """
    store = get_store()
    agents = store.for_store().list_agents()  # List[Dict[str, Any]]

    total_agents = len(agents)
    total_services = sum(int(a.get("service_count", 0)) for a in agents)
    total_tools = sum(int(a.get("tool_count", 0)) for a in agents)
    healthy_agents = sum(1 for a in agents if int(a.get("healthy_services", 0)) > 0)
    unhealthy_agents = total_agents - healthy_agents

    response_data = {
        "agents": agents,
        "summary": {
            "total_agents": total_agents,
            "total_services": total_services,
            "total_tools": total_tools,
            "healthy_agents": healthy_agents,
            "unhealthy_agents": unhealthy_agents
        }
    }

    return ResponseBuilder.success(
        message=f"Retrieved {total_agents} agents",
        data=response_data
    )

@store_router.post("/for_store/call_tool", response_model=APIResponse)
@timed_response
async def store_call_tool(request: SimpleToolExecutionRequest):
    """Store 级别工具执行"""
    store = get_store()
    result = await store.for_store().call_tool_async(request.tool_name, request.args)

    # 规范化 CallToolResult 或其它返回值为可序列化结构
    def _normalize_result(res):
        try:
            # FastMCP CallToolResult: 有 content/is_error 字段
            if hasattr(res, 'content'):
                items = []
                for c in getattr(res, 'content', []) or []:
                    try:
                        if isinstance(c, dict):
                            items.append(c)
                        elif hasattr(c, 'type') and hasattr(c, 'text'):
                            items.append({"type": getattr(c, 'type', 'text'), "text": getattr(c, 'text', '')})
                        elif hasattr(c, 'type') and hasattr(c, 'uri'):
                            items.append({"type": getattr(c, 'type', 'uri'), "uri": getattr(c, 'uri', '')})
                        else:
                            items.append(str(c))
                    except Exception:
                        items.append(str(c))
                return {"content": items, "is_error": bool(getattr(res, 'is_error', False))}
            # 已是 Dict/List
            if isinstance(res, (dict, list)):
                return res
            # 其它类型转字符串
            return {"result": str(res)}
        except Exception:
            return {"result": str(res)}

    normalized = _normalize_result(result)

    return ResponseBuilder.success(
        message=f"Tool '{request.tool_name}' executed successfully",
        data=normalized
    )

# ❌ 已删除 POST /for_store/get_service_info (v0.6.0)
# 请使用 GET /for_store/service_info/{service_name} 替代（RESTful规范）

@store_router.put("/for_store/update_service/{service_name}", response_model=APIResponse)
@timed_response
async def store_update_service(service_name: str, request: Request):
    """Store 级别更新服务配置"""
    body = await request.json()
    
    store = get_store()
    context = store.for_store()
    result = await context.update_service_async(service_name, body)
    
    if not result:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"Failed to update service '{service_name}'",
            field="service_name"
        )
    
    return ResponseBuilder.success(
        message=f"Service '{service_name}' updated successfully",
        data={"service_name": service_name, "updated_fields": list(body.keys())}
    )

@store_router.delete("/for_store/delete_service/{service_name}", response_model=APIResponse)
@timed_response
async def store_delete_service(service_name: str):
    """Store 级别删除服务"""
    store = get_store()
    context = store.for_store()
    result = await context.delete_service_async(service_name)
    
    if not result:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"Failed to delete service '{service_name}'",
            field="service_name",
            details={"service_name": service_name}
        )
    
    return ResponseBuilder.success(
        message=f"Service '{service_name}' deleted successfully",
        data={
            "service_name": service_name,
            "deleted_at": ResponseBuilder._get_timestamp()
        }
    )

@store_router.post("/for_store/disconnect_service", response_model=APIResponse)
@timed_response
async def store_disconnect_service(request: Request):
    """Store 级别断开服务（生命周期断链，不修改配置）

    Body 示例：
    {
      "service_name": "remote-demo",
      "reason": "user_requested"
    }
    """
    body = await request.json()
    service_name = body.get("service_name") or body.get("name")
    reason = body.get("reason", "user_requested")

    if not service_name:
        return ResponseBuilder.error(
            code=ErrorCode.VALIDATION_ERROR,
            message="Missing service_name"
        )

    store = get_store()
    context = store.for_store()

    try:
        ok = await context.disconnect_service_async(service_name, reason=reason)
        if ok:
            return ResponseBuilder.success(
                message=f"Service '{service_name}' disconnected",
                data={"service_name": service_name, "status": "disconnected"}
            )
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_OPERATION_FAILED,
            message=f"Failed to disconnect service '{service_name}'",
            details={"service_name": service_name}
        )
    except Exception as e:
        return ResponseBuilder.error(
            code=ErrorCode.INTERNAL_ERROR,
            message=f"Failed to disconnect service '{service_name}': {e}",
            details={"service_name": service_name}
        )

@store_router.get("/for_store/show_config", response_model=APIResponse)
@timed_response
async def store_show_config(scope: str = "all"):
    """获取运行时配置和服务映射关系
    
    Args:
        scope: 显示范围 ("all" 或 "global_agent_store")
    """
    store = get_store()
    config_data = await store.for_store().show_config_async(scope=scope)
    
    # 检查是否有错误
    if "error" in config_data:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=config_data["error"],
            details=config_data
        )
    
    scope_desc = "所有Agent配置" if scope == "all" else "global_agent_store配置"
    return ResponseBuilder.success(
        message=f"Retrieved {scope_desc}",
        data=config_data
    )

@store_router.delete("/for_store/delete_config/{client_id_or_service_name}", response_model=APIResponse)
@timed_response
async def store_delete_config(client_id_or_service_name: str):
    """Store 级别删除服务配置"""
    store = get_store()
    result = await store.for_store().delete_config_async(client_id_or_service_name)
    
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

@store_router.put("/for_store/update_config/{client_id_or_service_name}", response_model=APIResponse)
@timed_response
async def store_update_config(client_id_or_service_name: str, new_config: dict):
    """Store 级别更新服务配置"""
    store = get_store()
    context = store.for_store()
    
    # 使用带超时的配置更新方法
    success = await ServiceOperationHelper.update_config_with_timeout(
        context, 
        new_config,
        timeout=30.0
    )
    
    if not success:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=f"Failed to update configuration for {client_id_or_service_name}",
            field="client_id_or_service_name"
        )
    
    return ResponseBuilder.success(
        message=f"Configuration updated for {client_id_or_service_name}",
        data={"identifier": client_id_or_service_name, "updated": True}
    )

@store_router.post("/for_store/reset_config", response_model=APIResponse)
@timed_response
async def store_reset_config(scope: str = "all"):
    """重置配置（缓存+文件全量重置）
    
    ⚠️ 此操作不可逆，请谨慎使用
    """
    store = get_store()
    success = await store.for_store().reset_config_async(scope=scope)
    
    if not success:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=f"Failed to reset configuration",
            details={"scope": scope}
        )
    
    scope_desc = "所有配置" if scope == "all" else "global_agent_store配置"
    return ResponseBuilder.success(
        message=f"{scope_desc} reset successfully",
        data={"scope": scope, "reset": True}
    )

@store_router.post("/for_store/reset_mcpjson", response_model=APIResponse)
@timed_response
async def store_reset_mcpjson():
    """重置 mcp.json 配置文件
    
    ⚠️ 建议使用 /for_store/reset_config 替代
    """
    store = get_store()
    success = await store.for_store().reset_mcp_json_file_async()
    
    if not success:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message="Failed to reset MCP JSON file"
        )
    
    return ResponseBuilder.success(
        message="MCP JSON file and cache reset successfully",
        data={"reset": True}
    )

# Removed shard-file reset APIs (client_services.json / agent_clients.json) in single-source mode

@store_router.get("/for_store/setup_config", response_model=APIResponse)
@timed_response
async def store_setup_config():
    """获取初始化的所有配置详情
    
    🚧 此接口正在开发中，返回结构可能会调整
    """
    store = get_store()
    
    # TODO: 实现完整的配置详情获取逻辑
    # 临时返回基础信息
    setup_info = {
        "status": "under_development",
        "message": "此接口正在开发中，将在后续版本实现完整功能",
        "available_endpoints": {
            "config_query": "GET /for_store/show_config - 查看运行时配置",
            "mcp_json": "GET /for_store/show_mcpjson - 查看 mcp.json 文件",
            "services": "GET /for_store/list_services - 查看所有服务"
        }
    }
    
    return ResponseBuilder.success(
        message="Setup config endpoint (under development)",
        data=setup_info
    )

# === Store 级别统计和监控 ===

@store_router.get("/for_store/tool_records", response_model=APIResponse)
@timed_response
async def get_store_tool_records(limit: int = 50):
    """获取Store级别的工具执行记录"""
    store = get_store()
    records_data = await store.for_store().get_tool_records_async(limit)
    
    # 简化返回结构
    return ResponseBuilder.success(
        message=f"Retrieved {len(records_data.get('executions', []))} tool execution records",
        data=records_data
    )

# === 向后兼容性路由 ===

@store_router.post("/for_store/use_tool", response_model=APIResponse)
async def store_use_tool(request: SimpleToolExecutionRequest):
    """Store 级别工具执行 - 向后兼容别名
    
    推荐使用 /for_store/call_tool 接口
    """
    return await store_call_tool(request)

@store_router.post("/for_store/restart_service", response_model=APIResponse)
@timed_response
async def store_restart_service(request: Request):
    """Store 级别重启服务"""
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
    context = store.for_store()
    
    result = await context.restart_service_async(service_name)
    
    if not result:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_OPERATION_FAILED,
            message=f"Failed to restart service '{service_name}'",
            field="service_name"
        )
    
    return ResponseBuilder.success(
        message=f"Service '{service_name}' restarted successfully",
        data={"service_name": service_name, "restarted": True}
    )

@store_router.post("/for_store/wait_service", response_model=APIResponse)
@timed_response
async def store_wait_service(request: Request):
    """Store 级别等待服务达到指定状态"""
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
    context = store.for_store()
    
    result = await context.wait_service_async(
        client_id_or_service_name=client_id_or_service_name,
        status=status,
        timeout=timeout,
        raise_on_timeout=raise_on_timeout
    )
    
    return ResponseBuilder.success(
        message=f"Service wait {'completed' if result else 'timeout'}",
        data={
            "service": client_id_or_service_name,
            "target_status": status,
            "result": result
        }
    )
# ===  Agent 相关端点已移除 ===
# 使用 /for_agent/{agent_id}/list_services 来获取Agent的服务列表（推荐）

 



@store_router.get("/for_store/show_mcpjson", response_model=APIResponse)
@timed_response
async def store_show_mcpjson():
    """获取 mcp.json 配置文件的原始内容"""
    store = get_store()
    mcpjson = store.show_mcpjson()
    
    return ResponseBuilder.success(
        message="MCP JSON content retrieved",
        data=mcpjson
    )

# === 服务详情相关 API ===

@store_router.get("/for_store/service_info/{service_name}", response_model=APIResponse)
@timed_response
async def store_get_service_info_detailed(service_name: str):
    """获取服务详细信息"""
    store = get_store()
    context = store.for_store()
    
    # 查找服务
    all_services = context.list_services()
    service = None
    for s in all_services:
        s_name = s.get("name") if isinstance(s, dict) else s.name
        if s_name == service_name:
            service = s
            break
    
    if not service:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"Service '{service_name}' not found",
            field="service_name"
        )
    
    # 构建简化的服务信息（兼容字典和对象）
    if isinstance(service, dict):
        service_info = {
            "name": service.get("name", ""),
            "status": service.get("status", "unknown"),
            "type": service.get("type", "unknown"),
            "client_id": service.get("client_id", ""),
            "url": service.get("url", ""),
            "tools_count": service.get("tools_count", 0) or service.get("tool_count", 0) or 0
        }
    else:
        service_info = {
            "name": service.name,
            "status": service.status.value if service.status else "unknown",
            "type": service.transport_type.value if service.transport_type else "unknown",
            "client_id": service.client_id or "",
            "url": service.url or "",
            "tools_count": service.tool_count or 0
        }
    
    return ResponseBuilder.success(
        message=f"Service info retrieved for '{service_name}'",
        data=service_info
    )

@store_router.get("/for_store/service_status/{service_name}", response_model=APIResponse)
@timed_response
async def store_get_service_status(service_name: str):
    """获取服务状态（轻量级，纯缓存读取）"""
    store = get_store()
    context = store.for_store()
    
    # 查找服务
    all_services = context.list_services()
    service = None
    for s in all_services:
        s_name = s.get("name") if isinstance(s, dict) else s.name
        if s_name == service_name:
            service = s
            break
    
    if not service:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"Service '{service_name}' not found",
            field="service_name"
        )
    
    # 简化的状态信息（兼容字典和对象）
    if isinstance(service, dict):
        status_info = {
            "name": service.get("name", ""),
            "status": service.get("status", "unknown"),
            "client_id": service.get("client_id", "")
        }
    else:
        status_info = {
            "name": service.name,
            "status": service.status.value if service.status else "unknown",
            "client_id": service.client_id or ""
        }
    
    return ResponseBuilder.success(
        message=f"Service status retrieved for '{service_name}'",
        data=status_info
    )
