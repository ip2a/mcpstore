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
from pydantic import BaseModel
from functools import wraps

# === 统一响应模型 ===
# APIResponse 已移动到 common.py 中，通过导入使用

# === 工具函数 ===
def handle_exceptions(func):
    """统一的异常处理装饰器"""
    @wraps(func)
    async def wrapper(*args, **kwargs):
        try:
            result = await func(*args, **kwargs)
            return APIResponse(success=True, data=result)
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
            return APIResponse(
                success=True,
                data=result,
                message="Successfully registered all services" if result else "Failed to register services"
            )
        
        # 2/3. 配置方式添加服务
        if isinstance(payload, dict):
            if "name" not in payload:
                raise HTTPException(status_code=400, detail="Service name is required")
                
            if "url" in payload and "command" in payload:
                raise HTTPException(status_code=400, detail="Cannot specify both url and command")
                
            if "url" in payload and "transport" not in payload:
                raise HTTPException(status_code=400, detail="Transport type is required for URL-based service")
                
            if "command" in payload and not isinstance(payload.get("args", []), list):
                raise HTTPException(status_code=400, detail="Args must be a list")
                
            result = await context.add_service(payload)
            return APIResponse(
                success=True,
                data=result,
                message="Successfully added service" if result else "Failed to add service"
            )
        
        raise HTTPException(status_code=400, detail="Invalid payload format")
        
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=str(e)
        )

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
        raise HTTPException(status_code=400, detail="Invalid tool_name")
    if not request.args or not isinstance(request.args, dict):
        raise HTTPException(status_code=400, detail="Invalid args format")
    return await store.for_store().use_tool(request.tool_name, request.args)

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
            return APIResponse(
                success=True,
                data=result,
                message="Successfully registered services" if result else "Failed to register services"
            )
        
        # 2. 配置方式
        if isinstance(payload, dict):
            if "name" not in payload:
                raise HTTPException(status_code=400, detail="Service name is required")
                
            if "url" in payload and "command" in payload:
                raise HTTPException(status_code=400, detail="Cannot specify both url and command")
                
            if "url" in payload and "transport" not in payload:
                raise HTTPException(status_code=400, detail="Transport type is required for URL-based service")
                
            if "command" in payload and not isinstance(payload.get("args", []), list):
                raise HTTPException(status_code=400, detail="Args must be a list")
                
            result = await context.add_service(payload)
            return APIResponse(
                success=True,
                data=result,
                message="Successfully added service" if result else "Failed to add service"
            )
        
        raise HTTPException(status_code=400, detail="Invalid payload format")
        
    except Exception as e:
        return APIResponse(
            success=False,
            data=False,
            message=str(e)
        )

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
        raise HTTPException(status_code=400, detail="Invalid tool_name")
    if not request.args or not isinstance(request.args, dict):
        raise HTTPException(status_code=400, detail="Invalid args format")
    return await store.for_agent(agent_id).use_tool(request.tool_name, request.args)

# === 通用服务信息查询 ===
@router.get("/services/{name}", response_model=APIResponse)
@handle_exceptions
async def get_service_info(name: str, agent_id: Optional[str] = None):
    """获取服务信息，支持 Store/Agent 上下文"""
    if agent_id:
        validate_agent_id(agent_id)
        return await store.for_agent(agent_id).get_service_info(name)
    return await store.for_store().get_service_info(name)

# === 配置管理 ===
@router.get("/config", response_model=APIResponse)
@handle_exceptions
async def get_config(agent_id: Optional[str] = None):
    """获取配置，支持 Store/Agent 上下文"""
    if agent_id:
        validate_agent_id(agent_id)
        return store.get_json_config(agent_id)
    return store.get_json_config()

@router.put("/config", response_model=APIResponse)
@handle_exceptions
async def update_config(payload: JsonUpdateRequest):
    """更新配置"""
    if not payload.config:
        raise HTTPException(status_code=400, detail="Config is required")
    if payload.client_id:
        validate_agent_id(payload.client_id)
    return await store.update_json_service(payload) 
