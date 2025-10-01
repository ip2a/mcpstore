"""
MCPStore API - Agent-level routes
Contains all Agent-level API endpoints
"""

import logging
from typing import Dict, Any, Union, List

from fastapi import APIRouter, HTTPException, Depends, Request
from mcpstore import MCPStore
from mcpstore.core.models.common import APIResponse

from .api_decorators import handle_exceptions, get_store, validate_agent_id
from .api_models import (
    ToolExecutionRecordResponse, ToolRecordsResponse, ToolRecordsSummaryResponse,
    SimpleToolExecutionRequest
)

# Create Agent-level router
agent_router = APIRouter()

logger = logging.getLogger(__name__)

# === Agent-level operations ===
@agent_router.post("/for_agent/{agent_id}/add_service", response_model=APIResponse)
@handle_exceptions
async def agent_add_service(
    agent_id: str,
    payload: Union[List[str], Dict[str, Any]]
):
    """Agent-level service registration
    Supports two modes:
    1. Register by service name list:
       POST /for_agent/{agent_id}/add_service
       ["service_name1", "service_name2"]

    2. Add by configuration:
       POST /for_agent/{agent_id}/add_service
       {
           "name": "new_service",
           "command": "python",
           "args": ["service.py"],
           "env": {"DEBUG": "true"}
       }

    Args:
        agent_id: Agent ID
        payload: Service configuration or service name list
    """
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)

        # 使用 add_service_with_details_async 获取可序列化的结果
        result = await context.add_service_with_details_async(payload)

        return APIResponse(
            success=result.get("success", False),
            data=result,
            message=result.get("message", f"Service operation completed for agent '{agent_id}'")
        )
        
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to add service for agent '{agent_id}': {str(e)}")

@agent_router.get("/for_agent/{agent_id}/list_services", response_model=APIResponse)
@handle_exceptions
async def agent_list_services(agent_id: str) -> APIResponse:
    """Agent 级别获取服务列表"""
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)
        services = await context.list_services_async()

        #  修复：正确获取transport字段
        services_data = [
            {
                "name": service.name,
                "status": service.status.value if hasattr(service.status, 'value') else str(service.status),
                "transport": service.transport_type.value if service.transport_type else 'unknown',
                "config": getattr(service, 'config', {}),
                "client_id": getattr(service, 'client_id', None)
            }
            for service in services
        ]

        return APIResponse(
            success=True,
            data=services_data,
            message=f"Retrieved {len(services_data)} services for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to retrieve services for agent '{agent_id}': {str(e)}"
        )

@agent_router.post("/for_agent/{agent_id}/reset_service", response_model=APIResponse)
@handle_exceptions
async def agent_reset_service(agent_id: str, request: Request) -> APIResponse:
    """Agent 级别重置服务状态
    
    重置已存在服务的状态到 INITIALIZING，清除所有错误计数和历史记录，触发重新连接。
    
    适用场景：
    - ✅ 服务处于 unreachable 或 disconnected 状态，需要重试
    - ✅ 清除服务的连续失败计数和错误信息
    - ✅ 手动触发服务重新连接
    - ❌ 不适用：添加新服务（应使用 add_service）

    支持三种调用方式：
    1. {"service_name": "weather"}                  # 推荐：明确service_name（原始名称）
    2. {"client_id": "client_123"}                  # 明确client_id
    3. {"identifier": "service_name_or_client_id"}  # 通用方式

    注意：Agent级别会自动处理服务名称映射
    
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
                "expected_recovery_time": "2-4s",
                "agent_id": "agent_001"
            }
        }
    """
    try:
        validate_agent_id(agent_id)

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
        context = store.for_agent(agent_id)

        # 提取参数
        identifier = body.get("identifier")
        client_id = body.get("client_id")
        service_name = body.get("service_name")

        # 确定使用的标识符
        used_identifier = service_name or identifier or client_id
        
        # 获取重置前的状态信息
        from datetime import datetime
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
            message=f"Service '{used_identifier}' has been reset and will attempt reconnection for agent '{agent_id}'",
            data={
                "service_name": used_identifier,
                "previous_state": previous_state.value if previous_state else "unknown",
                "new_state": "initializing",
                "reset_timestamp": datetime.now().isoformat(),
                "cleared_data": cleared_data,
                "expected_recovery_time": "2-4s",
                "agent_id": agent_id,
                "context": "agent"
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
            message=f"Failed to reset service for agent '{agent_id}': {str(e)}",
            data=None
        )

@agent_router.get("/for_agent/{agent_id}/list_tools", response_model=APIResponse)
@handle_exceptions
async def agent_list_tools(agent_id: str) -> APIResponse:
    """Agent 级别获取工具列表"""
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)
        # 使用SDK的统计方法
        result = context.get_tools_with_stats()

        return APIResponse(
            success=True,
            data=result["tools"],
            metadata=result["metadata"],
            message=f"Retrieved {result['metadata']['total_tools']} tools from {result['metadata']['services_count']} services for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to retrieve tools for agent '{agent_id}': {str(e)}"
        )

@agent_router.get("/for_agent/{agent_id}/check_services", response_model=APIResponse)
@handle_exceptions
async def agent_check_services(agent_id: str) -> APIResponse:
    """Agent 级别健康检查"""
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)
        health_status = await context.check_services_async()

        return APIResponse(
            success=True,
            data=health_status,
            message=f"Health check completed for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"error": str(e)},
            message=f"Health check failed for agent '{agent_id}': {str(e)}"
        )

@agent_router.post("/for_agent/{agent_id}/call_tool", response_model=APIResponse)
@handle_exceptions
async def agent_call_tool(agent_id: str, request: SimpleToolExecutionRequest) -> APIResponse:
    """Agent 级别工具执行"""
    try:
        import time
        import uuid

        validate_agent_id(agent_id)
        
        # 记录执行开始时间
        start_time = time.time()
        trace_id = str(uuid.uuid4())[:8]

        store = get_store()
        context = store.for_agent(agent_id)
        result = await context.call_tool_async(request.tool_name, request.args)

        # 计算执行时间
        duration_ms = int((time.time() - start_time) * 1000)

        return APIResponse(
            success=True,
            data=result,
            metadata={
                "execution_time_ms": duration_ms,
                "trace_id": trace_id,
                "tool_name": request.tool_name,
                "service_name": request.service_name,
                "agent_id": agent_id
            },
            message=f"Tool '{request.tool_name}' executed successfully for agent '{agent_id}' in {duration_ms}ms"
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
                "service_name": request.service_name,
                "agent_id": agent_id
            },
            message=f"Tool execution failed for agent '{agent_id}': {str(e)}"
        )

@agent_router.put("/for_agent/{agent_id}/update_service/{service_name}", response_model=APIResponse)
@handle_exceptions
async def agent_update_service(agent_id: str, service_name: str, request: Request):
    """Agent 级别更新服务配置"""
    try:
        validate_agent_id(agent_id)
        body = await request.json()
        
        store = get_store()
        context = store.for_agent(agent_id)
        result = await context.update_service_async(service_name, body)
        
        return APIResponse(
            success=bool(result),
            data=result,
            message=f"Service '{service_name}' updated successfully for agent '{agent_id}'" if result else f"Failed to update service '{service_name}' for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to update service '{service_name}' for agent '{agent_id}': {str(e)}"
        )

@agent_router.delete("/for_agent/{agent_id}/delete_service/{service_name}", response_model=APIResponse)
@handle_exceptions
async def agent_delete_service(agent_id: str, service_name: str):
    """Agent 级别删除服务"""
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)
        result = await context.delete_service_async(service_name)
        
        return APIResponse(
            success=bool(result),
            data=result,
            message=f"Service '{service_name}' deleted successfully for agent '{agent_id}'" if result else f"Failed to delete service '{service_name}' for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to delete service '{service_name}' for agent '{agent_id}': {str(e)}"
        )

@agent_router.get("/for_agent/{agent_id}/show_mcpconfig", response_model=APIResponse)
@handle_exceptions
async def agent_show_mcpconfig(agent_id: str):
    """Agent 级别获取MCP配置"""
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)
        config = context.show_mcpconfig()

        return APIResponse(
            success=True,
            data=config,
            message=f"MCP configuration retrieved for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get MCP configuration for agent '{agent_id}': {str(e)}"
        )

@agent_router.get("/for_agent/{agent_id}/show_config", response_model=APIResponse)
@handle_exceptions
async def agent_show_config(agent_id: str):
    """
    Agent 级别显示配置信息

    显示指定Agent的所有服务配置，包括：
    - 服务名称（显示实际的带后缀版本）
    - 对应的client_id（用于后续CRUD操作）
    - 完整的服务配置信息
    """
    try:
        validate_agent_id(agent_id)
        store = get_store()
        config_data = await store.for_agent(agent_id).show_config_async()

        # 检查是否有错误
        if "error" in config_data:
            return APIResponse(
                success=False,
                data=config_data,
                message=config_data["error"]
            )

        return APIResponse(
            success=True,
            data=config_data,
            message=f"Successfully retrieved configuration for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"error": str(e), "agent_id": agent_id, "services": {}, "summary": {"total_services": 0, "total_clients": 0}},
            message=f"Failed to show agent '{agent_id}' configuration: {str(e)}"
        )

@agent_router.delete("/for_agent/{agent_id}/delete_config/{client_id_or_service_name}", response_model=APIResponse)
@handle_exceptions
async def agent_delete_config(agent_id: str, client_id_or_service_name: str):
    """
    Agent 级别删除服务配置

    Args:
        agent_id: Agent ID
        client_id_or_service_name: client_id或服务名（智能识别）

    Returns:
        APIResponse: 删除结果
    """
    try:
        validate_agent_id(agent_id)
        store = get_store()
        result = await store.for_agent(agent_id).delete_config_async(client_id_or_service_name)

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
            data={"error": str(e), "agent_id": agent_id, "client_id": None, "service_name": None},
            message=f"Failed to delete agent '{agent_id}' configuration: {str(e)}"
        )

@agent_router.put("/for_agent/{agent_id}/update_config/{client_id_or_service_name}", response_model=APIResponse)
@handle_exceptions
async def agent_update_config(agent_id: str, client_id_or_service_name: str, new_config: dict):
    """
    Agent 级别更新服务配置

    Args:
        agent_id: Agent ID
        client_id_or_service_name: client_id或服务名（智能识别）
        new_config: 新的配置信息

    Returns:
        APIResponse: 更新结果
    """
    try:
        validate_agent_id(agent_id)
        store = get_store()
        result = await store.for_agent(agent_id).update_config_async(client_id_or_service_name, new_config)

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
            data={"error": str(e), "agent_id": agent_id, "client_id": None, "service_name": None, "old_config": None, "new_config": None},
            message=f"Failed to update agent '{agent_id}' configuration: {str(e)}"
        )

@agent_router.post("/for_agent/{agent_id}/reset_config", response_model=APIResponse)
@handle_exceptions
async def agent_reset_config(agent_id: str):
    """
    Agent 级别重置配置 - 缓存优先模式

    重置指定Agent的所有服务配置，包括：
    - 清空Agent在缓存中的所有数据
    - 同步更新到映射文件
    - 不影响其他Agent的配置
    """
    try:
        validate_agent_id(agent_id)
        store = get_store()
        success = await store.for_agent(agent_id).reset_config_async()
        return APIResponse(
            success=success,
            data={"agent_id": agent_id, "reset": success},
            message=f"Agent '{agent_id}' configuration reset successfully" if success else f"Failed to reset agent '{agent_id}' configuration"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"agent_id": agent_id, "reset": False, "error": str(e)},
            message=f"Failed to reset agent '{agent_id}' configuration: {str(e)}"
        )

# === Agent 级别统计和监控 ===

@agent_router.get("/for_agent/{agent_id}/tool_records", response_model=APIResponse)
async def get_agent_tool_records(agent_id: str, limit: int = 50, store: MCPStore = Depends(get_store)):
    """获取Agent级别的工具执行记录"""
    try:
        validate_agent_id(agent_id)
        records_data = await store.for_agent(agent_id).get_tool_records_async(limit)

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
            message=f"Retrieved {len(executions)} tool execution records for agent '{agent_id}'"
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
            message=f"Failed to get tool records for agent '{agent_id}': {str(e)}"
        )

# === 向后兼容性路由 ===

@agent_router.post("/for_agent/{agent_id}/use_tool", response_model=APIResponse)
@handle_exceptions
async def agent_use_tool(agent_id: str, request: SimpleToolExecutionRequest):
    """Agent 级别工具执行 - 向后兼容别名

    注意：此接口是 /for_agent/{agent_id}/call_tool 的别名，保持向后兼容性。
    推荐使用 /for_agent/{agent_id}/call_tool 接口，与 FastMCP 命名保持一致。
    """
    return await agent_call_tool(agent_id, request)

@agent_router.post("/for_agent/{agent_id}/wait_service", response_model=APIResponse)
@handle_exceptions
async def agent_wait_service(agent_id: str, request: Request):
    """
    Agent 级别等待服务达到指定状态

    Args:
        agent_id: Agent ID

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
        context = store.for_agent(agent_id)

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
                "agent_id": agent_id,
                "client_id_or_service_name": client_id_or_service_name,
                "target_status": status,
                "timeout": timeout,
                "result": result,
                "context": "agent"
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
        logger.error(f"Agent wait service error: {e}")
        return APIResponse(
            success=False,
            message=f"Failed to wait for service: {str(e)}",
            data={"error": str(e)}
        )

@agent_router.post("/for_agent/{agent_id}/restart_service", response_model=APIResponse)
@handle_exceptions
async def agent_restart_service(agent_id: str, request: Request):
    """
    Agent 级别重启服务

    请求体格式：
    {
        "service_name": "local_service_name"  // 必需，要重启的服务名（Agent本地名称）
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
        context = store.for_agent(agent_id)

        result = await context.restart_service_async(service_name)

        return APIResponse(
            success=result,
            message=f"Agent service restart {'completed successfully' if result else 'failed'}",
            data={
                "agent_id": agent_id,
                "service_name": service_name,
                "result": result,
                "context": "agent"
            }
        )

    except ValueError as e:
        return APIResponse(
            success=False,
            message=f"Invalid parameter: {str(e)}",
            data={"error": "invalid_parameter", "details": str(e)}
        )
    except Exception as e:
        logger.error(f"Agent restart service error: {e}")
        return APIResponse(
            success=False,
            message=f"Failed to restart agent service: {str(e)}",
            data={"error": str(e)}
        )


# === Agent 级别服务详情相关 API ===

@agent_router.get("/for_agent/{agent_id}/service_info/{service_name}", response_model=APIResponse)
@handle_exceptions
async def agent_get_service_info_detailed(agent_id: str, service_name: str):
    """Agent 级别获取服务详细信息
    
    提供服务的完整信息，包括：
    - 基本配置信息
    - 运行状态
    - 生命周期状态元数据
    - 工具列表
    - 健康检查结果
    """
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)
        
        # 优先使用 SDK 的鲁棒解析逻辑，支持本地名/全局名
        # 先尝试用 SDK 直接获取（带工具和连接态）
        info = context.get_service_info(service_name)
        if not info or not getattr(info, 'success', False):
            return APIResponse(
                success=False,
                data={},
                message=getattr(info, 'message', f"Service '{service_name}' not found for agent '{agent_id}'")
            )

        # 从 SDK 返回中提取基础 ServiceInfo（为兼容后续构造保留）
        service = getattr(info, 'service', None)

        # 构建详细的服务信息
        service_info = {
            "name": service.name,
            "status": service.status.value if hasattr(service.status, 'value') else str(service.status),
            "transport": service.transport_type.value if service.transport_type else 'unknown',
            "client_id": getattr(service, 'client_id', None),
            "url": getattr(service, 'url', None),
            "command": getattr(service, 'command', None),
            "args": getattr(service, 'args', None),
            "env": getattr(service, 'env', None),
            "tool_count": getattr(service, 'tool_count', 0),
            "is_active": getattr(service, 'state_metadata', None) is not None,
            "config": getattr(service, 'config', {}),
        }
        
        # 添加生命周期状态元数据
        if hasattr(service, 'state_metadata') and service.state_metadata:
            service_info["lifecycle"] = {
                "consecutive_successes": getattr(service.state_metadata, 'consecutive_successes', 0),
                "consecutive_failures": getattr(service.state_metadata, 'consecutive_failures', 0),
                "last_ping_time": getattr(service.state_metadata, 'last_ping_time', None),
                "error_message": getattr(service.state_metadata, 'error_message', None),
                "reconnect_attempts": getattr(service.state_metadata, 'reconnect_attempts', 0),
                "state_entered_time": getattr(service.state_metadata, 'state_entered_time', None)
            }
            # 转换时间格式
            if service_info["lifecycle"]["last_ping_time"]:
                service_info["lifecycle"]["last_ping_time"] = service_info["lifecycle"]["last_ping_time"].isoformat()
            if service_info["lifecycle"]["state_entered_time"]:
                service_info["lifecycle"]["state_entered_time"] = service_info["lifecycle"]["state_entered_time"].isoformat()
        
        # 获取工具列表：从 SDK 结果直接取（更可靠），或回退到统计
        try:
            if hasattr(info, 'tools') and isinstance(info.tools, list) and info.tools:
                service_info["tools"] = info.tools
            else:
                tools_info = context.get_tools_with_stats()
                # 兼容本地名/全局名：匹配本地名
                local_name = service.name if hasattr(service, 'name') else service_name
                service_tools = [tool for tool in tools_info["tools"] if tool.get("service_name") == local_name]
                service_info["tools"] = service_tools
        except Exception as e:
            logger.warning(f"Failed to get tools for service {service_name} in agent {agent_id}: {e}")
            service_info["tools"] = []

        # 执行健康检查
        try:
            health_status = await context.check_services_async()
            service_health = None
            if isinstance(health_status, dict) and "services" in health_status:
                service_health = health_status["services"].get(service_name)
            service_info["health"] = service_health or {"status": "unknown", "message": "Health check not available"}
        except Exception as e:
            logger.warning(f"Failed to get health for service {service_name} in agent {agent_id}: {e}")
            service_info["health"] = {"status": "error", "message": str(e)}
        
        return APIResponse(
            success=True,
            data=service_info,
            message=f"Detailed service info retrieved for '{service_name}' in agent '{agent_id}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to get detailed service info for {service_name} in agent {agent_id}: {e}")
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get detailed service info: {str(e)}"
        )

@agent_router.get("/for_agent/{agent_id}/service_status/{service_name}", response_model=APIResponse)
@handle_exceptions
async def agent_get_service_status(agent_id: str, service_name: str):
    """Agent 级别获取服务状态"""
    try:
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
            return APIResponse(
                success=False,
                data={},
                message=f"Service '{service_name}' not found for agent '{agent_id}'"
            )
        
        # 构建状态信息
        status_info = {
            "name": service.name,
            "status": service.status.value if hasattr(service.status, 'value') else str(service.status),
            "is_active": getattr(service, 'state_metadata', None) is not None,
            "client_id": getattr(service, 'client_id', None),
            "last_updated": None
        }
        
        # 添加生命周期状态
        if hasattr(service, 'state_metadata') and service.state_metadata:
            lifecycle = {
                "consecutive_successes": getattr(service.state_metadata, 'consecutive_successes', 0),
                "consecutive_failures": getattr(service.state_metadata, 'consecutive_failures', 0),
                "error_message": getattr(service.state_metadata, 'error_message', None),
                "reconnect_attempts": getattr(service.state_metadata, 'reconnect_attempts', 0),
                "last_ping_time": getattr(service.state_metadata, 'last_ping_time', None),
                "state_entered_time": getattr(service.state_metadata, 'state_entered_time', None)
            }
            status_info.update(lifecycle)
            # 转换时间格式
            if status_info["last_ping_time"]:
                status_info["last_ping_time"] = status_info["last_ping_time"].isoformat()
            if status_info["state_entered_time"]:
                status_info["state_entered_time"] = status_info["state_entered_time"].isoformat()
            status_info["last_updated"] = status_info["last_ping_time"] or status_info["state_entered_time"]
        
        return APIResponse(
            success=True,
            data=status_info,
            message=f"Service status retrieved for '{service_name}' in agent '{agent_id}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to get service status for {service_name} in agent {agent_id}: {e}")
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get service status: {str(e)}"
        )

