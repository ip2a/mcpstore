"""
MCPStore API 路由
提供所有 HTTP API 端点，保持与 MCPStore 核心方法的一致性
"""

from fastapi import APIRouter, HTTPException, Depends
from mcpstore import MCPStore
from mcpstore.core.models.service import (
    RegisterRequestUnion, JsonUpdateRequest,
    ServiceInfoResponse, ServicesResponse
)
from mcpstore.core.models.tool import (
    ToolExecutionRequest, ToolsResponse
)
from mcpstore.core.models.common import (
    APIResponse, RegistrationResponse, ConfigResponse,
    ExecutionResponse
)
from typing import Optional, List, Dict, Any, Union
from pydantic import BaseModel, ValidationError, Field
from functools import wraps
from datetime import timedelta
import asyncio

# === 统一响应模型 ===
# APIResponse 已移动到 common.py 中，通过导入使用

# === 监控配置模型 ===
class MonitoringConfig(BaseModel):
    """监控配置模型"""
    heartbeat_interval_seconds: Optional[int] = Field(default=None, ge=10, le=300, description="心跳检查间隔（秒），范围10-300")
    reconnection_interval_seconds: Optional[int] = Field(default=None, ge=10, le=600, description="重连尝试间隔（秒），范围10-600")
    cleanup_interval_hours: Optional[int] = Field(default=None, ge=1, le=24, description="资源清理间隔（小时），范围1-24")
    max_reconnection_queue_size: Optional[int] = Field(default=None, ge=10, le=200, description="最大重连队列大小，范围10-200")
    max_heartbeat_history_hours: Optional[int] = Field(default=None, ge=1, le=168, description="心跳历史保留时间（小时），范围1-168")
    http_timeout_seconds: Optional[int] = Field(default=None, ge=1, le=30, description="HTTP超时时间（秒），范围1-30")

# === 工具函数 ===
def handle_exceptions(func):
    """统一的异常处理装饰器"""
    @wraps(func)
    async def wrapper(*args, **kwargs):
        try:
            result = await func(*args, **kwargs)
            # 如果结果已经是APIResponse，直接返回
            if isinstance(result, APIResponse):
                return result
            # 否则包装成APIResponse
            return APIResponse(success=True, data=result)
        except HTTPException:
            # HTTPException应该直接传递，不要包装
            raise
        except ValidationError as e:
            # Pydantic验证错误，返回400
            raise HTTPException(status_code=400, detail=f"Validation error: {str(e)}")
        except ValueError as e:
            raise HTTPException(status_code=400, detail=str(e))
        except Exception as e:
            raise HTTPException(status_code=500, detail=str(e))
    return wrapper

def validate_agent_id(agent_id: str):
    """验证 agent_id"""
    if not agent_id:
        raise HTTPException(status_code=400, detail="agent_id is required")
    if not isinstance(agent_id, str):
        raise HTTPException(status_code=400, detail="Invalid agent_id format")

    # 检查agent_id格式：只允许字母、数字、下划线、连字符
    import re
    if not re.match(r'^[a-zA-Z0-9_-]+$', agent_id):
        raise HTTPException(status_code=400, detail="Invalid agent_id format: only letters, numbers, underscore and hyphen allowed")

    # 检查长度
    if len(agent_id) > 100:
        raise HTTPException(status_code=400, detail="agent_id too long (max 100 characters)")

def validate_service_names(service_names: Optional[List[str]]):
    """验证 service_names"""
    if service_names and not isinstance(service_names, list):
        raise HTTPException(status_code=400, detail="Invalid service_names format")
    if service_names and not all(isinstance(name, str) for name in service_names):
        raise HTTPException(status_code=400, detail="All service names must be strings")

router = APIRouter()
store = MCPStore.setup_store()

# === Store 级别操作 ===
@router.post("/for_store/add_service", response_model=APIResponse)
@handle_exceptions
async def store_add_service(
    payload: Optional[Dict[str, Any]] = None
):
    """Store 级别注册服务
    支持三种模式：
    1. 空参数注册：注册所有 mcp.json 中的服务
       POST /for_store/add_service
    
    2. URL方式添加服务：
       POST /for_store/add_service
       {
           "name": "weather",
           "url": "https://weather-api.example.com/mcp",
           "transport": "streamable-http"
       }
    
    3. 命令方式添加服务：
       POST /for_store/add_service
       {
           "name": "assistant",
           "command": "python",
           "args": ["./assistant_server.py"],
           "env": {"DEBUG": "true"}
       }
    
    Returns:
        APIResponse: {
            "success": true/false,
            "data": true/false,  # 是否成功添加服务
            "message": "错误信息（如果有）"
        }
    """
    try:
        context = store.for_store()
        
        # 1. 空参数注册
        if not payload:
            result = await context.add_service()
            # add_service返回MCPStoreContext对象，表示成功
            success = result is not None
            return APIResponse(
                success=success,
                data=success,
                message="Successfully registered all services" if success else "Failed to register services"
            )
        
        # 2/3. 配置方式添加服务
        if isinstance(payload, dict):
            # 检查是否是mcpServers格式
            if "mcpServers" in payload:
                # mcpServers格式，不需要name字段
                pass
            else:
                # 单个服务配置格式，需要name字段
                if "name" not in payload:
                    raise HTTPException(status_code=400, detail="Service name is required")

                if "url" in payload and "command" in payload:
                    raise HTTPException(status_code=400, detail="Cannot specify both url and command")

                # 自动推断transport类型（如果未指定）
                if "url" in payload and "transport" not in payload:
                    url = payload["url"]
                    if "/sse" in url.lower():
                        payload["transport"] = "sse"
                    else:
                        payload["transport"] = "streamable-http"

                if "command" in payload and not isinstance(payload.get("args", []), list):
                    raise HTTPException(status_code=400, detail="Args must be a list")
                
            result = await context.add_service(payload)
            # add_service返回MCPStoreContext对象，表示成功
            success = result is not None
            return APIResponse(
                success=success,
                data=success,
                message="Successfully added service" if success else "Failed to add service"
            )
        
        raise HTTPException(status_code=400, detail="Invalid payload format")
        
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to add service: {str(e)}")

@router.get("/for_store/list_services", response_model=APIResponse)
@handle_exceptions
async def store_list_services():
    """Store 级别获取服务列表"""
    return await store.for_store().list_services()

@router.get("/for_store/list_tools", response_model=APIResponse)
@handle_exceptions
async def store_list_tools():
    """Store 级别获取工具列表"""
    return await store.for_store().list_tools()

@router.get("/for_store/check_services", response_model=APIResponse)
@handle_exceptions
async def store_check_services():
    """Store 级别健康检查"""
    return await store.for_store().check_services()

@router.post("/for_store/use_tool", response_model=APIResponse)
@handle_exceptions
async def store_use_tool(request: ToolExecutionRequest):
    """Store 级别使用工具"""
    if not request.tool_name or not isinstance(request.tool_name, str):
        raise HTTPException(status_code=400, detail="tool_name is required and must be a string")
    if request.args is None or not isinstance(request.args, dict):
        raise HTTPException(status_code=400, detail="args is required and must be a dictionary")

    try:
        # 先检查工具是否存在
        tools = await store.for_store().list_tools()
        tool_exists = any(tool.name == request.tool_name for tool in tools)
        if not tool_exists:
            raise HTTPException(status_code=400, detail=f"Tool '{request.tool_name}' not found")

        result = await store.for_store().use_tool(request.tool_name, request.args)
        return APIResponse(
            success=True,
            data=result,
            message=f"Tool '{request.tool_name}' executed successfully"
        )
    except HTTPException:
        raise
    except Exception as e:
        # 如果工具存在但执行失败，仍然返回成功但包含错误信息
        return APIResponse(
            success=False,
            data={"error": str(e)},
            message=f"Tool '{request.tool_name}' execution failed: {str(e)}"
        )

# === Agent 级别操作 ===
@router.post("/for_agent/{agent_id}/add_service", response_model=APIResponse)
@handle_exceptions
async def agent_add_service(
    agent_id: str,
    payload: Union[List[str], Dict[str, Any]]
):
    """Agent 级别注册服务
    支持两种模式：
    1. 通过服务名列表注册：
       POST /for_agent/{agent_id}/add_service
       ["服务名1", "服务名2"]
    
    2. 通过配置添加：
       POST /for_agent/{agent_id}/add_service
       {
           "name": "新服务",
           "command": "python",
           "args": ["service.py"],
           "env": {"DEBUG": "true"}
       }
    
    Args:
        agent_id: Agent ID
        payload: 服务配置或服务名列表
    
    Returns:
        APIResponse: {
            "success": true/false,
            "data": true/false,  # 是否成功添加服务
            "message": "错误信息（如果有）"
        }
    """
    try:
        validate_agent_id(agent_id)
        context = store.for_agent(agent_id)
        
        # 1. 服务名列表方式
        if isinstance(payload, list):
            validate_service_names(payload)
            result = await context.add_service(payload)
            # add_service返回MCPStoreContext对象，表示成功
            success = result is not None
            return APIResponse(
                success=success,
                data=success,
                message="Successfully registered services" if success else "Failed to register services"
            )
        
        # 2. 配置方式
        if isinstance(payload, dict):
            # 检查是否是mcpServers格式
            if "mcpServers" in payload:
                # mcpServers格式，不需要name字段
                pass
            else:
                # 单个服务配置格式，需要name字段
                if "name" not in payload:
                    raise HTTPException(status_code=400, detail="Service name is required")

                if "url" in payload and "command" in payload:
                    raise HTTPException(status_code=400, detail="Cannot specify both url and command")

                # 自动推断transport类型（如果未指定）
                if "url" in payload and "transport" not in payload:
                    url = payload["url"]
                    if "/sse" in url.lower():
                        payload["transport"] = "sse"
                    else:
                        payload["transport"] = "streamable-http"

                if "command" in payload and not isinstance(payload.get("args", []), list):
                    raise HTTPException(status_code=400, detail="Args must be a list")
                
            result = await context.add_service(payload)
            # add_service返回MCPStoreContext对象，表示成功
            success = result is not None
            return APIResponse(
                success=success,
                data=success,
                message="Successfully added service" if success else "Failed to add service"
            )
        
        raise HTTPException(status_code=400, detail="Invalid payload format")
        
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to add service for agent '{agent_id}': {str(e)}")

@router.get("/for_agent/{agent_id}/list_services", response_model=APIResponse)
@handle_exceptions
async def agent_list_services(agent_id: str):
    """Agent 级别获取服务列表"""
    validate_agent_id(agent_id)
    return await store.for_agent(agent_id).list_services()

@router.get("/for_agent/{agent_id}/list_tools", response_model=APIResponse)
@handle_exceptions
async def agent_list_tools(agent_id: str):
    """Agent 级别获取工具列表"""
    validate_agent_id(agent_id)
    return await store.for_agent(agent_id).list_tools()

@router.get("/for_agent/{agent_id}/check_services", response_model=APIResponse)
@handle_exceptions
async def agent_check_services(agent_id: str):
    """Agent 级别健康检查"""
    validate_agent_id(agent_id)
    return await store.for_agent(agent_id).check_services()

@router.post("/for_agent/{agent_id}/use_tool", response_model=APIResponse)
@handle_exceptions
async def agent_use_tool(agent_id: str, request: ToolExecutionRequest):
    """Agent 级别使用工具"""
    validate_agent_id(agent_id)
    if not request.tool_name or not isinstance(request.tool_name, str):
        raise HTTPException(status_code=400, detail="tool_name is required and must be a string")
    if request.args is None or not isinstance(request.args, dict):
        raise HTTPException(status_code=400, detail="args is required and must be a dictionary")

    try:
        # 先检查工具是否存在
        tools = await store.for_agent(agent_id).list_tools()
        tool_exists = any(tool.name == request.tool_name for tool in tools)
        if not tool_exists:
            raise HTTPException(status_code=400, detail=f"Tool '{request.tool_name}' not found for agent '{agent_id}'")

        result = await store.for_agent(agent_id).use_tool(request.tool_name, request.args)
        return APIResponse(
            success=True,
            data=result,
            message=f"Tool '{request.tool_name}' executed successfully for agent '{agent_id}'"
        )
    except HTTPException:
        raise
    except Exception as e:
        # 如果工具存在但执行失败，仍然返回成功但包含错误信息
        return APIResponse(
            success=False,
            data={"error": str(e)},
            message=f"Tool '{request.tool_name}' execution failed for agent '{agent_id}': {str(e)}"
        )

# === 通用服务信息查询 ===
@router.get("/services/{name}", response_model=APIResponse)
@handle_exceptions
async def get_service_info(name: str, agent_id: Optional[str] = None):
    """获取服务信息，支持 Store/Agent 上下文"""
    if agent_id:
        validate_agent_id(agent_id)
        return await store.for_agent(agent_id).get_service_info(name)
    return await store.for_store().get_service_info(name)

# === Store 级别服务管理操作 ===
@router.post("/for_store/delete_service", response_model=APIResponse)
@handle_exceptions
async def store_delete_service(request: Dict[str, str]):
    """Store 级别删除服务"""
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        result = await store.for_store().delete_service(service_name)
        return APIResponse(
            success=result,
            data=result,
            message=f"Service {service_name} deleted successfully" if result else f"Failed to delete service {service_name}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to delete service {service_name}: {str(e)}"
        )

@router.post("/for_store/update_service", response_model=APIResponse)
@handle_exceptions
async def store_update_service(request: Dict[str, Any]):
    """Store 级别更新服务配置"""
    service_name = request.get("name")
    config = request.get("config")

    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")
    if not config:
        raise HTTPException(status_code=400, detail="Service config is required")

    try:
        result = await store.for_store().update_service(service_name, config)
        return APIResponse(
            success=result,
            data=result,
            message=f"Service {service_name} updated successfully" if result else f"Failed to update service {service_name}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to update service {service_name}: {str(e)}"
        )

@router.post("/for_store/restart_service", response_model=APIResponse)
@handle_exceptions
async def store_restart_service(request: Dict[str, str]):
    """Store 级别重启服务"""
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        context = store.for_store()

        # 获取服务配置
        service_info = await context.get_service_info(service_name)
        if not service_info:
            raise HTTPException(status_code=404, detail=f"Service {service_name} not found")

        # 删除服务
        delete_result = await context.delete_service(service_name)
        if not delete_result:
            raise HTTPException(status_code=500, detail=f"Failed to stop service {service_name}")

        # 重新添加服务
        add_result = await context.add_service([service_name])

        return APIResponse(
            success=add_result,
            data=add_result,
            message=f"Service {service_name} restarted successfully" if add_result else f"Failed to restart service {service_name}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to restart service {service_name}: {str(e)}"
        )

# === Agent 级别服务管理操作 ===
@router.post("/for_agent/{agent_id}/delete_service", response_model=APIResponse)
@handle_exceptions
async def agent_delete_service(agent_id: str, request: Dict[str, str]):
    """Agent 级别删除服务"""
    validate_agent_id(agent_id)
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        result = await store.for_agent(agent_id).delete_service(service_name)
        return APIResponse(
            success=result,
            data=result,
            message=f"Service {service_name} deleted successfully" if result else f"Failed to delete service {service_name}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to delete service {service_name}: {str(e)}"
        )

@router.post("/for_agent/{agent_id}/update_service", response_model=APIResponse)
@handle_exceptions
async def agent_update_service(agent_id: str, request: Dict[str, Any]):
    """Agent 级别更新服务配置"""
    validate_agent_id(agent_id)
    service_name = request.get("name")
    config = request.get("config")

    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")
    if not config:
        raise HTTPException(status_code=400, detail="Service config is required")

    try:
        result = await store.for_agent(agent_id).update_service(service_name, config)
        return APIResponse(
            success=result,
            data=result,
            message=f"Service {service_name} updated successfully" if result else f"Failed to update service {service_name}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to update service {service_name}: {str(e)}"
        )

@router.post("/for_agent/{agent_id}/restart_service", response_model=APIResponse)
@handle_exceptions
async def agent_restart_service(agent_id: str, request: Dict[str, str]):
    """Agent 级别重启服务"""
    validate_agent_id(agent_id)
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        context = store.for_agent(agent_id)

        # 获取服务配置
        service_info = await context.get_service_info(service_name)
        if not service_info:
            raise HTTPException(status_code=404, detail=f"Service {service_name} not found")

        # 删除服务
        delete_result = await context.delete_service(service_name)
        if not delete_result:
            raise HTTPException(status_code=500, detail=f"Failed to stop service {service_name}")

        # 重新添加服务
        add_result = await context.add_service([service_name])

        return APIResponse(
            success=add_result,
            data=add_result,
            message=f"Service {service_name} restarted successfully" if add_result else f"Failed to restart service {service_name}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to restart service {service_name}: {str(e)}"
        )

# === Store 级别批量操作 ===
@router.post("/for_store/batch_add_services", response_model=APIResponse)
@handle_exceptions
async def store_batch_add_services(request: Dict[str, List[Any]]):
    """Store 级别批量添加服务"""
    services = request.get("services", [])
    if not services:
        raise HTTPException(status_code=400, detail="Services list is required")

    context = store.for_store()
    results = []

    for i, service in enumerate(services):
        try:
            if isinstance(service, str):
                # 服务名方式
                result = await context.add_service([service])
            elif isinstance(service, dict):
                # 配置方式
                result = await context.add_service(service)
            else:
                results.append({
                    "index": i,
                    "success": False,
                    "message": "Invalid service format"
                })
                continue

            # add_service返回MCPStoreContext对象，表示成功
            success = result is not None
            results.append({
                "index": i,
                "service": service,
                "success": success,
                "message": f"Add operation {'succeeded' if success else 'failed'}"
            })

        except Exception as e:
            results.append({
                "index": i,
                "service": service,
                "success": False,
                "message": str(e)
            })

    success_count = sum(1 for r in results if r.get("success", False))
    total_count = len(results)

    return APIResponse(
        success=success_count > 0,
        data={
            "results": results,
            "summary": {
                "total": total_count,
                "succeeded": success_count,
                "failed": total_count - success_count
            }
        },
        message=f"Batch add completed: {success_count}/{total_count} succeeded"
    )

@router.post("/for_store/batch_update_services", response_model=APIResponse)
@handle_exceptions
async def store_batch_update_services(request: Dict[str, List[Dict[str, Any]]]):
    """Store 级别批量更新服务"""
    updates = request.get("updates", [])
    if not updates:
        raise HTTPException(status_code=400, detail="Updates list is required")

    context = store.for_store()
    results = []

    for i, update in enumerate(updates):
        if not isinstance(update, dict):
            results.append({
                "index": i,
                "success": False,
                "message": "Invalid update format"
            })
            continue

        name = update.get("name")
        config = update.get("config")

        if not name or not config:
            results.append({
                "index": i,
                "success": False,
                "message": "Name and config are required"
            })
            continue

        try:
            result = await context.update_service(name, config)
            results.append({
                "index": i,
                "name": name,
                "success": result,
                "message": f"Update operation {'succeeded' if result else 'failed'}"
            })

        except Exception as e:
            results.append({
                "index": i,
                "name": name,
                "success": False,
                "message": str(e)
            })

    success_count = sum(1 for r in results if r.get("success", False))
    total_count = len(results)

    return APIResponse(
        success=success_count > 0,
        data={
            "results": results,
            "summary": {
                "total": total_count,
                "succeeded": success_count,
                "failed": total_count - success_count
            }
        },
        message=f"Batch update completed: {success_count}/{total_count} succeeded"
    )

# === Agent 级别批量操作 ===
@router.post("/for_agent/{agent_id}/batch_add_services", response_model=APIResponse)
@handle_exceptions
async def agent_batch_add_services(agent_id: str, request: Dict[str, List[Any]]):
    """Agent 级别批量添加服务"""
    validate_agent_id(agent_id)
    services = request.get("services", [])
    if not services:
        raise HTTPException(status_code=400, detail="Services list is required")

    context = store.for_agent(agent_id)
    results = []

    for i, service in enumerate(services):
        try:
            if isinstance(service, str):
                # 服务名方式
                result = await context.add_service([service])
            elif isinstance(service, dict):
                # 配置方式
                result = await context.add_service(service)
            else:
                results.append({
                    "index": i,
                    "success": False,
                    "message": "Invalid service format"
                })
                continue

            # add_service返回MCPStoreContext对象，表示成功
            success = result is not None
            results.append({
                "index": i,
                "service": service,
                "success": success,
                "message": f"Add operation {'succeeded' if success else 'failed'}"
            })

        except Exception as e:
            results.append({
                "index": i,
                "service": service,
                "success": False,
                "message": str(e)
            })

    success_count = sum(1 for r in results if r.get("success", False))
    total_count = len(results)

    return APIResponse(
        success=success_count > 0,
        data={
            "results": results,
            "summary": {
                "total": total_count,
                "succeeded": success_count,
                "failed": total_count - success_count
            }
        },
        message=f"Batch add completed: {success_count}/{total_count} succeeded"
    )

@router.post("/for_agent/{agent_id}/batch_update_services", response_model=APIResponse)
@handle_exceptions
async def agent_batch_update_services(agent_id: str, request: Dict[str, List[Dict[str, Any]]]):
    """Agent 级别批量更新服务"""
    validate_agent_id(agent_id)
    updates = request.get("updates", [])
    if not updates:
        raise HTTPException(status_code=400, detail="Updates list is required")

    context = store.for_agent(agent_id)
    results = []

    for i, update in enumerate(updates):
        if not isinstance(update, dict):
            results.append({
                "index": i,
                "success": False,
                "message": "Invalid update format"
            })
            continue

        name = update.get("name")
        config = update.get("config")

        if not name or not config:
            results.append({
                "index": i,
                "success": False,
                "message": "Name and config are required"
            })
            continue

        try:
            result = await context.update_service(name, config)
            results.append({
                "index": i,
                "name": name,
                "success": result,
                "message": f"Update operation {'succeeded' if result else 'failed'}"
            })

        except Exception as e:
            results.append({
                "index": i,
                "name": name,
                "success": False,
                "message": str(e)
            })

    success_count = sum(1 for r in results if r.get("success", False))
    total_count = len(results)

    return APIResponse(
        success=success_count > 0,
        data={
            "results": results,
            "summary": {
                "total": total_count,
                "succeeded": success_count,
                "failed": total_count - success_count
            }
        },
        message=f"Batch update completed: {success_count}/{total_count} succeeded"
    )

# === Store 级别服务信息查询 ===
@router.post("/for_store/get_service_info", response_model=APIResponse)
@handle_exceptions
async def store_get_service_info(request: Dict[str, str]):
    """Store 级别获取服务信息"""
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        result = await store.for_store().get_service_info(service_name)

        # 检查服务是否存在 - 主要检查service字段是否为None
        if (not result or
            (hasattr(result, 'service') and result.service is None) or
            (isinstance(result, dict) and result.get('service') is None)):
            raise HTTPException(status_code=404, detail=f"Service '{service_name}' not found")

        return APIResponse(
            success=True,
            data=result,
            message=f"Service '{service_name}' information retrieved successfully"
        )
    except HTTPException:
        raise
    except Exception as e:
        # 如果是服务不存在的错误，返回404
        error_msg = str(e).lower()
        if "not found" in error_msg or "does not exist" in error_msg:
            raise HTTPException(status_code=404, detail=f"Service '{service_name}' not found")
        else:
            raise HTTPException(status_code=500, detail=f"Failed to get service info: {str(e)}")

# === Agent 级别服务信息查询 ===
@router.post("/for_agent/{agent_id}/get_service_info", response_model=APIResponse)
@handle_exceptions
async def agent_get_service_info(agent_id: str, request: Dict[str, str]):
    """Agent 级别获取服务信息"""
    validate_agent_id(agent_id)
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        result = await store.for_agent(agent_id).get_service_info(service_name)

        # 检查服务是否存在 - 主要检查service字段是否为None
        if (not result or
            (hasattr(result, 'service') and result.service is None) or
            (isinstance(result, dict) and result.get('service') is None)):
            raise HTTPException(status_code=404, detail=f"Service '{service_name}' not found for agent '{agent_id}'")

        return APIResponse(
            success=True,
            data=result,
            message=f"Service '{service_name}' information retrieved successfully for agent '{agent_id}'"
        )
    except HTTPException:
        raise
    except Exception as e:
        # 如果是服务不存在的错误，返回404
        error_msg = str(e).lower()
        if "not found" in error_msg or "does not exist" in error_msg:
            raise HTTPException(status_code=404, detail=f"Service '{service_name}' not found for agent '{agent_id}'")
        else:
            raise HTTPException(status_code=500, detail=f"Failed to get service info for agent '{agent_id}': {str(e)}")

# === Store 级别配置管理 ===
@router.get("/for_store/get_config", response_model=APIResponse)
@handle_exceptions
async def store_get_config():
    """Store 级别获取配置"""
    return store.get_json_config()

@router.get("/for_store/show_mcpconfig", response_model=APIResponse)
@handle_exceptions
async def store_show_mcpconfig():
    """Store 级别查看MCP配置"""
    try:
        config = store.for_store().show_mcpconfig()
        return APIResponse(
            success=True,
            data=config,
            message="Store MCP configuration retrieved successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get Store MCP configuration: {str(e)}"
        )

@router.post("/for_store/update_config", response_model=APIResponse)
@handle_exceptions
async def store_update_config(payload: JsonUpdateRequest):
    """Store 级别更新配置"""
    if not payload.config:
        raise HTTPException(status_code=400, detail="Config is required")
    return await store.update_json_service(payload)

@router.get("/for_store/validate_config", response_model=APIResponse)
@handle_exceptions
async def store_validate_config():
    """Store 级别验证配置有效性"""
    try:
        config = store.get_json_config()
        is_valid = bool(config and isinstance(config, dict))

        return APIResponse(
            success=is_valid,
            data={
                "valid": is_valid,
                "config": config
            },
            message="Configuration is valid" if is_valid else "Configuration is invalid"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"valid": False},
            message=f"Configuration validation failed: {str(e)}"
        )

@router.post("/for_store/reload_config", response_model=APIResponse)
@handle_exceptions
async def store_reload_config():
    """Store 级别重新加载配置"""
    try:
        await store.orchestrator.refresh_services()
        return APIResponse(
            success=True,
            data=True,
            message="Configuration reloaded successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to reload configuration: {str(e)}"
        )

# === Agent 级别配置管理 ===
@router.get("/for_agent/{agent_id}/get_config", response_model=APIResponse)
@handle_exceptions
async def agent_get_config(agent_id: str):
    """Agent 级别获取配置"""
    validate_agent_id(agent_id)
    try:
        config = store.get_json_config(agent_id)
        return APIResponse(
            success=True,
            data=config,
            message=f"Configuration retrieved successfully for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get configuration for agent '{agent_id}': {str(e)}"
        )

@router.get("/for_agent/{agent_id}/show_mcpconfig", response_model=APIResponse)
@handle_exceptions
async def agent_show_mcpconfig(agent_id: str):
    """Agent 级别查看MCP配置"""
    validate_agent_id(agent_id)
    try:
        config = store.for_agent(agent_id).show_mcpconfig()
        return APIResponse(
            success=True,
            data=config,
            message=f"Agent MCP configuration retrieved successfully for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get Agent MCP configuration for agent '{agent_id}': {str(e)}"
        )

@router.post("/for_agent/{agent_id}/update_config", response_model=APIResponse)
@handle_exceptions
async def agent_update_config(agent_id: str, payload: JsonUpdateRequest):
    """Agent 级别更新配置"""
    validate_agent_id(agent_id)
    if not payload.config:
        raise HTTPException(status_code=400, detail="Config is required")
    payload.client_id = agent_id  # 确保使用正确的agent_id
    return await store.update_json_service(payload)

@router.get("/for_agent/{agent_id}/validate_config", response_model=APIResponse)
@handle_exceptions
async def agent_validate_config(agent_id: str):
    """Agent 级别验证配置有效性"""
    validate_agent_id(agent_id)
    try:
        config = store.get_json_config(agent_id)
        is_valid = bool(config and isinstance(config, dict))

        return APIResponse(
            success=is_valid,
            data={
                "valid": is_valid,
                "config": config
            },
            message="Configuration is valid" if is_valid else "Configuration is invalid"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={"valid": False},
            message=f"Configuration validation failed: {str(e)}"
        )

# === Store 级别统计和监控 ===
@router.get("/for_store/get_stats", response_model=APIResponse)
@handle_exceptions
async def store_get_stats():
    """Store 级别获取系统统计信息"""
    try:
        context = store.for_store()

        # 获取服务列表和健康状态
        services = await context.list_services()
        health_check = await context.check_services()
        tools = await context.list_tools()

        # 统计信息
        total_services = len(services) if services else 0
        healthy_services = 0
        unhealthy_services = 0

        if isinstance(health_check, dict) and "services" in health_check:
            for service in health_check["services"]:
                if service.get("status") == "healthy":
                    healthy_services += 1
                else:
                    unhealthy_services += 1

        total_tools = len(tools) if tools else 0

        # 按传输类型分组服务
        transport_stats = {}
        if services:
            for service in services:
                transport = getattr(service, 'transport_type', 'unknown')
                transport_name = transport.value if hasattr(transport, 'value') else str(transport)
                transport_stats[transport_name] = transport_stats.get(transport_name, 0) + 1

        stats = {
            "services": {
                "total": total_services,
                "healthy": healthy_services,
                "unhealthy": unhealthy_services,
                "by_transport": transport_stats
            },
            "tools": {
                "total": total_tools
            },
            "system": {
                "orchestrator_status": health_check.get("orchestrator_status", "unknown") if isinstance(health_check, dict) else "unknown",
                "context": "store"
            }
        }

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

# === Agent 级别统计和监控 ===
@router.get("/for_agent/{agent_id}/get_stats", response_model=APIResponse)
@handle_exceptions
async def agent_get_stats(agent_id: str):
    """Agent 级别获取系统统计信息"""
    validate_agent_id(agent_id)
    try:
        context = store.for_agent(agent_id)

        # 获取服务列表和健康状态
        services = await context.list_services()
        health_check = await context.check_services()
        tools = await context.list_tools()

        # 统计信息
        total_services = len(services) if services else 0
        healthy_services = 0
        unhealthy_services = 0

        if isinstance(health_check, dict) and "services" in health_check:
            for service in health_check["services"]:
                if service.get("status") == "healthy":
                    healthy_services += 1
                else:
                    unhealthy_services += 1

        total_tools = len(tools) if tools else 0

        # 按传输类型分组服务
        transport_stats = {}
        if services:
            for service in services:
                transport = getattr(service, 'transport_type', 'unknown')
                transport_name = transport.value if hasattr(transport, 'value') else str(transport)
                transport_stats[transport_name] = transport_stats.get(transport_name, 0) + 1

        stats = {
            "services": {
                "total": total_services,
                "healthy": healthy_services,
                "unhealthy": unhealthy_services,
                "by_transport": transport_stats
            },
            "tools": {
                "total": total_tools
            },
            "system": {
                "orchestrator_status": health_check.get("orchestrator_status", "unknown") if isinstance(health_check, dict) else "unknown",
                "context": "agent",
                "agent_id": agent_id
            }
        }

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

# === Store 级别服务状态查询 ===
@router.post("/for_store/get_service_status", response_model=APIResponse)
@handle_exceptions
async def store_get_service_status(request: Dict[str, str]):
    """Store 级别获取服务详细状态信息"""
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        context = store.for_store()

        # 获取服务信息
        service_info = await context.get_service_info(service_name)
        if not service_info:
            raise HTTPException(status_code=404, detail=f"Service {service_name} not found")

        # 获取健康状态
        health_check = await context.check_services()
        service_health = None

        if isinstance(health_check, dict) and "services" in health_check:
            for service in health_check["services"]:
                if service.get("name") == service_name:
                    service_health = service
                    break

        # 获取工具列表
        tools = await context.list_tools()
        service_tools = [tool for tool in tools if getattr(tool, 'service_name', '') == service_name] if tools else []

        status_info = {
            "service": service_info,
            "health": service_health,
            "tools": {
                "count": len(service_tools),
                "list": service_tools
            },
            "last_check": health_check.get("timestamp") if isinstance(health_check, dict) else None
        }

        return APIResponse(
            success=True,
            data=status_info,
            message=f"Service {service_name} status retrieved successfully"
        )
    except HTTPException:
        raise
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get service status: {str(e)}"
        )

# === Agent 级别服务状态查询 ===
@router.post("/for_agent/{agent_id}/get_service_status", response_model=APIResponse)
@handle_exceptions
async def agent_get_service_status(agent_id: str, request: Dict[str, str]):
    """Agent 级别获取服务详细状态信息"""
    validate_agent_id(agent_id)
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        context = store.for_agent(agent_id)

        # 获取服务信息
        service_info = await context.get_service_info(service_name)
        if not service_info:
            raise HTTPException(status_code=404, detail=f"Service {service_name} not found")

        # 获取健康状态
        health_check = await context.check_services()
        service_health = None

        if isinstance(health_check, dict) and "services" in health_check:
            for service in health_check["services"]:
                if service.get("name") == service_name:
                    service_health = service
                    break

        # 获取工具列表
        tools = await context.list_tools()
        service_tools = [tool for tool in tools if getattr(tool, 'service_name', '') == service_name] if tools else []

        status_info = {
            "service": service_info,
            "health": service_health,
            "tools": {
                "count": len(service_tools),
                "list": service_tools
            },
            "last_check": health_check.get("timestamp") if isinstance(health_check, dict) else None
        }

        return APIResponse(
            success=True,
            data=status_info,
            message=f"Service {service_name} status retrieved successfully"
        )
    except HTTPException:
        raise
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get service status: {str(e)}"
        )

# === Store 级别健康检查 ===
@router.get("/for_store/health", response_model=APIResponse)
@handle_exceptions
async def store_health_check():
    """Store 级别系统健康检查"""
    try:
        # 检查Store级别健康状态
        store_health = await store.for_store().check_services()

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

        # 判断整体健康状态
        is_healthy = True
        if isinstance(store_health, dict):
            if store_health.get("orchestrator_status") != "running":
                is_healthy = False

            services = store_health.get("services", [])
            if services:
                unhealthy_count = sum(1 for s in services if s.get("status") != "healthy")
                if unhealthy_count > 0:
                    health_info["system"]["unhealthy_services"] = unhealthy_count
                    # 如果有不健康的服务，但系统仍在运行，标记为degraded
                    if is_healthy:
                        health_info["status"] = "degraded"
        else:
            is_healthy = False

        if not is_healthy:
            health_info["status"] = "unhealthy"

        return APIResponse(
            success=is_healthy,
            data=health_info,
            message=f"System status: {health_info['status']}"
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

# === Agent 级别健康检查 ===
@router.get("/for_agent/{agent_id}/health", response_model=APIResponse)
@handle_exceptions
async def agent_health_check(agent_id: str):
    """Agent 级别系统健康检查"""
    validate_agent_id(agent_id)
    try:
        # 检查Agent级别健康状态
        agent_health = await store.for_agent(agent_id).check_services()

        # 基本系统信息
        health_info = {
            "status": "healthy",
            "timestamp": agent_health.get("timestamp") if isinstance(agent_health, dict) else None,
            "agent": agent_health,
            "system": {
                "api_version": "0.2.0",
                "store_initialized": bool(store),
                "orchestrator_status": agent_health.get("orchestrator_status", "unknown") if isinstance(agent_health, dict) else "unknown",
                "context": "agent",
                "agent_id": agent_id
            }
        }

        # 判断整体健康状态
        is_healthy = True
        if isinstance(agent_health, dict):
            if agent_health.get("orchestrator_status") != "running":
                is_healthy = False

            services = agent_health.get("services", [])
            if services:
                unhealthy_count = sum(1 for s in services if s.get("status") != "healthy")
                if unhealthy_count > 0:
                    health_info["system"]["unhealthy_services"] = unhealthy_count
                    # 如果有不健康的服务，但系统仍在运行，标记为degraded
                    if is_healthy:
                        health_info["status"] = "degraded"
        else:
            is_healthy = False

        if not is_healthy:
            health_info["status"] = "unhealthy"

        return APIResponse(
            success=is_healthy,
            data=health_info,
            message=f"System status: {health_info['status']}"
        )

    except Exception as e:
        return APIResponse(
            success=False,
            data={
                "status": "unhealthy",
                "error": str(e),
                "context": "agent",
                "agent_id": agent_id
            },
            message=f"Health check failed: {str(e)}"
        )

# === Store 级别重置配置 ===
@router.post("/for_store/reset_config", response_model=APIResponse)
@handle_exceptions
async def store_reset_config():
    """Store 级别重置配置"""
    try:
        success = await store.for_store().reset_config()
        return APIResponse(
            success=success,
            data=success,
            message="Store configuration reset successfully" if success else "Failed to reset store configuration"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to reset store configuration: {str(e)}"
        )

@router.post("/for_store/reset_json_config", response_model=APIResponse)
@handle_exceptions
async def store_reset_json_config():
    """Store 级别重置JSON配置文件"""
    try:
        success = await store.for_store().reset_json_config()
        return APIResponse(
            success=success,
            data=success,
            message="JSON configuration reset successfully" if success else "Failed to reset JSON configuration"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to reset JSON configuration: {str(e)}"
        )

@router.post("/for_store/restore_default_config", response_model=APIResponse)
@handle_exceptions
async def store_restore_default_config():
    """Store 级别恢复默认配置"""
    try:
        success = await store.for_store().restore_default_config()
        return APIResponse(
            success=success,
            data=success,
            message="Default configuration restored successfully" if success else "Failed to restore default configuration"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to restore default configuration: {str(e)}"
        )

# === Agent 级别重置配置 ===
@router.post("/for_agent/{agent_id}/reset_config", response_model=APIResponse)
@handle_exceptions
async def agent_reset_config(agent_id: str):
    """Agent 级别重置配置"""
    validate_agent_id(agent_id)
    try:
        success = await store.for_agent(agent_id).reset_config()
        return APIResponse(
            success=success,
            data=success,
            message=f"Agent {agent_id} configuration reset successfully" if success else f"Failed to reset agent {agent_id} configuration"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=f"Failed to reset agent {agent_id} configuration: {str(e)}"
        )

# === 监控状态API ===
@router.get("/monitoring/status", response_model=APIResponse)
@handle_exceptions
async def get_monitoring_status():
    """获取监控系统状态"""
    try:
        orchestrator = store.orchestrator

        # 获取监控任务状态
        heartbeat_active = orchestrator.heartbeat_task and not orchestrator.heartbeat_task.done()
        reconnection_active = orchestrator.reconnection_task and not orchestrator.reconnection_task.done()
        cleanup_active = orchestrator.cleanup_task and not orchestrator.cleanup_task.done()

        # 获取智能重连队列状态
        reconnection_status = orchestrator.smart_reconnection.get_queue_status()

        # 获取服务统计
        total_services = 0
        healthy_services = 0
        for client_id, services in orchestrator.registry.sessions.items():
            total_services += len(services)
            for service_name in services:
                if await orchestrator.is_service_healthy(service_name, client_id):
                    healthy_services += 1

        status_data = {
            "monitoring_tasks": {
                "heartbeat_active": heartbeat_active,
                "reconnection_active": reconnection_active,
                "cleanup_active": cleanup_active,
                "heartbeat_interval_seconds": orchestrator.heartbeat_interval.total_seconds(),
                "reconnection_interval_seconds": orchestrator.reconnection_interval.total_seconds(),
                "cleanup_interval_seconds": orchestrator.cleanup_interval.total_seconds()
            },
            "service_statistics": {
                "total_services": total_services,
                "healthy_services": healthy_services,
                "unhealthy_services": total_services - healthy_services,
                "health_percentage": round((healthy_services / total_services * 100) if total_services > 0 else 0, 2)
            },
            "reconnection_queue": reconnection_status,
            "resource_limits": {
                "max_reconnection_queue_size": orchestrator.max_reconnection_queue_size,
                "max_heartbeat_history_hours": orchestrator.max_heartbeat_history_hours,
                "http_timeout_seconds": orchestrator.http_timeout
            }
        }

        return APIResponse(
            success=True,
            data=status_data,
            message="Monitoring status retrieved successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get monitoring status: {str(e)}"
        )

@router.post("/monitoring/config", response_model=APIResponse)
@handle_exceptions
async def update_monitoring_config(config: MonitoringConfig):
    """更新监控配置"""
    try:
        orchestrator = store.orchestrator
        updated_fields = []

        # 更新心跳间隔
        if config.heartbeat_interval_seconds is not None:
            orchestrator.heartbeat_interval = timedelta(seconds=config.heartbeat_interval_seconds)
            updated_fields.append(f"heartbeat_interval: {config.heartbeat_interval_seconds}s")

        # 更新重连间隔
        if config.reconnection_interval_seconds is not None:
            orchestrator.reconnection_interval = timedelta(seconds=config.reconnection_interval_seconds)
            updated_fields.append(f"reconnection_interval: {config.reconnection_interval_seconds}s")

        # 更新清理间隔
        if config.cleanup_interval_hours is not None:
            orchestrator.cleanup_interval = timedelta(hours=config.cleanup_interval_hours)
            updated_fields.append(f"cleanup_interval: {config.cleanup_interval_hours}h")

        # 更新重连队列大小
        if config.max_reconnection_queue_size is not None:
            orchestrator.max_reconnection_queue_size = config.max_reconnection_queue_size
            updated_fields.append(f"max_reconnection_queue_size: {config.max_reconnection_queue_size}")

        # 更新心跳历史保留时间
        if config.max_heartbeat_history_hours is not None:
            orchestrator.max_heartbeat_history_hours = config.max_heartbeat_history_hours
            updated_fields.append(f"max_heartbeat_history_hours: {config.max_heartbeat_history_hours}h")

        # 更新HTTP超时时间
        if config.http_timeout_seconds is not None:
            orchestrator.http_timeout = config.http_timeout_seconds
            updated_fields.append(f"http_timeout: {config.http_timeout_seconds}s")

        if not updated_fields:
            return APIResponse(
                success=True,
                data={},
                message="No configuration changes provided"
            )

        # 重启监控任务以应用新配置
        await orchestrator._restart_monitoring_tasks()

        return APIResponse(
            success=True,
            data={"updated_fields": updated_fields},
            message=f"Monitoring configuration updated: {', '.join(updated_fields)}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to update monitoring configuration: {str(e)}"
        )

@router.post("/monitoring/restart", response_model=APIResponse)
@handle_exceptions
async def restart_monitoring():
    """重启监控任务"""
    try:
        orchestrator = store.orchestrator

        # 停止现有任务
        tasks_to_stop = [
            ("heartbeat", orchestrator.heartbeat_task),
            ("reconnection", orchestrator.reconnection_task),
            ("cleanup", orchestrator.cleanup_task)
        ]

        stopped_tasks = []
        for task_name, task in tasks_to_stop:
            if task and not task.done():
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    pass
                stopped_tasks.append(task_name)

        # 重新启动监控
        await orchestrator.start_monitoring()

        return APIResponse(
            success=True,
            data={"restarted_tasks": stopped_tasks},
            message=f"Monitoring tasks restarted: {', '.join(stopped_tasks) if stopped_tasks else 'all tasks'}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to restart monitoring: {str(e)}"
        )

# === 批量操作API ===
@router.post("/for_store/batch_update_services", response_model=APIResponse)
@handle_exceptions
async def store_batch_update_services(request: Dict[str, List[Dict]]):
    """Store级别批量更新服务配置"""
    services = request.get("services", [])
    if not services:
        raise HTTPException(status_code=400, detail="Services list is required")

    try:
        context = store.for_store()
        results = []

        for service_config in services:
            service_name = service_config.get("name")
            if not service_name:
                results.append({"name": "unknown", "success": False, "error": "Service name is required"})
                continue

            try:
                # 更新服务配置
                result = await context.update_service(service_name, service_config)
                results.append({"name": service_name, "success": True, "result": result})
            except Exception as e:
                results.append({"name": service_name, "success": False, "error": str(e)})

        success_count = sum(1 for r in results if r["success"])
        total_count = len(results)

        return APIResponse(
            success=success_count > 0,
            data={
                "results": results,
                "summary": {
                    "total": total_count,
                    "success": success_count,
                    "failed": total_count - success_count
                }
            },
            message=f"Batch update completed: {success_count}/{total_count} services updated successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Batch update failed: {str(e)}"
        )

@router.post("/for_store/batch_restart_services", response_model=APIResponse)
@handle_exceptions
async def store_batch_restart_services(request: Dict[str, List[str]]):
    """Store级别批量重启服务"""
    service_names = request.get("service_names", [])
    if not service_names:
        raise HTTPException(status_code=400, detail="Service names list is required")

    try:
        context = store.for_store()
        results = []

        for service_name in service_names:
            try:
                # 重启服务
                result = await context.restart_service(service_name)
                results.append({"name": service_name, "success": True, "result": result})
            except Exception as e:
                results.append({"name": service_name, "success": False, "error": str(e)})

        success_count = sum(1 for r in results if r["success"])
        total_count = len(results)

        return APIResponse(
            success=success_count > 0,
            data={
                "results": results,
                "summary": {
                    "total": total_count,
                    "success": success_count,
                    "failed": total_count - success_count
                }
            },
            message=f"Batch restart completed: {success_count}/{total_count} services restarted successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Batch restart failed: {str(e)}"
        )

@router.post("/for_store/batch_delete_services", response_model=APIResponse)
@handle_exceptions
async def store_batch_delete_services(request: Dict[str, List[str]]):
    """Store级别批量删除服务"""
    service_names = request.get("service_names", [])
    if not service_names:
        raise HTTPException(status_code=400, detail="Service names list is required")

    try:
        context = store.for_store()
        results = []

        for service_name in service_names:
            try:
                # 删除服务
                result = await context.delete_service(service_name)
                results.append({"name": service_name, "success": True, "result": result})
            except Exception as e:
                results.append({"name": service_name, "success": False, "error": str(e)})

        success_count = sum(1 for r in results if r["success"])
        total_count = len(results)

        return APIResponse(
            success=success_count > 0,
            data={
                "results": results,
                "summary": {
                    "total": total_count,
                    "success": success_count,
                    "failed": total_count - success_count
                }
            },
            message=f"Batch delete completed: {success_count}/{total_count} services deleted successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Batch delete failed: {str(e)}"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to restart monitoring: {str(e)}"
        )
