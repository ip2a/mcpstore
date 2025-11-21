"""
MCPStore API - LangChain Integration Routes
Contains LangChain adapter and tool conversion related API endpoints
"""

import logging
from typing import Dict, Any, List, Optional

from fastapi import APIRouter, HTTPException, Depends
from mcpstore.core.models.common import APIResponse

from .api_decorators import handle_exceptions, get_store, validate_agent_id

# Create LangChain router
langchain_router = APIRouter()

logger = logging.getLogger(__name__)

# === Store-level LangChain APIs ===

@langchain_router.get("/for_store/langchain_tools", response_model=APIResponse)
@handle_exceptions
async def store_get_langchain_tools():
    """Store 级别获取 LangChain 工具列表"""
    try:
        store = get_store()
        context = store.for_store()
        
        # 获取 LangChain 适配器
        langchain_adapter = context.for_langchain()
        
        # 获取工具列表
        tools = await langchain_adapter.list_tools_async()
        
        # 转换为可序列化的格式
        tools_data = []
        for tool in tools:
            tool_info = {
                "name": tool.name,
                "description": tool.description,
                "args_schema": tool.args_schema.model_json_schema() if hasattr(tool, 'args_schema') and tool.args_schema else None,
                "is_structured": hasattr(tool, 'args_schema') and tool.args_schema is not None,
                "tool_type": type(tool).__name__
            }
            tools_data.append(tool_info)
        
        return APIResponse(
            success=True,
            data={
                "tools": tools_data,
                "total_tools": len(tools_data),
                "structured_tools": len([t for t in tools_data if t["is_structured"]])
            },
            message=f"Retrieved {len(tools_data)} LangChain tools from Store"
        )
        
    except Exception as e:
        logger.error(f"Failed to get LangChain tools from Store: {e}")
        return APIResponse(
            success=False,
            data={"tools": []},
            message=f"Failed to get LangChain tools: {str(e)}"
        )

@langchain_router.get("/for_store/langchain_tools/{service_name}", response_model=APIResponse)
@handle_exceptions
async def store_get_langchain_tools_by_service(service_name: str):
    """Store 级别获取指定服务的 LangChain 工具"""
    try:
        store = get_store()
        context = store.for_store()
        
        # 首先检查服务是否存在
        all_services = context.list_services()
        service_exists = any(s.name == service_name for s in all_services)
        
        if not service_exists:
            return APIResponse(
                success=False,
                data={"tools": []},
                message=f"Service '{service_name}' not found"
            )
        
        # 获取所有工具并筛选
        langchain_adapter = context.for_langchain()
        all_tools = await langchain_adapter.list_tools_async()
        
        # 获取该服务的工具列表
        tools_info = context.get_tools_with_stats()
        service_tool_names = [tool["name"] for tool in tools_info["tools"] if tool.get("service_name") == service_name]
        
        # 筛选对应的 LangChain 工具
        service_tools = [tool for tool in all_tools if tool.name in service_tool_names]
        
        # 转换为可序列化的格式
        tools_data = []
        for tool in service_tools:
            tool_info = {
                "name": tool.name,
                "description": tool.description,
                "args_schema": tool.args_schema.model_json_schema() if hasattr(tool, 'args_schema') and tool.args_schema else None,
                "is_structured": hasattr(tool, 'args_schema') and tool.args_schema is not None,
                "tool_type": type(tool).__name__
            }
            tools_data.append(tool_info)
        
        return APIResponse(
            success=True,
            data={
                "service_name": service_name,
                "tools": tools_data,
                "total_tools": len(tools_data),
                "structured_tools": len([t for t in tools_data if t["is_structured"]])
            },
            message=f"Retrieved {len(tools_data)} LangChain tools from service '{service_name}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to get LangChain tools for service {service_name}: {e}")
        return APIResponse(
            success=False,
            data={"tools": []},
            message=f"Failed to get LangChain tools for service '{service_name}': {str(e)}"
        )

@langchain_router.post("/for_store/langchain_tool_execute", response_model=APIResponse)
@handle_exceptions
async def store_execute_langchain_tool(payload: Dict[str, Any]):
    """Store 级别执行 LangChain 工具
    
    Request Body:
    {
        "tool_name": "tool_name",  # 工具名称
        "args": {},               # 工具参数（可选）
        "kwargs": {}              # 关键字参数（可选）
    }
    """
    try:
        tool_name = payload.get("tool_name")
        if not tool_name:
            raise HTTPException(status_code=400, detail="Tool name is required")
        
        args = payload.get("args", [])
        kwargs = payload.get("kwargs", {})
        
        store = get_store()
        context = store.for_store()
        
        # 使用 LangChain 适配器执行工具
        langchain_adapter = context.for_langchain()
        
        # 获取工具列表以找到对应的工具
        tools = await langchain_adapter.list_tools_async()
        target_tool = None
        
        for tool in tools:
            if tool.name == tool_name:
                target_tool = tool
                break
        
        if not target_tool:
            return APIResponse(
                success=False,
                data={},
                message=f"Tool '{tool_name}' not found"
            )
        
        # 执行工具
        if hasattr(target_tool, 'coroutine') and target_tool.coroutine:
            # 优先使用异步执行
            result = await target_tool.coroutine(*args, **kwargs)
        else:
            # 使用同步执行
            result = target_tool.func(*args, **kwargs)
        
        return APIResponse(
            success=True,
            data={
                "tool_name": tool_name,
                "result": result,
                "execution_type": "async" if hasattr(target_tool, 'coroutine') and target_tool.coroutine else "sync"
            },
            message=f"Tool '{tool_name}' executed successfully"
        )
        
    except Exception as e:
        logger.error(f"Failed to execute LangChain tool: {e}")
        return APIResponse(
            success=False,
            data={"error": str(e)},
            message=f"Failed to execute LangChain tool: {str(e)}"
        )

# === Agent-level LangChain APIs ===

@langchain_router.get("/for_agent/{agent_id}/langchain_tools", response_model=APIResponse)
@handle_exceptions
async def agent_get_langchain_tools(agent_id: str):
    """Agent 级别获取 LangChain 工具列表"""
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)
        
        # 获取 LangChain 适配器
        langchain_adapter = context.for_langchain()
        
        # 获取工具列表
        tools = await langchain_adapter.list_tools_async()
        
        # 转换为可序列化的格式
        tools_data = []
        for tool in tools:
            tool_info = {
                "name": tool.name,
                "description": tool.description,
                "args_schema": tool.args_schema.model_json_schema() if hasattr(tool, 'args_schema') and tool.args_schema else None,
                "is_structured": hasattr(tool, 'args_schema') and tool.args_schema is not None,
                "tool_type": type(tool).__name__
            }
            tools_data.append(tool_info)
        
        return APIResponse(
            success=True,
            data={
                "agent_id": agent_id,
                "tools": tools_data,
                "total_tools": len(tools_data),
                "structured_tools": len([t for t in tools_data if t["is_structured"]])
            },
            message=f"Retrieved {len(tools_data)} LangChain tools from agent '{agent_id}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to get LangChain tools from agent {agent_id}: {e}")
        return APIResponse(
            success=False,
            data={"tools": []},
            message=f"Failed to get LangChain tools from agent '{agent_id}': {str(e)}"
        )

@langchain_router.get("/for_agent/{agent_id}/langchain_tools/{service_name}", response_model=APIResponse)
@handle_exceptions
async def agent_get_langchain_tools_by_service(agent_id: str, service_name: str):
    """Agent 级别获取指定服务的 LangChain 工具"""
    try:
        validate_agent_id(agent_id)
        store = get_store()
        context = store.for_agent(agent_id)
        
        # 首先检查服务是否存在
        all_services = await context.list_services_async()
        service_exists = any(s.name == service_name for s in all_services)
        
        if not service_exists:
            return APIResponse(
                success=False,
                data={"tools": []},
                message=f"Service '{service_name}' not found for agent '{agent_id}'"
            )
        
        # 获取所有工具并筛选
        langchain_adapter = context.for_langchain()
        all_tools = await langchain_adapter.list_tools_async()
        
        # 获取该服务的工具列表
        tools_info = context.get_tools_with_stats()
        service_tool_names = [tool["name"] for tool in tools_info["tools"] if tool.get("service_name") == service_name]
        
        # 筛选对应的 LangChain 工具
        service_tools = [tool for tool in all_tools if tool.name in service_tool_names]
        
        # 转换为可序列化的格式
        tools_data = []
        for tool in service_tools:
            tool_info = {
                "name": tool.name,
                "description": tool.description,
                "args_schema": tool.args_schema.model_json_schema() if hasattr(tool, 'args_schema') and tool.args_schema else None,
                "is_structured": hasattr(tool, 'args_schema') and tool.args_schema is not None,
                "tool_type": type(tool).__name__
            }
            tools_data.append(tool_info)
        
        return APIResponse(
            success=True,
            data={
                "agent_id": agent_id,
                "service_name": service_name,
                "tools": tools_data,
                "total_tools": len(tools_data),
                "structured_tools": len([t for t in tools_data if t["is_structured"]])
            },
            message=f"Retrieved {len(tools_data)} LangChain tools from service '{service_name}' in agent '{agent_id}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to get LangChain tools for service {service_name} in agent {agent_id}: {e}")
        return APIResponse(
            success=False,
            data={"tools": []},
            message=f"Failed to get LangChain tools for service '{service_name}' in agent '{agent_id}': {str(e)}"
        )

@langchain_router.post("/for_agent/{agent_id}/langchain_tool_execute", response_model=APIResponse)
@handle_exceptions
async def agent_execute_langchain_tool(agent_id: str, payload: Dict[str, Any]):
    """Agent 级别执行 LangChain 工具
    
    Request Body:
    {
        "tool_name": "tool_name",  # 工具名称
        "args": {},               # 工具参数（可选）
        "kwargs": {}              # 关键字参数（可选）
    }
    """
    try:
        validate_agent_id(agent_id)
        tool_name = payload.get("tool_name")
        if not tool_name:
            raise HTTPException(status_code=400, detail="Tool name is required")
        
        args = payload.get("args", [])
        kwargs = payload.get("kwargs", {})
        
        store = get_store()
        context = store.for_agent(agent_id)
        
        # 使用 LangChain 适配器执行工具
        langchain_adapter = context.for_langchain()
        
        # 获取工具列表以找到对应的工具
        tools = await langchain_adapter.list_tools_async()
        target_tool = None
        
        for tool in tools:
            if tool.name == tool_name:
                target_tool = tool
                break
        
        if not target_tool:
            return APIResponse(
                success=False,
                data={},
                message=f"Tool '{tool_name}' not found for agent '{agent_id}'"
            )
        
        # 执行工具
        if hasattr(target_tool, 'coroutine') and target_tool.coroutine:
            # 优先使用异步执行
            result = await target_tool.coroutine(*args, **kwargs)
        else:
            # 使用同步执行
            result = target_tool.func(*args, **kwargs)
        
        return APIResponse(
            success=True,
            data={
                "agent_id": agent_id,
                "tool_name": tool_name,
                "result": result,
                "execution_type": "async" if hasattr(target_tool, 'coroutine') and target_tool.coroutine else "sync"
            },
            message=f"Tool '{tool_name}' executed successfully for agent '{agent_id}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to execute LangChain tool for agent {agent_id}: {e}")
        return APIResponse(
            success=False,
            data={"error": str(e)},
            message=f"Failed to execute LangChain tool for agent '{agent_id}': {str(e)}"
        )

# === LangChain 工具信息 API ===

@langchain_router.get("/for_store/langchain_tool_info/{tool_name}", response_model=APIResponse)
@handle_exceptions
async def store_get_langchain_tool_info(tool_name: str):
    """Store 级别获取 LangChain 工具详细信息"""
    try:
        store = get_store()
        context = store.for_store()
        
        # 获取 LangChain 适配器和工具列表
        langchain_adapter = context.for_langchain()
        tools = await langchain_adapter.list_tools_async()
        
        # 查找目标工具
        target_tool = None
        for tool in tools:
            if tool.name == tool_name:
                target_tool = tool
                break
        
        if not target_tool:
            return APIResponse(
                success=False,
                data={},
                message=f"Tool '{tool_name}' not found"
            )
        
        # 构建工具信息
        tool_info = {
            "name": target_tool.name,
            "description": target_tool.description,
            "is_structured": hasattr(target_tool, 'args_schema') and target_tool.args_schema is not None,
            "tool_type": type(target_tool).__name__,
            "has_coroutine": hasattr(target_tool, 'coroutine') and target_tool.coroutine is not None
        }
        
        # 添加参数模式信息
        if hasattr(target_tool, 'args_schema') and target_tool.args_schema:
            tool_info["args_schema"] = target_tool.args_schema.model_json_schema()
            # 提取参数信息
            schema = target_tool.args_schema.model_json_schema()
            properties = schema.get("properties", {})
            required = schema.get("required", [])
            
            tool_info["parameters"] = {
                "required": required,
                "optional": [p for p in properties.keys() if p not in required],
                "total_count": len(properties)
            }
        else:
            tool_info["parameters"] = {
                "required": [],
                "optional": [],
                "total_count": 0
            }
        
        # 获取原始工具信息
        try:
            original_tools = context.get_tools_with_stats()
            original_tool = next((t for t in original_tools["tools"] if t["name"] == tool_name), None)
            if original_tool:
                tool_info["original_info"] = {
                    "service_name": original_tool.get("service_name"),
                    "input_schema": original_tool.get("inputSchema"),
                    "description": original_tool.get("description")
                }
        except Exception as e:
            logger.warning(f"Failed to get original tool info for {tool_name}: {e}")
        
        return APIResponse(
            success=True,
            data=tool_info,
            message=f"Tool info retrieved for '{tool_name}'"
        )
        
    except Exception as e:
        logger.error(f"Failed to get LangChain tool info for {tool_name}: {e}")
        return APIResponse(
            success=False,
            data={},
            message=f"Failed to get LangChain tool info: {str(e)}"
        )