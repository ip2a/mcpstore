"""
MCPStore API - Agent-level routes
Contains all Agent-level API endpoints
"""

import logging
from typing import Dict, Any, Union, List

from fastapi import APIRouter, HTTPException, Depends, Request

from mcpstore import MCPStore
from mcpstore.core.models import ResponseBuilder, ErrorCode, timed_response
from mcpstore.core.models.common import APIResponse  # 保留用于 response_model
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
@timed_response
async def agent_add_service(
    agent_id: str,
    payload: Union[List[str], Dict[str, Any]]
):
    """Agent级别添加服务"""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    
    # 使用 add_service_with_details_async 获取可序列化的结果
    result = await context.add_service_with_details_async(payload)
    
    if not result.get("success", False):
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_INITIALIZATION_FAILED,
            message=result.get("message", f"Service operation failed for agent '{agent_id}'"),
            details=result
        )
    
    return ResponseBuilder.success(
        message=result.get("message", f"Service operation completed for agent '{agent_id}'"),
        data=result
    )

@agent_router.get("/for_agent/{agent_id}/list_services", response_model=APIResponse)
@timed_response
async def agent_list_services(agent_id: str):
    """Agent级别获取服务列表"""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    services = await context.list_services_async()
    
    # 构造完整的服务数据
    services_data = []
    for service in services:
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
    
    return ResponseBuilder.success(
        message=f"Retrieved {len(services_data)} services for agent '{agent_id}'",
        data=services_data
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
async def agent_list_tools(agent_id: str):
    """Agent级别获取工具列表"""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    
    # 获取工具列表
    tools = context.list_tools()
    
    # 简化工具数据
    tools_data = [
        {
            "name": tool.name,
            "service": getattr(tool, 'service_name', 'unknown'),
            "description": tool.description or ""
        }
        for tool in tools
    ]
    
    return ResponseBuilder.success(
        message=f"Retrieved {len(tools_data)} tools for agent '{agent_id}'",
        data=tools_data
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

