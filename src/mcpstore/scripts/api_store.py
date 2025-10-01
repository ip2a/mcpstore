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

# Note: sync_services 接口已删除（v0.6.0）
# 原因：文件监听机制已自动化配置同步，无需手动触发
# 迁移：直接修改 mcp.json 文件，系统将在1秒内自动同步

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

@store_router.post("/for_store/reset_service", response_model=APIResponse)
@handle_exceptions
async def store_reset_service(request: Request) -> APIResponse:
    """Store 级别重置服务状态
    
    重置已存在服务的状态到 INITIALIZING，清除所有错误计数和历史记录，触发重新连接。
    
    适用场景：
    - ✅ 服务处于 unreachable 或 disconnected 状态，需要重试
    - ✅ 清除服务的连续失败计数和错误信息
    - ✅ 手动触发服务重新连接
    - ❌ 不适用：添加新服务（应使用 add_service）

    支持三种调用方式：
    1. {"service_name": "weather"}                  # 推荐：明确service_name
    2. {"client_id": "client_123"}                  # 明确client_id
    3. {"identifier": "service_name_or_client_id"}  # 通用方式
    
    请求示例：
        {"service_name": "weather"}
    
    响应示例：
        {
            "success": true,
            "data": {
                "service_name": "weather",
                "previous_state": "unreachable",
                "new_state": "initializing",
                "reset_timestamp": "2025-10-01T12:34:56Z",
                "cleared_data": {
                    "consecutive_failures": 5,
                    "reconnect_attempts": 3,
                    "error_message": "Connection timeout"
                },
                "expected_recovery_time": "2-4s"
            }
        }
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

        # 确定使用的标识符
        used_identifier = service_name or identifier or client_id
        
        # 获取重置前的状态信息
        from datetime import datetime
        agent_id = store.orchestrator.client_manager.global_agent_store_id
        previous_state = store.registry.get_service_state(agent_id, used_identifier)
        previous_metadata = store.registry.get_service_metadata(agent_id, used_identifier)
        
        # 记录清除的数据
        cleared_data = {}
        if previous_metadata:
            cleared_data = {
                "consecutive_failures": previous_metadata.consecutive_failures,
                "reconnect_attempts": previous_metadata.reconnect_attempts,
                "error_message": previous_metadata.error_message
            }

        # 调用 init_service 方法重置状态
        await context.init_service_async(
            client_id_or_service_name=identifier,
            client_id=client_id,
            service_name=service_name
        )

        return APIResponse(
            success=True,
            message=f"Service '{used_identifier}' has been reset and will attempt reconnection",
            data={
                "service_name": used_identifier,
                "previous_state": previous_state.value if previous_state else "unknown",
                "new_state": "initializing",
                "reset_timestamp": datetime.now().isoformat(),
                "cleared_data": cleared_data,
                "expected_recovery_time": "2-4s",
                "context": "store"
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
            message=f"Failed to reset service: {str(e)}",
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

# ❌ 已删除 POST /for_store/get_service_info (v0.6.0)
# 请使用 GET /for_store/service_info/{service_name} 替代（RESTful规范）

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

@store_router.get("/for_store/show_config", response_model=APIResponse)
@handle_exceptions
async def store_show_config(scope: str = "all"):
    """
    【缓存层】获取运行时配置和服务映射关系
    
    数据来源：从 Registry 缓存读取
    返回内容：
    - 服务配置
    - client_id 映射关系
    - 运行时状态（通过其他接口获取）
    
    使用场景：
    - 查看当前运行的服务配置
    - 检查 service → client_id 的映射关系
    - 调试服务注册状态
    - 查看所有 Agent 的服务分布
    
    对比 show_mcpjson：
    - show_mcpjson：文件层，静态配置
    - show_config：缓存层，运行时状态

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
    【推荐】重置配置（缓存+文件全量重置）
    
    执行操作：
    1. 清空 Registry 缓存（所有服务状态、工具、会话等）
    2. 重置 mcp.json 配置文件
    
    使用场景：
    - 清理所有服务，重新开始
    - 解决配置冲突问题
    - 系统维护和重置
    
    Args:
        scope: 重置范围
            - "all": 重置所有缓存和所有JSON文件（默认）
            - "global_agent_store": 只重置global_agent_store
    
    注意：此操作不可逆，请谨慎使用
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

@store_router.post("/for_store/reset_mcpjson", response_model=APIResponse)
@handle_exceptions
async def store_reset_mcpjson() -> APIResponse:
    """
    【文件层】重置 mcp.json 配置文件
    
    ⚠️ 警告：此接口会同时清空缓存和文件，与 reset_config 功能重复
    
    执行操作：
    1. 清空 Registry 缓存（所有服务状态）
    2. 重置 mcp.json 为空配置 {"mcpServers": {}}
    
    对比 reset_config：
    - reset_config: 重置所有配置（缓存+文件）
    - reset_mcpjson: 重置所有配置（缓存+文件）
    - 实际功能相同，建议统一使用 reset_config
    
    已更名：reset_mcp_json_file → reset_mcpjson（v0.6.0）
    """
    try:
        store = get_store()
        success = await store.for_store().reset_mcp_json_file_async()
        return APIResponse(
            success=success,
            data=success,
            message="MCP JSON file and cache reset successfully" if success else "Failed to reset MCP JSON file"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to reset MCP JSON file: {str(e)}"
        )

# Removed shard-file reset APIs (client_services.json / agent_clients.json) in single-source mode

@store_router.get("/for_store/setup_config", response_model=APIResponse)
@handle_exceptions
async def store_setup_config() -> APIResponse:
    """
    获取初始化的所有配置详情
    
    返回内容：
    - Store 配置信息
    - 所有 Agent 配置
    - 服务映射关系
    - 缓存状态概览
    - 生命周期管理器状态
    
    使用场景：
    - 系统启动后查看完整配置
    - 调试配置问题
    - 导出系统配置快照
    - 管理界面展示系统状态
    
    🚧 注意：此接口正在开发中，返回结构可能会调整
    """
    try:
        store = get_store()
        
        # TODO: 实现完整的配置详情获取逻辑
        # 1. 获取 Store 级别配置
        # 2. 获取所有 Agent 配置
        # 3. 获取服务映射关系
        # 4. 获取缓存状态
        # 5. 获取生命周期管理器状态
        
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
        
        return APIResponse(
            success=True,
            data=setup_info,
            message="Setup config endpoint (under development)"
        )
        
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get setup config: {str(e)}"
        )

# === Store 级别统计和监控 ===

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
# ===  Agent 相关端点已移除 ===
# 使用 /for_agent/{agent_id}/list_services 来获取Agent的服务列表（推荐）

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



@store_router.get("/for_store/show_mcpjson", response_model=APIResponse)
@handle_exceptions
async def store_show_mcpjson() -> APIResponse:
    """
    【文件层】获取 mcp.json 配置文件的原始内容
    
    数据来源：直接读取 mcp.json 文件
    返回内容：文件的静态配置，不包含运行时状态
    
    使用场景：
    - 查看持久化的服务配置
    - 检查配置文件是否正确
    - 导出配置用于备份
    
    对比 show_config：
    - show_mcpjson：文件层，静态配置
    - show_config：缓存层，运行时状态
    """
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
    """
    【完整】获取服务详细信息
    
    数据来源：Registry 缓存 + 主动健康检查
    性能：🐌 较慢（包含健康检查调用）
    
    返回内容：
    - 基本配置信息（command, args, env, url）
    - 运行状态（status, transport, client_id）
    - 生命周期状态元数据
    - 工具列表（完整的工具信息）
    - 健康检查结果（实时检查）
    
    使用场景：
    - 服务详情页展示
    - 调试和诊断
    - 完整服务信息导出
    
    🔮 后续优化计划：
    - [ ] 考虑移除主动健康检查，改为纯缓存读取
    - [ ] 将健康检查独立为专门的接口（已有独立接口）
    - [ ] 提升查询性能，与 service_status 对齐
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
    """
    【轻量级】获取服务状态（纯缓存读取）
    
    数据来源：Registry 缓存
    性能：⚡ 极快（毫秒级）
    
    返回内容：
    - 服务基本信息（name, client_id, status）
    - 生命周期状态（成功/失败计数、错误信息）
    - 最后更新时间
    
    使用场景：
    - 轮询监控服务状态
    - Dashboard 实时展示
    - 快速状态检查
    - 列表页批量查询
    
    ⚠️ 注意：
    - 不执行主动健康检查（使用专门的健康检查接口）
    - 不包含工具列表（使用 service_info 或 list_tools）
    - 纯读取缓存，不发起网络请求
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
