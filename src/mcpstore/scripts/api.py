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
from pydantic import BaseModel, Field
from mcpstore.core.models.common import (
    APIResponse, RegistrationResponse, ConfigResponse,
    ExecutionResponse
)
from mcpstore.core.monitoring import (
    ToolUsageStats, NetworkEndpoint, SystemResourceInfo
)
from typing import Optional, List, Dict, Any, Union
from pydantic import BaseModel, ValidationError, Field
import logging
import traceback
from functools import wraps
from datetime import timedelta
import asyncio
import time

# 创建logger实例
logger = logging.getLogger(__name__)

# === 监控相关的响应模型 ===

class ToolUsageStatsResponse(BaseModel):
    """工具使用统计响应"""
    tool_name: str = Field(description="工具名称")
    service_name: str = Field(description="服务名称")
    execution_count: int = Field(description="执行次数")
    last_executed: Optional[str] = Field(description="最后执行时间")
    average_response_time: float = Field(description="平均响应时间")
    success_rate: float = Field(description="成功率")

class NetworkEndpointResponse(BaseModel):
    """网络端点响应"""
    endpoint_name: str = Field(description="端点名称")
    url: str = Field(description="端点URL")
    status: str = Field(description="状态")
    response_time: float = Field(description="响应时间")
    last_checked: str = Field(description="最后检查时间")
    uptime_percentage: float = Field(description="可用性百分比")

class SystemResourceInfoResponse(BaseModel):
    """系统资源信息响应"""
    server_uptime: str = Field(description="服务器运行时间")
    memory_total: int = Field(description="总内存")
    memory_used: int = Field(description="已用内存")
    memory_percentage: float = Field(description="内存使用率")
    disk_usage_percentage: float = Field(description="磁盘使用率")
    network_traffic_in: int = Field(description="网络入流量")
    network_traffic_out: int = Field(description="网络出流量")

class AddAlertRequest(BaseModel):
    """添加告警请求"""
    type: str = Field(description="告警类型: warning, error, info")
    title: str = Field(description="告警标题")
    message: str = Field(description="告警消息")
    service_name: Optional[str] = Field(None, description="相关服务名称")

class NetworkEndpointCheckRequest(BaseModel):
    """网络端点检查请求"""
    endpoints: List[Dict[str, str]] = Field(description="端点列表")

# 简化的工具执行请求模型（用于API）
class SimpleToolExecutionRequest(BaseModel):
    tool_name: str = Field(..., description="工具名称")
    args: Dict[str, Any] = Field(default_factory=dict, description="工具参数")
    service_name: Optional[str] = Field(None, description="服务名称（可选，会自动推断）")

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

def monitor_api_performance(func):
    """API性能监控装饰器"""
    @wraps(func)
    async def wrapper(*args, **kwargs):
        start_time = time.time()

        # 获取store实例（从依赖注入中）
        store = None
        for arg in args:
            if isinstance(arg, MCPStore):
                store = arg
                break

        # 如果没有在args中找到，检查kwargs
        if store is None:
            store = kwargs.get('store')

        try:
            # 增加活跃连接数
            store = get_store()
            if store:
                store.for_store().increment_active_connections()

            result = await func(*args, **kwargs)

            # 记录API调用
            if store:
                response_time = (time.time() - start_time) * 1000  # 转换为毫秒
                store.for_store().record_api_call(response_time)

            return result
        finally:
            # 减少活跃连接数
            if store:
                store.for_store().decrement_active_connections()

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

# === 依赖注入函数 ===
def get_store() -> MCPStore:
    """获取MCPStore实例的依赖注入函数"""
    # 从api_app模块获取当前的store实例
    from .api_app import get_store as get_app_store
    return get_app_store()

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
    
    3. 命令方式添加服务（本地服务）：
       POST /for_store/add_service
       {
           "name": "assistant",
           "command": "python",
           "args": ["./assistant_server.py"],
           "env": {"DEBUG": "true"},
           "working_dir": "/path/to/service"
       }

       注意：本地服务需要确保：
       - 命令路径正确且可执行
       - 工作目录存在且有权限
       - 环境变量设置正确
    
    Returns:
        APIResponse: {
            "success": true/false,
            "data": true/false,  # 是否成功添加服务
            "message": "错误信息（如果有）"
        }
    """
    try:
        store = get_store()
        store = get_store()

        context = store.for_store()

        # 1. 空参数注册
        if not payload:
            result = await context.add_service_async()
            success = result is not None
            return APIResponse(
                success=success,
                data=success,
                message="Successfully registered all services" if success else "Failed to register services"
            )

        # 2/3. 配置方式添加服务 - 直接使用SDK的详细处理方法
        # SDK已经包含了所有业务逻辑：配置验证、transport推断、服务名解析等
        result = await context.add_service_with_details_async(payload)

        # 直接返回SDK处理的结果，只需要包装成APIResponse格式
        return APIResponse(
            success=result["success"],
            data={
                "added_services": result["added_services"],
                "failed_services": result["failed_services"],
                "service_details": result["service_details"],
                "total_services": result["total_services"],
                "total_tools": result["total_tools"]
            },
            message=result["message"]
        )

    except Exception as e:
        logger.error(f"Failed to add service: {str(e)}")
        logger.error(f"Traceback: {traceback.format_exc()}")
        raise HTTPException(status_code=500, detail=f"Failed to add service: {str(e)}")

@router.get("/for_store/list_services", response_model=APIResponse)
@handle_exceptions
async def store_list_services():
    """Store 级别获取服务列表"""
    try:
        store = get_store()
        store = get_store()

        context = store.for_store()
        services = context.list_services()

        return APIResponse(
            success=True,
            data=services,
            message=f"Retrieved {len(services)} services successfully"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to retrieve services: {str(e)}"
        )

@router.get("/for_store/list_tools", response_model=APIResponse)
@handle_exceptions
async def store_list_tools():
    """Store 级别获取工具列表"""
    try:
        store = get_store()
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

@router.get("/for_store/check_services", response_model=APIResponse)
@handle_exceptions
async def store_check_services():
    """Store 级别健康检查"""
    try:
        store = get_store()
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

@router.post("/for_store/use_tool", response_model=APIResponse)
@handle_exceptions
async def store_use_tool(request: SimpleToolExecutionRequest):
    """Store 级别使用工具"""
    if not request.tool_name or not isinstance(request.tool_name, str):
        raise HTTPException(status_code=400, detail="tool_name is required and must be a string")
    if request.args is None or not isinstance(request.args, dict):
        raise HTTPException(status_code=400, detail="args is required and must be a dictionary")

    try:
        import time
        import uuid

        # 记录执行开始时间
        start_time = time.time()
        trace_id = str(uuid.uuid4())[:8]

        # 🔧 直接使用SDK的use_tool_async方法，它已经包含了完整的工具解析逻辑
        # SDK会自动处理：工具名称解析、服务推断、格式转换等
        store = get_store()
        store = get_store()

        store = get_store()


        result = await store.for_store().use_tool_async(request.tool_name, request.args)

        # 计算执行时间
        duration_ms = int((time.time() - start_time) * 1000)

        # 📊 记录工具执行统计
        try:
            # 从工具名提取服务名
            service_name = request.tool_name.split('_')[0] if '_' in request.tool_name else 'unknown'

            # 判断执行是否成功
            success = True
            if hasattr(result, 'is_error') and result.is_error:
                success = False
            elif isinstance(result, dict) and result.get('error'):
                success = False

            # 记录工具执行（store已在函数开头获取）
            store.for_store().record_tool_execution(
                request.tool_name,
                service_name,
                duration_ms,
                success
            )
        except Exception as e:
            # 监控记录失败不应该影响工具执行
            logger.warning(f"Failed to record tool execution: {e}")

        # 提取实际结果（SDK返回的是FastMCP标准结果）
        actual_result = result.result if hasattr(result, 'result') else result

        return APIResponse(
            success=True,
            data=actual_result,
            execution_info={
                "duration_ms": duration_ms,
                "tool_version": "1.0.0",
                "service_name": "auto-resolved",  # SDK已经处理了服务解析
                "trace_id": trace_id
            },
            message=f"Tool '{request.tool_name}' executed successfully"
        )
    except HTTPException:
        raise
    except Exception as e:
        # 📊 记录失败的工具执行
        try:
            duration_ms = int((time.time() - start_time) * 1000)
            service_name = request.tool_name.split('_')[0] if '_' in request.tool_name else 'unknown'

            # 获取store实例记录失败的工具执行
            store = get_store()
            store.for_store().record_tool_execution(
                request.tool_name,
                service_name,
                duration_ms,
                False  # 执行失败
            )
        except Exception as monitor_error:
            logger.warning(f"Failed to record failed tool execution: {monitor_error}")

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
        store = get_store()
        context = store.for_agent(agent_id)
        
        # 直接使用SDK的详细处理方法，支持所有格式
        # SDK已经包含了所有业务逻辑：配置验证、transport推断、服务名解析等
        result = await context.add_service_with_details_async(payload)

        # 直接返回SDK处理的结果，只需要包装成APIResponse格式
        return APIResponse(
            success=result["success"],
            data={
                "added_services": result["added_services"],
                "failed_services": result["failed_services"],
                "service_details": result["service_details"],
                "total_services": result["total_services"],
                "total_tools": result["total_tools"]
            },
            message=result["message"]
        )

        
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to add service for agent '{agent_id}': {str(e)}")

@router.get("/for_agent/{agent_id}/list_services", response_model=APIResponse)
@handle_exceptions
async def agent_list_services(agent_id: str):
    """Agent 级别获取服务列表"""
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)
        services = await context.list_services()

        return APIResponse(
            success=True,
            data=services,
            message=f"Retrieved {len(services)} services for agent '{agent_id}'"
        )
    except Exception as e:
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to retrieve services for agent '{agent_id}': {str(e)}"
        )

@router.get("/for_agent/{agent_id}/list_tools", response_model=APIResponse)
@handle_exceptions
async def agent_list_tools(agent_id: str):
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

@router.get("/for_agent/{agent_id}/check_services", response_model=APIResponse)
@handle_exceptions
async def agent_check_services(agent_id: str):
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

@router.post("/for_agent/{agent_id}/use_tool", response_model=APIResponse)
@handle_exceptions
async def agent_use_tool(agent_id: str, request: SimpleToolExecutionRequest):
    """Agent 级别使用工具"""
    validate_agent_id(agent_id)
    if not request.tool_name or not isinstance(request.tool_name, str):
        raise HTTPException(status_code=400, detail="tool_name is required and must be a string")
    if request.args is None or not isinstance(request.args, dict):
        raise HTTPException(status_code=400, detail="args is required and must be a dictionary")

    try:
        import time
        import uuid

        # 记录执行开始时间
        start_time = time.time()
        trace_id = str(uuid.uuid4())[:8]

        # 🔧 直接使用SDK的use_tool_async方法，它已经包含了完整的工具解析逻辑
        store = get_store()
        result = await store.for_agent(agent_id).use_tool_async(request.tool_name, request.args)

        # 计算执行时间
        duration_ms = int((time.time() - start_time) * 1000)

        # 📊 记录工具执行统计
        try:
            # 从工具名提取服务名
            service_name = request.tool_name.split('_')[0] if '_' in request.tool_name else 'unknown'

            # 判断执行是否成功
            success = True
            if hasattr(result, 'is_error') and result.is_error:
                success = False
            elif isinstance(result, dict) and result.get('error'):
                success = False

            # 记录工具执行
            store = get_store()

            store.for_agent(agent_id).record_tool_execution(
                request.tool_name,
                service_name,
                duration_ms,
                success
            )
        except Exception as e:
            # 监控记录失败不应该影响工具执行
            logger.warning(f"Failed to record tool execution for agent {agent_id}: {e}")

        # 提取实际结果
        actual_result = result.result if hasattr(result, 'result') else result

        return APIResponse(
            success=True,
            data=actual_result,
            execution_info={
                "duration_ms": duration_ms,
                "tool_version": "1.0.0",
                "service_name": "auto-resolved",  # SDK已经处理了服务解析
                "agent_id": agent_id,
                "trace_id": trace_id
            },
            message=f"Tool '{request.tool_name}' executed successfully for agent '{agent_id}'"
        )
    except HTTPException:
        raise
    except Exception as e:
        # 📊 记录失败的工具执行
        try:
            duration_ms = int((time.time() - start_time) * 1000)
            service_name = request.tool_name.split('_')[0] if '_' in request.tool_name else 'unknown'

            store = get_store()


            store.for_agent(agent_id).record_tool_execution(
                request.tool_name,
                service_name,
                duration_ms,
                False  # 执行失败
            )
        except Exception as monitor_error:
            logger.warning(f"Failed to record failed tool execution for agent {agent_id}: {monitor_error}")

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
        return await store.for_agent(agent_id).get_service_info_async(name)
    return await store.for_store().get_service_info_async(name)

# === Store 级别服务管理操作 ===
@router.post("/for_store/delete_service", response_model=APIResponse)
@handle_exceptions
async def store_delete_service(request: Dict[str, str]):
    """Store 级别删除服务"""
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        store = get_store()

        store = get_store()


        result = await store.for_store().delete_service_async(service_name)
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
        store = get_store()

        store = get_store()


        result = await store.for_store().update_service_async(service_name, config)
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
        store = get_store()

        context = store.for_store()

        # 获取服务配置
        service_info = await context.get_service_info_async(service_name)
        if not service_info:
            raise HTTPException(status_code=404, detail=f"Service {service_name} not found")

        # 删除服务
        delete_result = await context.delete_service_async(service_name)
        if not delete_result:
            raise HTTPException(status_code=500, detail=f"Failed to stop service {service_name}")

        # 重新添加服务
        add_result = await context.add_service_async([service_name])

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
        result = await store.for_agent(agent_id).delete_service_async(service_name)
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
        result = await store.for_agent(agent_id).update_service_async(service_name, config)
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
        service_info = await context.get_service_info_async(service_name)
        if not service_info:
            raise HTTPException(status_code=404, detail=f"Service {service_name} not found")

        # 删除服务
        delete_result = await context.delete_service_async(service_name)
        if not delete_result:
            raise HTTPException(status_code=500, detail=f"Failed to stop service {service_name}")

        # 重新添加服务
        add_result = await context.add_service_async([service_name])

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

    store = get_store()


    context = store.for_store()
    results = []

    for i, service in enumerate(services):
        try:
            if isinstance(service, str):
                # 服务名方式
                result = await context.add_service_async([service])
            elif isinstance(service, dict):
                # 配置方式
                result = await context.add_service_async(service)
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

    store = get_store()


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
            result = await context.update_service_async(name, config)
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
    # 使用SDK的批量操作方法
    result = await context.batch_add_services_async(services)

    return APIResponse(
        success=result["success"],
        data={
            "results": result["results"],
            "summary": result["summary"]
        },
        message=result["message"]
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
            result = await context.update_service_async(name, config)
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
        store = get_store()

        store = get_store()


        result = await store.for_store().get_service_info_async(service_name)

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
        result = await store.for_agent(agent_id).get_service_info_async(service_name)

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
    store = get_store()

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
    store = get_store()

    return await store.update_json_service(payload)

@router.get("/for_store/validate_config", response_model=APIResponse)
@handle_exceptions
async def store_validate_config():
    """Store 级别验证配置有效性"""
    try:
        store = get_store()

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
        store = get_store()

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
    store = get_store()

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

# === Agent 级别统计和监控 ===
@router.get("/for_agent/{agent_id}/get_stats", response_model=APIResponse)
@handle_exceptions
async def agent_get_stats(agent_id: str):
    """Agent 级别获取系统统计信息"""
    validate_agent_id(agent_id)
    try:
        context = store.for_agent(agent_id)
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

# === Store 级别服务状态查询 ===
@router.post("/for_store/get_service_status", response_model=APIResponse)
@handle_exceptions
async def store_get_service_status(request: Dict[str, str]):
    """Store 级别获取服务详细状态信息"""
    service_name = request.get("name")
    if not service_name:
        raise HTTPException(status_code=400, detail="Service name is required")

    try:
        store = get_store()

        context = store.for_store()

        # 获取服务信息
        service_info = await context.get_service_info_async(service_name)
        if not service_info:
            raise HTTPException(status_code=404, detail=f"Service {service_name} not found")

        # 获取健康状态
        health_check = await context.check_services_async()
        service_health = None

        if isinstance(health_check, dict) and "services" in health_check:
            for service in health_check["services"]:
                if service.get("name") == service_name:
                    service_health = service
                    break

        # 获取工具列表
        tools = await context.list_tools_async()
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
        service_info = await context.get_service_info_async(service_name)
        if not service_info:
            raise HTTPException(status_code=404, detail=f"Service {service_name} not found")

        # 获取健康状态
        health_check = await context.check_services_async()
        service_health = None

        if isinstance(health_check, dict) and "services" in health_check:
            for service in health_check["services"]:
                if service.get("name") == service_name:
                    service_health = service
                    break

        # 获取工具列表
        tools = await context.list_tools_async()
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
        store = get_store()
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
        store = get_store()

        store = get_store()


        success = await store.for_store().reset_config_async()
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

# === Store 级别文件直接重置 ===
@router.post("/for_store/reset_mcp_json_file", response_model=APIResponse)
@handle_exceptions
async def store_reset_mcp_json_file():
    """Store 级别直接重置MCP JSON配置文件"""
    try:
        store = get_store()

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

@router.post("/for_store/reset_client_services_file", response_model=APIResponse)
@handle_exceptions
async def store_reset_client_services_file():
    """Store 级别直接重置client_services.json文件"""
    try:
        store = get_store()

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

@router.post("/for_store/reset_agent_clients_file", response_model=APIResponse)
@handle_exceptions
async def store_reset_agent_clients_file():
    """Store 级别直接重置agent_clients.json文件"""
    try:
        store = get_store()

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


# === Agent 级别重置配置 ===
@router.post("/for_agent/{agent_id}/reset_config", response_model=APIResponse)
@handle_exceptions
async def agent_reset_config(agent_id: str):
    """Agent 级别重置配置"""
    validate_agent_id(agent_id)
    try:
        success = await store.for_agent(agent_id).reset_config_async()
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
        store = get_store()

        context = store.for_store()
        results = []

        for service_config in services:
            service_name = service_config.get("name")
            if not service_name:
                results.append({"name": "unknown", "success": False, "error": "Service name is required"})
                continue

            try:
                # 更新服务配置
                result = await context.update_service_async(service_name, service_config)
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
        store = get_store()

        context = store.for_store()
        results = []

        for service_name in service_names:
            try:
                # 重启服务
                result = context.restart_service(service_name)
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
        store = get_store()

        context = store.for_store()
        results = []

        for service_name in service_names:
            try:
                # 删除服务
                result = await context.delete_service_async(service_name)
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

# === 监控和统计API ===





@router.get("/for_store/tool_usage_stats", response_model=APIResponse)
async def get_store_tool_usage_stats(limit: int = 10, store: MCPStore = Depends(get_store)):
    """获取Store级别的工具使用统计"""
    try:
        store = get_store()

        stats = await store.for_store().get_tool_usage_stats_async(limit)

        stats_data = [
            ToolUsageStatsResponse(
                tool_name=stat.tool_name,
                service_name=stat.service_name,
                execution_count=stat.execution_count,
                last_executed=stat.last_executed,
                average_response_time=stat.average_response_time,
                success_rate=stat.success_rate
            ).dict() for stat in stats
        ]

        return APIResponse(
            success=True,
            data=stats_data,
            message="Tool usage statistics retrieved successfully"
        )
    except Exception as e:
        logger.error(f"Failed to get tool usage stats: {e}")
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to get tool usage stats: {str(e)}"
        )

@router.get("/for_agent/{agent_id}/tool_usage_stats", response_model=APIResponse)
async def get_agent_tool_usage_stats(agent_id: str, limit: int = 10, store: MCPStore = Depends(get_store)):
    """获取Agent级别的工具使用统计"""
    try:
        validate_agent_id(agent_id)
        stats = await store.for_agent(agent_id).get_tool_usage_stats_async(limit)

        stats_data = [
            ToolUsageStatsResponse(
                tool_name=stat.tool_name,
                service_name=stat.service_name,
                execution_count=stat.execution_count,
                last_executed=stat.last_executed,
                average_response_time=stat.average_response_time,
                success_rate=stat.success_rate
            ).dict() for stat in stats
        ]

        return APIResponse(
            success=True,
            data=stats_data,
            message=f"Agent '{agent_id}' tool usage statistics retrieved successfully"
        )
    except Exception as e:
        logger.error(f"Failed to get agent tool usage stats: {e}")
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to get agent tool usage stats: {str(e)}"
        )









@router.post("/for_store/network_check", response_model=APIResponse)
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
        logger.error(f"Failed to check network endpoints: {e}")
        return APIResponse(
            success=False,
            data=[],
            message=f"Failed to check network endpoints: {str(e)}"
        )

@router.get("/for_store/system_resources", response_model=APIResponse)
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
        logger.error(f"Failed to get system resources: {e}")
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get system resources: {str(e)}"
        )


