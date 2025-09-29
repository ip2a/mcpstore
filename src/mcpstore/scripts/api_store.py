"""
MCPStore API - Store-level routes
Contains all Store-level API endpoints
"""

from typing import Optional, Dict, Any, Union

from fastapi import APIRouter, HTTPException, Depends, Request
from mcpstore import MCPStore
from mcpstore.core.models.common import APIResponse
from mcpstore.core.models.service import JsonUpdateRequest

from .api_decorators import handle_exceptions, get_store
from .api_service_utils import (
    ServiceOperationHelper
)
from .api_models import (
    ToolExecutionRecordResponse, ToolRecordsResponse, ToolRecordsSummaryResponse,
    NetworkEndpointResponse, SystemResourceInfoResponse, NetworkEndpointCheckRequest,
    SimpleToolExecutionRequest
)

# Create Store-level router
store_router = APIRouter()

# === Store-level operations ===

@store_router.post("/for_store/sync_services", response_model=APIResponse)
@handle_exceptions
async def store_sync_services() -> APIResponse:
    """手动触发服务同步
    
    强制从 mcp.json 重新同步 global_agent_store 中的所有服务。
    这将重新加载配置并更新所有服务的状态。
    
    Returns:
        APIResponse: 包含同步结果的响应对象
        
    Response Data Structure:
        {
            "success": bool,           # 同步是否成功
            "data": {
                "total_services": int, # 总服务数量
                "added": int,          # 新增服务数量
                "removed": int,        # 移除服务数量
                "updated": int,        # 更新服务数量
                "errors": List[str]    # 错误信息列表
            },
            "message": str            # 响应消息
        }
        
    Raises:
        MCPStoreException: 当同步过程中出现错误时抛出
    """
    try:
        store = get_store()

        if hasattr(store.orchestrator, 'sync_manager') and store.orchestrator.sync_manager:
            results = await store.orchestrator.sync_manager.manual_sync()

            return APIResponse(
                success=True,
                message="Services synchronized successfully",
                data=results
            )
        else:
            return APIResponse(
                success=False,
                message="Sync manager not available",
                data=None
            )

    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Sync failed: {str(e)}")

@store_router.get("/for_store/sync_status", response_model=APIResponse)
@handle_exceptions
async def store_sync_status() -> APIResponse:
    """获取同步状态信息"""
    try:
        store = get_store()

        if hasattr(store.orchestrator, 'sync_manager') and store.orchestrator.sync_manager:
            status = store.orchestrator.sync_manager.get_sync_status()

            return APIResponse(
                success=True,
                message="Sync status retrieved",
                data=status
            )
        else:
            return APIResponse(
                success=True,
                message="Sync manager not available",
                data={
                    "is_running": False,
                    "reason": "sync_manager_not_initialized"
                }
            )
    except Exception as e:
        return APIResponse(
            success=False,
            message=f"Failed to get sync status: {str(e)}",
            data=None
        )

@store_router.post("/market/refresh", response_model=APIResponse)
@handle_exceptions
async def market_refresh(payload: Optional[Dict[str, Any]] = None) -> APIResponse:
    """Manually trigger market remote refresh (background-safe).
    Body example: {"remote_url": "https://.../servers.json", "force": false}
    """
    store = get_store()
    remote_url = None
    force = False
    if isinstance(payload, dict):
        remote_url = payload.get("remote_url")
        force = bool(payload.get("force", False))
    if remote_url:
        store._market_manager.add_remote_source(remote_url)
    ok = await store._market_manager.refresh_from_remote_async(force=force)
    return APIResponse(success=True, data={"refreshed": ok})

@store_router.post("/for_store/add_service", response_model=APIResponse)
@handle_exceptions
async def store_add_service(
    payload: Optional[Dict[str, Any]] = None,
    wait: Union[str, int, float] = "auto"
):
    """
    Store 级别注册服务

    支持三种模式:
    1. 空参数注册: 注册所有 mcp.json 中的服务
       POST /for_store/add_service?wait=auto

    2. URL方式添加服务:
       POST /for_store/add_service?wait=2000
       {
           "name": "weather",
           "url": "https://weather-api.example.com/mcp",
           "transport": "streamable-http"
       }

    3. 命令方式添加服务(本地服务):
       POST /for_store/add_service?wait=4000
       {
           "name": "assistant",
           "command": "python",
           "args": ["./assistant_server.py"],
           "env": {"DEBUG": "true"},
           "working_dir": "/path/to/service"
       }

    等待参数 (wait):
    - "auto": 自动根据服务类型判断(远程2s, 本地4s)
    - 数字: 等待时间(毫秒), 如 2000 表示等待2秒
    - 最小100ms, 最大30秒

    注意: 本地服务需要确保:
    - 命令路径正确且可执行
    - 工作目录存在且有权限
    - 环境变量设置正确
    """
    try:
        store = get_store()

        if payload is None:
            # 空参数：注册所有服务
            context_result = await store.for_store().add_service_async(wait=wait)
        else:
            # 有参数：添加特定服务
            context_result = await store.for_store().add_service_async(payload, wait=wait)

        # 返回可序列化的数据而不是MCPStoreContext对象
        if context_result:
            # 获取服务列表作为返回数据
            services = await store.for_store().list_services_async()
            # 将ServiceInfo对象转换为可序列化的字典
            services_data = []
            for service in services:
                #  改进：添加完整的生命周期状态信息
                service_data = {
                    "name": service.name,
                    "transport": service.transport_type.value if service.transport_type else "unknown",
                    "status": service.status.value if service.status else "unknown",
                    "client_id": service.client_id,
                    "tool_count": service.tool_count,
                    "url": service.url,
                    "is_active": service.state_metadata is not None,  # 区分已激活和仅配置的服务
                }

                # 如果有状态元数据，添加详细信息
                if service.state_metadata:
                    service_data.update({
                        "consecutive_successes": service.state_metadata.consecutive_successes,
                        "consecutive_failures": service.state_metadata.consecutive_failures,
                        "last_ping_time": service.state_metadata.last_ping_time.isoformat() if service.state_metadata.last_ping_time else None,
                        "error_message": service.state_metadata.error_message,
                        "reconnect_attempts": service.state_metadata.reconnect_attempts,
                        "state_entered_time": service.state_metadata.state_entered_time.isoformat() if service.state_metadata.state_entered_time else None
                    })
                else:
                    service_data.update({
                        "note": "Service exists in configuration but is not activated"
                    })

                services_data.append(service_data)

            return APIResponse(
                success=True,
                data={
                    "services": services_data,
                    "total_services": len(services_data),
                    "message": "Service registration completed successfully"
                },
                message="Service registration completed successfully"
            )
        else:
            return APIResponse(
                success=False,
                data=None,
                message="Service registration failed"
            )
    except Exception as e:
        return APIResponse(
            success=False,
            data=None,
            message=f"Failed to register service: {str(e)}"
        )

@store_router.get("/for_store/list_services", response_model=APIResponse)
@handle_exceptions
async def store_list_services() -> APIResponse:
    """获取 Store 级别服务列表
    
    返回所有已注册服务的完整信息，包括生命周期状态、
    健康状况、工具数量等详细信息。
    
    Returns:
        APIResponse: 包含服务列表的响应对象
        
    Response Data Structure:
        {
            "success": bool,
            "data": {
                "total_services": int,          # 总服务数量
                "active_services": int,         # 活跃服务数量
                "services": [                   # 服务列表
                    {
                        "name": str,           # 服务名称
                        "status": str,         # 服务状态
                        "transport": str,      # 传输类型
                        "client_id": str,      # 客户端ID
                        "url": str,            # 服务URL
                        "tool_count": int,     # 工具数量
                        "lifecycle": {         # 生命周期信息
                            "consecutive_successes": int,
                            "consecutive_failures": int,
                            "last_ping_time": str,
                            "error_message": str
                        }
                    }
                ]
            },
            "message": str
        }
    """
    try:
        store = get_store()
        context = store.for_store()
        services = context.list_services()

        #  改进：返回完整的服务信息，包括生命周期状态
        services_data = []
        for service in services:
            service_data = {
                "name": service.name,
                "url": service.url or "",
                "command": service.command or "",
                "transport": service.transport_type.value if service.transport_type else "unknown",
                "status": service.status.value if service.status else "unknown",
                "client_id": service.client_id or "",
                "tool_count": service.tool_count or 0,
                "is_active": service.state_metadata is not None,  # 区分已激活和仅配置的服务
            }

            # 如果有状态元数据，添加详细信息
            if service.state_metadata:
                service_data.update({
                    "consecutive_successes": service.state_metadata.consecutive_successes,
                    "consecutive_failures": service.state_metadata.consecutive_failures,
                    "last_ping_time": service.state_metadata.last_ping_time.isoformat() if service.state_metadata.last_ping_time else None,
                    "error_message": service.state_metadata.error_message,
                    "reconnect_attempts": service.state_metadata.reconnect_attempts,
                    "state_entered_time": service.state_metadata.state_entered_time.isoformat() if service.state_metadata.state_entered_time else None
                })
            else:
                service_data.update({
                    "consecutive_successes": 0,
                    "consecutive_failures": 0,
                    "last_ping_time": None,
                    "error_message": None,
                    "reconnect_attempts": 0,
                    "state_entered_time": None,
                    "note": "Service exists in configuration but is not activated"
                })

            services_data.append(service_data)

        # 统计信息
        active_services = len([s for s in services_data if s["is_active"]])
        config_only_services = len(services_data) - active_services

        return APIResponse(
            success=True,
            data={
                "services": services_data,
                "total_services": len(services_data),
                "active_services": active_services,
                "config_only_services": config_only_services
            },
            message=f"Retrieved {len(services_data)} services (active: {active_services}, config-only: {config_only_services})"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to retrieve services: {str(e)}"
        )

@store_router.post("/for_store/init_service", response_model=APIResponse)
@handle_exceptions
async def store_init_service(request: Request) -> APIResponse:
    """Store 级别初始化服务到 INITIALIZING 状态

    支持三种调用方式：
    1. {"identifier": "service_name_or_client_id"}  # 通用方式
    2. {"client_id": "client_123"}                  # 明确client_id
    3. {"service_name": "weather"}                  # 明确service_name
    """
    try:
        # 解析 JSON 请求体
        try:
            body = await request.json()
        except Exception as e:
            return APIResponse(
                success=False,
                message=f"Invalid JSON format: {str(e)}",
                data=None
            )

        store = get_store()
        context = store.for_store()

        # 提取参数
        identifier = body.get("identifier")
        client_id = body.get("client_id")
        service_name = body.get("service_name")

        # 调用 init_service 方法
        await context.init_service_async(
            client_id_or_service_name=identifier,
            client_id=client_id,
            service_name=service_name
        )

        # 确定使用的标识符用于响应消息
        used_identifier = identifier or client_id or service_name

        return APIResponse(
            success=True,
            message=f"Service '{used_identifier}' initialized to INITIALIZING state successfully",
            data={
                "identifier": used_identifier,
                "context": "store",
                "status": "initializing"
            }
        )

    except ValueError as e:
        return APIResponse(
            success=False,
            message=f"Parameter validation failed: {str(e)}",
            data=None
        )
    except Exception as e:
        return APIResponse(
            success=False,
            message=f"Failed to initialize service: {str(e)}",
            data=None
        )

@store_router.get("/for_store/list_tools", response_model=APIResponse)
@handle_exceptions
async def store_list_tools() -> APIResponse:
    """获取 Store 级别工具列表
    
    返回所有可用工具的详细信息，包括工具描述、输入模式、
    所属服务、执行统计等。
    
    Returns:
        APIResponse: 包含工具列表的响应对象
        
    Response Data Structure:
        {
            "success": bool,
            "data": [                      # 工具列表
                {
                    "name": str,         # 工具名称
                    "description": str,   # 工具描述
                    "inputSchema": dict,  # 输入模式
                    "service_name": str,  # 所属服务名称
                    "executable": bool,  # 是否可执行
                    "execution_count": int,  # 执行次数
                    "last_executed": str,     # 最后执行时间
                    "average_response_time": float  # 平均响应时间
                }
            ],
            "metadata": {                # 元数据
                "total_tools": int,     # 总工具数量
                "services_count": int,   # 服务数量
                "executable_tools": int # 可执行工具数量
            },
            "message": str
        }
    """
    try:
        store = get_store()
        context = store.for_store()
        # 使用SDK的统计方法
        result = context.get_tools_with_stats()

        return APIResponse(
            success=True,
            data=result["tools"],
            metadata=result["metadata"],
            message=f"Retrieved {result['metadata']['total_tools']} tools from {result['metadata']['services_count']} services"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to retrieve tools: {str(e)}"
        )

@store_router.get("/for_store/check_services", response_model=APIResponse)
@handle_exceptions
async def store_check_services() -> APIResponse:
    """Store 级别健康检查"""
    try:
        store = get_store()
        context = store.for_store()
        health_status = context.check_services()

        return APIResponse(
            success=True,
            data=health_status,
            message="Health check completed successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"error": str(e)},
            message=f"Health check failed: {str(e)}"
        )

@store_router.post("/for_store/call_tool", response_model=APIResponse)
@handle_exceptions
async def store_call_tool(request: SimpleToolExecutionRequest) -> APIResponse:
    """Store 级别工具执行"""
    try:
        import time
        import uuid

        # 记录执行开始时间
        start_time = time.time()
        trace_id = str(uuid.uuid4())[:8]

        #  直接使用SDK的call_tool_async方法，它已经包含了完整的工具解析逻辑
        # SDK会自动处理：工具名称解析、服务推断、格式转换等
        store = get_store()
        result = await store.for_store().call_tool_async(request.tool_name, request.args)

        # 计算执行时间
        duration_ms = int((time.time() - start_time) * 1000)

        return APIResponse(
            success=True,
            data=result,
            metadata={
                "execution_time_ms": duration_ms,
                "trace_id": trace_id,
                "tool_name": request.tool_name,
                "service_name": request.service_name
            },
            message=f"Tool '{request.tool_name}' executed successfully in {duration_ms}ms"
        )
    except Exception as e:
        duration_ms = int((time.time() - start_time) * 1000) if 'start_time' in locals() else 0
        return APIResponse(
            success=False,
            data={"error": str(e)},
            metadata={
                "execution_time_ms": duration_ms,
                "trace_id": trace_id if 'trace_id' in locals() else "unknown",
                "tool_name": request.tool_name,
                "service_name": request.service_name
            },
            message=f"Tool execution failed: {str(e)}"
        )

@store_router.post("/for_store/get_service_info", response_model=APIResponse)
@handle_exceptions
async def store_get_service_info(request: Request) -> APIResponse:
    """Store 级别获取服务信息"""
    try:
        body = await request.json()
        service_name = body.get("name")

        if not service_name:
            raise HTTPException(status_code=400, detail="Service name is required")

        store = get_store()
        context = store.for_store()
        service_info = context.get_service_info(service_name)

        return APIResponse(
            success=True,
            data=service_info,
            message=f"Service info retrieved for '{service_name}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get service info: {str(e)}"
        )

@store_router.put("/for_store/update_service/{service_name}", response_model=APIResponse)
@handle_exceptions
async def store_update_service(service_name: str, request: Request) -> APIResponse:
    """Store 级别更新服务配置"""
    try:
        body = await request.json()

        store = get_store()
        context = store.for_store()
        result = await context.update_service_async(service_name, body)

        return APIResponse(
            success=bool(result),
            data=result,
            message=f"Service '{service_name}' updated successfully" if result else f"Failed to update service '{service_name}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to update service '{service_name}': {str(e)}"
        )

@store_router.delete("/for_store/delete_service/{service_name}", response_model=APIResponse)
@handle_exceptions
async def store_delete_service(service_name: str):
    """Store 级别删除服务"""
    try:
        store = get_store()
        context = store.for_store()
        result = await context.delete_service_async(service_name)

        return APIResponse(
            success=bool(result),
            data=result,
            message=f"Service '{service_name}' deleted successfully" if result else f"Failed to delete service '{service_name}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to delete service '{service_name}': {str(e)}"
        )

@store_router.get("/for_store/show_mcpconfig", response_model=APIResponse)
@handle_exceptions
async def store_show_mcpconfig() -> APIResponse:
    """Store 级别获取MCP配置"""
    try:
        store = get_store()
        context = store.for_store()
        config = context.show_mcpconfig()

        return APIResponse(
            success=True,
            data=config,
            message="MCP configuration retrieved successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get MCP configuration: {str(e)}"
        )



@store_router.post("/for_store/delete_service_two_step", response_model=APIResponse)
@handle_exceptions
async def store_delete_service_two_step(request: Request):
    """Store 级别两步操作：从MCP JSON文件删除服务 + 注销服务"""
    try:
        body = await request.json()
        service_name = body.get("service_name") or body.get("name")

        if not service_name:
            raise HTTPException(status_code=400, detail="Service name is required")

        store = get_store()
        result = await store.for_store().delete_service_two_step(service_name)

        return APIResponse(
            success=result["overall_success"],
            data=result,
            message=f"Service {service_name} deleted successfully" if result["overall_success"]
                   else f"Partial success: JSON deleted={result['step1_json_delete']}, Service unregistered={result['step2_service_unregistration']}"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={"error": str(e)},
            message=f"Failed to delete service: {str(e)}"
        )

@store_router.post("/services/activate", response_model=APIResponse)
@handle_exceptions
async def activate_service(body: dict):
    """
    激活配置文件中的服务

    Request Body:
        {
            "name": "service_name"  # 要激活的服务名称
        }
    """
    try:
        service_name = body.get("name")

        if not service_name:
            raise HTTPException(status_code=400, detail="Service name is required")

        store = get_store()
        context = store.for_store()

        # 检查服务是否存在于配置中
        services = context.list_services()
        target_service = None
        for service in services:
            if service.name == service_name:
                target_service = service
                break

        if not target_service:
            return APIResponse(
                success=False,
                data={},
                message=f"Service '{service_name}' not found in configuration"
            )

        # 检查服务是否已经激活
        if target_service.state_metadata is not None:
            return APIResponse(
                success=True,
                data={
                    "service_name": service_name,
                    "status": target_service.status.value,
                    "already_active": True
                },
                message=f"Service '{service_name}' is already activated"
            )

        # 激活服务
        activation_config = {
            "name": service_name
        }
        if target_service.url:
            activation_config["url"] = target_service.url
        if target_service.command:
            activation_config["command"] = target_service.command

        #  修复：不直接返回MCPStoreContext对象
        context.add_service(activation_config)

        # 获取激活后的服务状态
        updated_services = context.list_services()
        activated_service = None
        for service in updated_services:
            if service.name == service_name:
                activated_service = service
                break

        return APIResponse(
            success=True,
            data={
                "service_name": service_name,
                "status": activated_service.status.value if activated_service else "unknown",
                "is_active": activated_service.state_metadata is not None if activated_service else False,
                "message": "Service activated successfully"
            },
            message=f"Service '{service_name}' activated successfully"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={"error": str(e)},
            message=f"Failed to activate service: {str(e)}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"error": str(e)},
            message=f"Failed to delete service: {str(e)}"
        )

@store_router.get("/for_store/show_config", response_model=APIResponse)
@handle_exceptions
async def store_show_config(scope: str = "all"):
    """
    Store 级别显示配置信息

    Args:
        scope: 显示范围
            - "all": 显示所有Agent的配置（默认）
            - "global_agent_store": 只显示global_agent_store的配置

    Returns:
        APIResponse: 包含配置信息的响应
    """
    try:
        store = get_store()
        config_data = await store.for_store().show_config_async(scope=scope)

        # 检查是否有错误
        if "error" in config_data:
            return APIResponse(
                success=False,
                data=config_data,
                message=config_data["error"]
            )

        scope_desc = "所有Agent配置" if scope == "all" else "global_agent_store配置"
        return APIResponse(
            success=True,
            data=config_data,
            message=f"Successfully retrieved {scope_desc}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"error": str(e), "services": {}, "summary": {"total_services": 0, "total_clients": 0}},
            message=f"Failed to show store configuration: {str(e)}"
        )

@store_router.delete("/for_store/delete_config/{client_id_or_service_name}", response_model=APIResponse)
@handle_exceptions
async def store_delete_config(client_id_or_service_name: str):
    """
    Store 级别删除服务配置

    Args:
        client_id_or_service_name: client_id或服务名（智能识别）

    Returns:
        APIResponse: 删除结果
    """
    try:
        store = get_store()
        result = await store.for_store().delete_config_async(client_id_or_service_name)

        if result.get("success"):
            return APIResponse(
                success=True,
                data=result,
                message=result.get("message", "Configuration deleted successfully")
            )
        else:
            return APIResponse(
                success=False,
                data=result,
                message=result.get("error", "Failed to delete configuration")
            )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"error": str(e), "client_id": None, "service_name": None},
            message=f"Failed to delete store configuration: {str(e)}"
        )

@store_router.put("/for_store/update_config/{client_id_or_service_name}", response_model=APIResponse)
@handle_exceptions
async def store_update_config(client_id_or_service_name: str, new_config: dict) -> APIResponse:
    """
    Store 级别更新服务配置

    Args:
        client_id_or_service_name: client_id或服务名（智能识别）
        new_config: 新的配置信息

    Returns:
        APIResponse: 更新结果
    """
    store = get_store()
    context = store.for_store()
    
    # 使用带超时的配置更新方法
    success = await ServiceOperationHelper.update_config_with_timeout(
        context, 
        new_config,
        timeout=30.0
    )

    if success:
        return APIResponse(
            success=True,
            data={"client_id_or_service_name": client_id_or_service_name, "config": new_config},
            message=f"Configuration updated successfully for {client_id_or_service_name}"
        )
    else:
        return APIResponse(
            success=False,
            data={"client_id_or_service_name": client_id_or_service_name},
            message=f"Failed to update configuration for {client_id_or_service_name}"
        )

@store_router.post("/for_store/reset_config", response_model=APIResponse)
@handle_exceptions
async def store_reset_config(scope: str = "all"):
    """
    Store 级别重置配置

    Args:
        scope: 重置范围
            - "all": 重置所有缓存和所有JSON文件（默认）
            - "global_agent_store": 只重置global_agent_store
    """
    try:
        store = get_store()
        success = await store.for_store().reset_config_async(scope=scope)

        scope_desc = "所有配置" if scope == "all" else "global_agent_store配置"
        return APIResponse(
            success=success,
            data={"scope": scope, "reset": success},
            message=f"Store {scope_desc} reset successfully" if success else f"Failed to reset store {scope_desc}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"scope": scope, "reset": False, "error": str(e)},
            message=f"Failed to reset store configuration: {str(e)}"
        )

@store_router.post("/for_store/reset_mcp_json_file", response_model=APIResponse)
@handle_exceptions
async def store_reset_mcp_json_file() -> APIResponse:
    """Store 级别直接重置MCP JSON配置文件"""
    try:
        store = get_store()
        success = await store.for_store().reset_mcp_json_file_async()
        return APIResponse(
            success=success,
            data=success,
            message="MCP JSON file reset successfully" if success else "Failed to reset MCP JSON file"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to reset MCP JSON file: {str(e)}"
        )

# Removed shard-file reset APIs (client_services.json / agent_clients.json) in single-source mode

# === Store 级别统计和监控 ===
@store_router.get("/for_store/get_stats", response_model=APIResponse)
@handle_exceptions
async def store_get_stats() -> APIResponse:
    """Store 级别获取系统统计信息"""
    try:
        store = get_store()
        context = store.for_store()
        # 使用SDK的统计方法
        stats = context.get_system_stats()

        return APIResponse(
            success=True,
            data=stats,
            message="System statistics retrieved successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get system statistics: {str(e)}"
        )

@store_router.get("/for_store/health", response_model=APIResponse)
@handle_exceptions
async def store_health_check() -> APIResponse:
    """Store 级别系统健康检查"""
    try:
        # 检查Store级别健康状态
        store = get_store()
        store_health = await store.for_store().check_services_async()

        # 基本系统信息
        health_info = {
            "status": "healthy",
            "timestamp": store_health.get("timestamp") if isinstance(store_health, dict) else None,
            "store": store_health,
            "system": {
                "api_version": "0.2.0",
                "store_initialized": bool(store),
                "orchestrator_status": store_health.get("orchestrator_status", "unknown") if isinstance(store_health, dict) else "unknown",
                "context": "store"
            }
        }

        return APIResponse(
            success=True,
            data=health_info,
            message="Health check completed successfully"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={
                "status": "unhealthy",
                "error": str(e),
                "context": "store"
            },
            message=f"Health check failed: {str(e)}"
        )

@store_router.get("/for_store/tool_records", response_model=APIResponse)
async def get_store_tool_records(limit: int = 50, store: MCPStore = Depends(get_store)):
    """获取Store级别的工具执行记录"""
    try:
        store = get_store()
        records_data = await store.for_store().get_tool_records_async(limit)

        # 转换执行记录
        executions = [
            ToolExecutionRecordResponse(
                id=record["id"],
                tool_name=record["tool_name"],
                service_name=record["service_name"],
                params=record["params"],
                result=record["result"],
                error=record["error"],
                response_time=record["response_time"],
                execution_time=record["execution_time"],
                timestamp=record["timestamp"]
            ).model_dump() for record in records_data["executions"]
        ]

        # 转换汇总信息
        summary = ToolRecordsSummaryResponse(
            total_executions=records_data["summary"]["total_executions"],
            by_tool=records_data["summary"]["by_tool"],
            by_service=records_data["summary"]["by_service"]
        ).model_dump()

        response_data = ToolRecordsResponse(
            executions=executions,
            summary=summary
        ).model_dump()

        return APIResponse(
            success=True,
            data=response_data,
            message=f"Retrieved {len(executions)} tool execution records"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={
                "executions": [],
                "summary": {
                    "total_executions": 0,
                    "by_tool": {},
                    "by_service": {}
                }
            },
            message=f"Failed to get tool records: {str(e)}"
        )

@store_router.post("/for_store/network_check", response_model=APIResponse)
async def check_store_network_endpoints(request: NetworkEndpointCheckRequest, store: MCPStore = Depends(get_store)):
    """检查Store级别的网络端点状态"""
    try:
        store = get_store()
        endpoints = await store.for_store().check_network_endpoints(request.endpoints)

        endpoints_data = [
            NetworkEndpointResponse(
                endpoint_name=endpoint.endpoint_name,
                url=endpoint.url,
                status=endpoint.status,
                response_time=endpoint.response_time,
                last_checked=endpoint.last_checked,
                uptime_percentage=endpoint.uptime_percentage
            ).dict() for endpoint in endpoints
        ]

        return APIResponse(
            success=True,
            data=endpoints_data,
            message="Network endpoints checked successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to check network endpoints: {str(e)}"
        )

@store_router.get("/for_store/system_resources", response_model=APIResponse)
async def get_store_system_resources(store: MCPStore = Depends(get_store)):
    """获取Store级别的系统资源信息"""
    try:
        store = get_store()
        resources = await store.for_store().get_system_resource_info_async()

        return APIResponse(
            success=True,
            data=SystemResourceInfoResponse(
                server_uptime=resources.server_uptime,
                memory_total=resources.memory_total,
                memory_used=resources.memory_used,
                memory_percentage=resources.memory_percentage,
                disk_usage_percentage=resources.disk_usage_percentage,
                network_traffic_in=resources.network_traffic_in,
                network_traffic_out=resources.network_traffic_out
            ).dict(),
            message="System resources retrieved successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get system resources: {str(e)}"
        )

# === 向后兼容性路由 ===

@store_router.post("/for_store/use_tool", response_model=APIResponse)
@handle_exceptions
async def store_use_tool(request: SimpleToolExecutionRequest):
    """Store 级别工具执行 - 向后兼容别名

    注意：此接口是 /for_store/call_tool 的别名，保持向后兼容性。
    推荐使用 /for_store/call_tool 接口，与 FastMCP 命名保持一致。
    """
    return await store_call_tool(request)

@store_router.post("/for_store/restart_service", response_model=APIResponse)
@handle_exceptions
async def store_restart_service(request: Request):
    """
    Store 级别重启服务

    请求体格式：
    {
        "service_name": "service_name"  // 必需，要重启的服务名
    }

    Returns:
        APIResponse: 重启结果
    """
    try:
        body = await request.json()

        # 提取参数
        service_name = body.get("service_name")
        if not service_name:
            return APIResponse(
                success=False,
                message="Missing required parameter: service_name",
                data={"error": "service_name is required"}
            )

        # 调用 SDK
        store = get_store()
        context = store.for_store()

        result = await context.restart_service_async(service_name)

        return APIResponse(
            success=result,
            message=f"Service restart {'completed successfully' if result else 'failed'}",
            data={
                "service_name": service_name,
                "result": result,
                "context": "store"
            }
        )

    except ValueError as e:
        return APIResponse(
            success=False,
            message=f"Invalid parameter: {str(e)}",
            data={"error": "invalid_parameter", "details": str(e)}
        )
    except Exception as e:
        logger.error(f"Store restart service error: {e}")
        return APIResponse(
            success=False,
            message=f"Failed to restart service: {str(e)}",
            data={"error": str(e)}
        )

@store_router.post("/for_store/wait_service", response_model=APIResponse)
@handle_exceptions
async def store_wait_service(request: Request):
    """
    Store 级别等待服务达到指定状态

    请求体格式：
    {
        "client_id_or_service_name": "service_name_or_client_id",
        "status": "healthy" | ["healthy", "warning"],  // 可选，默认"healthy"
        "timeout": 10.0,                               // 可选，默认10秒
        "raise_on_timeout": false                      // 可选，默认false
    }

    Returns:
        APIResponse: 等待结果
    """
    try:
        body = await request.json()

        # 提取参数
        client_id_or_service_name = body.get("client_id_or_service_name")
        if not client_id_or_service_name:
            return APIResponse(
                success=False,
                message="Missing required parameter: client_id_or_service_name",
                data={"error": "client_id_or_service_name is required"}
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

        return APIResponse(
            success=result,
            message=f"Service wait completed: {'success' if result else 'timeout'}",
            data={
                "client_id_or_service_name": client_id_or_service_name,
                "target_status": status,
                "timeout": timeout,
                "result": result,
                "context": "store"
            }
        )

    except TimeoutError as e:
        return APIResponse(
            success=False,
            message=f"Service wait timeout: {str(e)}",
            data={"error": "timeout", "details": str(e)}
        )
    except ValueError as e:
        return APIResponse(
            success=False,
            message=f"Invalid parameter: {str(e)}",
            data={"error": "invalid_parameter", "details": str(e)}
        )
    except Exception as e:
        logger.error(f"Store wait service error: {e}")
        return APIResponse(
            success=False,
            message=f"Failed to wait for service: {str(e)}",
            data={"error": str(e)}
        )

# ===  新增：Agent 相关端点 ===

@store_router.get("/for_store/list_services_by_agent", response_model=APIResponse)
@handle_exceptions
async def store_list_services_by_agent(agent_id: Optional[str] = None):
    """按 Agent 筛选服务列表"""
    try:
        store = get_store()
        context = store.for_store()

        # 获取所有服务
        all_services = context.list_services()

        if agent_id is None:
            # 返回所有服务
            services_data = []
            for service in all_services:
                service_data = {
                    "name": service.name,
                    "transport": service.transport_type.value if service.transport_type else "unknown",
                    "status": service.status.value if service.status else "unknown",
                    "client_id": service.client_id,
                    "tool_count": service.tool_count,
                    "is_agent_service": "_byagent_" in service.name,
                    "agent_id": None,
                    "local_name": None
                }

                # 如果是 Agent 服务，解析 Agent 信息
                if service_data["is_agent_service"]:
                    try:
                        from mcpstore.core.parsers.agent_service_parser import AgentServiceParser
                        parser = AgentServiceParser()
                        info = parser.parse_agent_service_name(service.name)
                        if info.is_valid:
                            service_data["agent_id"] = info.agent_id
                            service_data["local_name"] = info.local_name
                    except Exception as e:
                        logger.warning(f"Failed to parse agent service {service.name}: {e}")

                services_data.append(service_data)

            return APIResponse(
                success=True,
                message="All services retrieved successfully",
                data={
                    "services": services_data,
                    "total_count": len(services_data),
                    "agent_filter": None
                }
            )

        else:
            # 筛选指定 Agent 的服务
            agent_services = []
            store_services = []

            for service in all_services:
                if "_byagent_" in service.name:
                    # Agent 服务
                    try:
                        from mcpstore.core.parsers.agent_service_parser import AgentServiceParser
                        parser = AgentServiceParser()
                        info = parser.parse_agent_service_name(service.name)
                        if info.is_valid and info.agent_id == agent_id:
                            service_data = {
                                "name": service.name,
                                "transport": service.transport_type.value if service.transport_type else "unknown",
                                "status": service.status.value if service.status else "unknown",
                                "client_id": service.client_id,
                                "tool_count": service.tool_count,
                                "is_agent_service": True,
                                "agent_id": info.agent_id,
                                "local_name": info.local_name
                            }
                            agent_services.append(service_data)
                    except Exception as e:
                        logger.warning(f"Failed to parse agent service {service.name}: {e}")
                else:
                    # Store 原生服务
                    if agent_id == "global_agent_store":
                        service_data = {
                            "name": service.name,
                            "transport": service.transport_type.value if service.transport_type else "unknown",
                            "status": service.status.value if service.status else "unknown",
                            "client_id": service.client_id,
                            "tool_count": service.tool_count,
                            "is_agent_service": False,
                            "agent_id": "global_agent_store",
                            "local_name": service.name
                        }
                        store_services.append(service_data)

            # 合并结果
            filtered_services = agent_services + store_services

            return APIResponse(
                success=True,
                message=f"Services for agent '{agent_id}' retrieved successfully",
                data={
                    "services": filtered_services,
                    "total_count": len(filtered_services),
                    "agent_filter": agent_id,
                    "agent_services_count": len(agent_services),
                    "store_services_count": len(store_services)
                }
            )

    except Exception as e:
        logger.error(f"Store list services by agent error: {e}")
        return APIResponse(
            success=False,
            message=f"Failed to list services by agent: {str(e)}",
            data={"error": str(e)}
        )

@store_router.get("/for_store/list_all_agents", response_model=APIResponse)
@handle_exceptions
async def store_list_all_agents() -> APIResponse:
    """列出所有 Agent"""
    try:
        store = get_store()
        context = store.for_store()

        # 获取所有服务
        all_services = context.list_services()

        # 解析 Agent 信息
        agents_info = {}
        store_services_count = 0

        from mcpstore.core.parsers.agent_service_parser import AgentServiceParser
        parser = AgentServiceParser()

        for service in all_services:
            if "_byagent_" in service.name:
                # Agent 服务
                try:
                    info = parser.parse_agent_service_name(service.name)
                    if info.is_valid:
                        if info.agent_id not in agents_info:
                            agents_info[info.agent_id] = {
                                "agent_id": info.agent_id,
                                "services": [],
                                "service_count": 0,
                                "status_summary": {"healthy": 0, "warning": 0, "error": 0, "unknown": 0}
                            }

                        # 添加服务信息
                        service_data = {
                            "global_name": service.name,
                            "local_name": info.local_name,
                            "status": service.status.value if service.status else "unknown",
                            "client_id": service.client_id,
                            "tool_count": service.tool_count
                        }

                        agents_info[info.agent_id]["services"].append(service_data)
                        agents_info[info.agent_id]["service_count"] += 1

                        # 统计状态
                        status = service.status.value if service.status else "unknown"
                        if status in agents_info[info.agent_id]["status_summary"]:
                            agents_info[info.agent_id]["status_summary"][status] += 1
                        else:
                            agents_info[info.agent_id]["status_summary"]["unknown"] += 1

                except Exception as e:
                    logger.warning(f"Failed to parse agent service {service.name}: {e}")
            else:
                # Store 原生服务
                store_services_count += 1

        # 转换为列表格式
        agents_list = list(agents_info.values())

        return APIResponse(
            success=True,
            message="All agents retrieved successfully",
            data={
                "agents": agents_list,
                "total_agents": len(agents_list),
                "store_services_count": store_services_count,
                "total_services": len(all_services)
            }
        )

    except Exception as e:
        logger.error(f"Store list all agents error: {e}")
        return APIResponse(
            success=False,
            message=f"Failed to list all agents: {str(e)}",
            data={"error": str(e)}
        )



@store_router.get("/for_store/get_json_config", response_model=APIResponse)
@handle_exceptions
async def store_get_json_config() -> APIResponse:
    """Store 级别获取 JSON 配置"""
    try:
        store = get_store()
        config = store.get_json_config()
        return APIResponse(
            success=True,
            data=config,
            message="JSON configuration retrieved successfully"
        )
    except Exception as e:
        logger.error(f"Failed to get JSON config: {e}")
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get JSON configuration: {str(e)}"
        )

@store_router.get("/for_store/show_mcpjson", response_model=APIResponse)
@handle_exceptions
async def store_show_mcpjson() -> APIResponse:
    """Store 级别显示 mcp.json 内容（已存在，但确保与其他配置 API 一致）"""
    try:
        store = get_store()
        mcpjson = store.show_mcpjson()
        return APIResponse(
            success=True,
            data=mcpjson,
            message="MCP JSON content retrieved successfully"
        )
    except Exception as e:
        logger.error(f"Failed to show MCP JSON: {e}")
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to show MCP JSON: {str(e)}"
        )

# === 服务详情相关 API ===

@store_router.get("/for_store/service_info/{service_name}", response_model=APIResponse)
@handle_exceptions
async def store_get_service_info_detailed(service_name: str):
    """Store 级别获取服务详细信息
    
    提供服务的完整信息，包括：
    - 基本配置信息
    - 运行状态
    - 生命周期状态元数据
    - 工具列表
    - 健康检查结果
    """
    try:
        store = get_store()
        context = store.for_store()
        
        # 查找服务
        service = None
        all_services = context.list_services()
        for s in all_services:
            if s.name == service_name:
                service = s
                break
        
        if not service:
            return APIResponse(
                success=False,
                data={},
                message=f"Service '{service_name}' not found"
            )
        
        # 构建详细的服务信息
        service_info = {
            "name": service.name,
            "status": service.status.value if service.status else "unknown",
            "transport": service.transport_type.value if service.transport_type else "unknown",
            "client_id": service.client_id,
            "url": service.url,
            "command": service.command,
            "args": service.args,
            "env": service.env,
            "tool_count": service.tool_count,
            "is_active": service.state_metadata is not None,
            "config": getattr(service, 'config', {}),
        }
        
        # 添加生命周期状态元数据
        if service.state_metadata:
            service_info["lifecycle"] = {
                "consecutive_successes": service.state_metadata.consecutive_successes,
                "consecutive_failures": service.state_metadata.consecutive_failures,
                "last_ping_time": service.state_metadata.last_ping_time.isoformat() if service.state_metadata.last_ping_time else None,
                "error_message": service.state_metadata.error_message,
                "reconnect_attempts": service.state_metadata.reconnect_attempts,
                "state_entered_time": service.state_metadata.state_entered_time.isoformat() if service.state_metadata.state_entered_time else None
            }
        
        # 获取工具列表
        try:
            tools_info = context.get_tools_with_stats()
            service_tools = [tool for tool in tools_info["tools"] if tool.get("service_name") == service_name]
            service_info["tools"] = service_tools
        except Exception as e:
            logger.warning(f"Failed to get tools for service {service_name}: {e}")
            service_info["tools"] = []
        
        # 执行健康检查
        try:
            health_status = await context.check_services_async()
            service_health = None
            if isinstance(health_status, dict) and "services" in health_status:
                service_health = health_status["services"].get(service_name)
            service_info["health"] = service_health or {"status": "unknown", "message": "Health check not available"}
        except Exception as e:
            logger.warning(f"Failed to get health for service {service_name}: {e}")
            service_info["health"] = {"status": "error", "message": str(e)}
        
        return APIResponse(
            success=True,
            data=service_info,
            message=f"Detailed service info retrieved for '{service_name}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to get detailed service info for {service_name}: {e}")
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get detailed service info: {str(e)}"
        )

@store_router.get("/for_store/service_status/{service_name}", response_model=APIResponse)
@handle_exceptions
async def store_get_service_status(service_name: str):
    """Store 级别获取服务状态"""
    try:
        store = get_store()
        context = store.for_store()
        
        # 查找服务
        service = None
        all_services = context.list_services()
        for s in all_services:
            if s.name == service_name:
                service = s
                break
        
        if not service:
            return APIResponse(
                success=False,
                data={},
                message=f"Service '{service_name}' not found"
            )
        
        # 构建状态信息
        status_info = {
            "name": service.name,
            "status": service.status.value if service.status else "unknown",
            "is_active": service.state_metadata is not None,
            "client_id": service.client_id,
            "last_updated": None
        }
        
        # 添加生命周期状态
        if service.state_metadata:
            status_info.update({
                "consecutive_successes": service.state_metadata.consecutive_successes,
                "consecutive_failures": service.state_metadata.consecutive_failures,
                "error_message": service.state_metadata.error_message,
                "reconnect_attempts": service.state_metadata.reconnect_attempts,
                "last_ping_time": service.state_metadata.last_ping_time.isoformat() if service.state_metadata.last_ping_time else None,
                "state_entered_time": service.state_metadata.state_entered_time.isoformat() if service.state_metadata.state_entered_time else None
            })
            status_info["last_updated"] = status_info["last_ping_time"] or status_info["state_entered_time"]
        
        return APIResponse(
            success=True,
            data=status_info,
            message=f"Service status retrieved for '{service_name}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to get service status for {service_name}: {e}")
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get service status: {str(e)}"
        )

@store_router.post("/for_store/service_health/{service_name}", response_model=APIResponse)
@handle_exceptions
async def store_check_service_health(service_name: str):
    """Store 级别检查服务健康状态"""
    try:
        store = get_store()
        context = store.for_store()
        
        # 首先检查服务是否存在
        service = None
        all_services = context.list_services()
        for s in all_services:
            if s.name == service_name:
                service = s
                break
        
        if not service:
            return APIResponse(
                success=False,
                data={},
                message=f"Service '{service_name}' not found"
            )
        
        # 执行健康检查
        health_status = await context.check_services_async()
        service_health = None
        
        if isinstance(health_status, dict) and "services" in health_status:
            service_health = health_status["services"].get(service_name)
        
        if not service_health:
            return APIResponse(
                success=False,
                data={"service_name": service_name},
                message=f"Health status not available for service '{service_name}'"
            )
        
        # 构建健康详情
        health_details = {
            "service_name": service_name,
            "status": service_health.get("status", "unknown"),
            "message": service_health.get("message", "No health information available"),
            "timestamp": service_health.get("timestamp"),
            "uptime": service_health.get("uptime"),
            "error_count": service_health.get("error_count", 0),
            "last_error": service_health.get("last_error"),
            "response_time": service_health.get("response_time"),
            "is_healthy": service_health.get("status") in ["healthy", "ready"]
        }
        
        return APIResponse(
            success=True,
            data=health_details,
            message=f"Health check completed for service '{service_name}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to check service health for {service_name}: {e}")
        return APIResponse(
            success=False,
            data={"service_name": service_name, "error": str(e)},
            message=f"Failed to check service health: {str(e)}"
        )

@store_router.get("/for_store/service_health_details/{service_name}", response_model=APIResponse)
@handle_exceptions
async def store_get_service_health_details(service_name: str):
    """Store 级别获取服务健康详情"""
    try:
        store = get_store()
        context = store.for_store()
        
        # 首先检查服务是否存在
        service = None
        all_services = context.list_services()
        for s in all_services:
            if s.name == service_name:
                service = s
                break
        
        if not service:
            return APIResponse(
                success=False,
                data={},
                message=f"Service '{service_name}' not found"
            )
        
        # 获取完整的服务信息
        service_info = {
            "name": service.name,
            "status": service.status.value if service.status else "unknown",
            "client_id": service.client_id,
            "transport": service.transport_type.value if service.transport_type else "unknown"
        }
        
        # 添加生命周期状态
        if service.state_metadata:
            service_info["lifecycle"] = {
                "consecutive_successes": service.state_metadata.consecutive_successes,
                "consecutive_failures": service.state_metadata.consecutive_failures,
                "error_message": service.state_metadata.error_message,
                "reconnect_attempts": service.state_metadata.reconnect_attempts,
                "last_ping_time": service.state_metadata.last_ping_time.isoformat() if service.state_metadata.last_ping_time else None,
                "state_entered_time": service.state_metadata.state_entered_time.isoformat() if service.state_metadata.state_entered_time else None
            }
        
        # 执行健康检查
        health_status = await context.check_services_async()
        service_health = None
        
        if isinstance(health_status, dict) and "services" in health_status:
            service_health = health_status["services"].get(service_name)
        
        health_details = service_health or {
            "status": "unknown",
            "message": "Health check not available"
        }
        
        # 合并信息
        result = {
            "service": service_info,
            "health": health_details,
            "summary": {
                "is_healthy": health_details.get("status") in ["healthy", "ready"],
                "is_active": service.state_metadata is not None,
                "has_errors": bool(service.state_metadata and service.state_metadata.error_message),
                "consecutive_failures": service.state_metadata.consecutive_failures if service.state_metadata else 0
            }
        }
        
        return APIResponse(
            success=True,
            data=result,
            message=f"Health details retrieved for service '{service_name}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to get service health details for {service_name}: {e}")
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get service health details: {str(e)}"
        )
