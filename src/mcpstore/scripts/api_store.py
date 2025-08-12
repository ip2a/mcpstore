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
async def store_sync_services():
    """Manually trigger service synchronization

    Force re-synchronization of all services in global_agent_store from mcp.json
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
async def store_sync_status():
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

@store_router.post("/for_store/add_service", response_model=APIResponse)
@handle_exceptions
async def store_add_service(
    payload: Optional[Dict[str, Any]] = None,
    wait: Union[str, int, float] = "auto"
):
    """Store 级别注册服务
    支持三种模式：
    1. 空参数注册：注册所有 mcp.json 中的服务
       POST /for_store/add_service?wait=auto

    2. URL方式添加服务：
       POST /for_store/add_service?wait=2000
       {
           "name": "weather",
           "url": "https://weather-api.example.com/mcp",
           "transport": "streamable-http"
       }

    3. 命令方式添加服务（本地服务）：
       POST /for_store/add_service?wait=4000
       {
           "name": "assistant",
           "command": "python",
           "args": ["./assistant_server.py"],
           "env": {"DEBUG": "true"},
           "working_dir": "/path/to/service"
       }

    等待参数 (wait):
    - "auto": 自动根据服务类型判断（远程2s，本地4s）
    - 数字: 等待时间（毫秒），如 2000 表示等待2秒
    - 最小100ms，最大30秒

    注意：本地服务需要确保：
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
                # 🔧 改进：添加完整的生命周期状态信息
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
async def store_list_services():
    """Store 级别获取服务列表 - 返回完整的生命周期信息"""
    try:
        store = get_store()
        context = store.for_store()
        services = context.list_services()

        # 🔧 改进：返回完整的服务信息，包括生命周期状态
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
async def store_init_service(request: Request):
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
async def store_list_tools():
    """Store 级别获取工具列表"""
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
async def store_check_services():
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
async def store_call_tool(request: SimpleToolExecutionRequest):
    """Store 级别工具执行"""
    try:
        import time
        import uuid

        # 记录执行开始时间
        start_time = time.time()
        trace_id = str(uuid.uuid4())[:8]

        # 🔧 直接使用SDK的call_tool_async方法，它已经包含了完整的工具解析逻辑
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
async def store_get_service_info(request: Request):
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
async def store_update_service(service_name: str, request: Request):
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
async def store_show_mcpconfig():
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

        # 🔧 修复：不直接返回MCPStoreContext对象
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
async def store_update_config(client_id_or_service_name: str, new_config: dict):
    """
    Store 级别更新服务配置

    Args:
        client_id_or_service_name: client_id或服务名（智能识别）
        new_config: 新的配置信息

    Returns:
        APIResponse: 更新结果
    """
    try:
        store = get_store()
        result = await store.for_store().update_config_async(client_id_or_service_name, new_config)

        if result.get("success"):
            return APIResponse(
                success=True,
                data=result,
                message=result.get("message", "Configuration updated successfully")
            )
        else:
            return APIResponse(
                success=False,
                data=result,
                message=result.get("error", "Failed to update configuration")
            )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"error": str(e), "client_id": None, "service_name": None, "old_config": None, "new_config": None},
            message=f"Failed to update store configuration: {str(e)}"
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
async def store_reset_mcp_json_file():
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

@store_router.post("/for_store/reset_client_services_file", response_model=APIResponse)
@handle_exceptions
async def store_reset_client_services_file():
    """Store 级别直接重置client_services.json文件"""
    try:
        store = get_store()
        success = await store.for_store().reset_client_services_file_async()
        return APIResponse(
            success=success,
            data=success,
            message="client_services.json file reset successfully" if success else "Failed to reset client_services.json file"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to reset client_services.json file: {str(e)}"
        )

@store_router.post("/for_store/reset_agent_clients_file", response_model=APIResponse)
@handle_exceptions
async def store_reset_agent_clients_file():
    """Store 级别直接重置agent_clients.json文件"""
    try:
        store = get_store()
        success = await store.for_store().reset_agent_clients_file_async()
        return APIResponse(
            success=success,
            data=success,
            message="agent_clients.json file reset successfully" if success else "Failed to reset agent_clients.json file"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to reset agent_clients.json file: {str(e)}"
        )

# === Store 级别统计和监控 ===
@store_router.get("/for_store/get_stats", response_model=APIResponse)
@handle_exceptions
async def store_get_stats():
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
async def store_health_check():
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
