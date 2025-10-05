"""
MCPStore API - Store-level routes
Contains all Store-level API endpoints
"""

from typing import Optional, Dict, Any, Union

from fastapi import APIRouter, Depends, Request

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

@store_router.post("/market/refresh", response_model=APIResponse)
@timed_response
async def market_refresh(payload: Optional[Dict[str, Any]] = None):
    """手动触发市场远程刷新"""
    store = get_store()
    remote_url = None
    force = False
    if isinstance(payload, dict):
        remote_url = payload.get("remote_url")
        force = bool(payload.get("force", False))
    if remote_url:
        store._market_manager.add_remote_source(remote_url)
    ok = await store._market_manager.refresh_from_remote_async(force=force)
    
    return ResponseBuilder.success(
        message="Market refresh completed" if ok else "Market refresh failed",
        data={"refreshed": ok}
    )

@store_router.post("/for_store/add_service", response_model=APIResponse)
@timed_response
async def store_add_service(
    payload: Optional[Dict[str, Any]] = None,
    wait: Union[str, int, float] = "auto"
):
    """Store 级别添加服务
    
    支持三种模式:
    1. 空参数注册: 注册所有 mcp.json 中的服务
    2. URL方式添加服务
    3. 命令方式添加服务(本地服务)
    
    等待参数 (wait):
    - "auto": 自动根据服务类型判断(远程2s, 本地4s)
    - 数字: 等待时间(毫秒)
    """
    store = get_store()
    
    # 添加服务
    if payload is None:
        # 空参数：注册所有服务
        context_result = await store.for_store().add_service_async(wait=wait)
        service_name = "all services"
    else:
        # 有参数：添加特定服务
        context_result = await store.for_store().add_service_async(payload, wait=wait)
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
async def store_list_services():
    """获取 Store 级别服务列表
    
    返回所有已注册服务的完整信息，包括生命周期状态、
    健康状况、工具数量等详细信息。
    """
    store = get_store()
    context = store.for_store()
    services = context.list_services()

    # 构造服务列表数据
    services_data = []
    for service in services:
        service_data = {
            "name": service.name,
            "url": service.url or "",
            "command": service.command or "",
            "args": service.args or [],  # 添加命令参数
            "env": service.env or {},  # 添加环境变量
            "working_dir": service.working_dir or "",  # 添加工作目录
            "package_name": service.package_name or "",  # 添加包名
            "keep_alive": service.keep_alive,  # 添加保活标志
            "type": service.transport_type.value if service.transport_type else "unknown",
            "status": service.status.value if service.status else "unknown",
            "tools_count": service.tool_count or 0,
            "last_check": None,
            "client_id": service.client_id or "",  # 添加客户端ID
            "config": service.config or {}  # 添加完整配置（用于调试）
        }

        # 如果有状态元数据，添加详细信息
        if service.state_metadata:
            service_data["last_check"] = service.state_metadata.last_ping_time.isoformat() if service.state_metadata.last_ping_time else None

        services_data.append(service_data)

    # 简化返回，直接返回列表
    return ResponseBuilder.success(
        message=f"Retrieved {len(services_data)} services",
        data=services_data
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
async def store_list_tools():
    """获取 Store 级别工具列表
    
    返回所有可用工具的详细信息，包括工具描述、输入模式、所属服务等。
    """
    store = get_store()
    context = store.for_store()
    
    # 获取所有工具
    tools = context.list_tools()
    
    # 简化工具数据
    tools_data = []
    for tool in tools:
        tools_data.append({
            "name": tool.name,
            "service": getattr(tool, 'service_name', 'unknown'),
            "description": tool.description or "",
            "input_schema": tool.inputSchema if hasattr(tool, 'inputSchema') else {}
        })
    
    return ResponseBuilder.success(
        message=f"Retrieved {len(tools_data)} tools",
        data=tools_data
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

@store_router.get("/for_store/list_all_agents", response_model=APIResponse)
@timed_response
async def store_list_all_agents():
    """列出所有 Agent"""
    store = get_store()
    
    # 获取所有Agent列表
    agents = store.list_all_agents() if hasattr(store, 'list_all_agents') else []
    
    return ResponseBuilder.success(
        message=f"Retrieved {len(agents)} agents",
        data=agents if agents else []
    )



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
        if s.name == service_name:
            service = s
            break
    
    if not service:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"Service '{service_name}' not found",
            field="service_name"
        )
    
    # 构建简化的服务信息
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
        if s.name == service_name:
            service = s
            break
    
    if not service:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"Service '{service_name}' not found",
            field="service_name"
        )
    
    # 简化的状态信息
    status_info = {
        "name": service.name,
        "status": service.status.value if service.status else "unknown",
        "client_id": service.client_id or ""
    }
    
    return ResponseBuilder.success(
        message=f"Service status retrieved for '{service_name}'",
        data=status_info
    )
