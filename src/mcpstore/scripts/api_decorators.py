"""
MCPStore API Decorators and Utility Functions
Contains common functionality such as exception handling, performance monitoring, validation, etc.
"""

import time
from functools import wraps
from typing import Optional, List

from fastapi import HTTPException
from mcpstore import MCPStore
from mcpstore.core.models.common import APIResponse
from pydantic import ValidationError


# === Decorator functions ===

def handle_exceptions(func):
    """Unified exception handling decorator"""
    @wraps(func)
    async def wrapper(*args, **kwargs):
        try:
            result = await func(*args, **kwargs)
            # If result is already APIResponse, return directly
            if isinstance(result, APIResponse):
                return result
            # Otherwise wrap as APIResponse
            return APIResponse(success=True, data=result)
        except HTTPException:
            # HTTPException should be passed directly, don't wrap
            raise
        except ValidationError as e:
            # Pydantic validation error, return 400
            raise HTTPException(status_code=400, detail=f"Validation error: {str(e)}")
        except ValueError as e:
            raise HTTPException(status_code=400, detail=str(e))
        except Exception as e:
            raise HTTPException(status_code=500, detail=str(e))
    return wrapper

def monitor_api_performance(func):
    """API performance monitoring decorator"""
    @wraps(func)
    async def wrapper(*args, **kwargs):
        start_time = time.time()

        # Get store instance (from dependency injection)
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
            from .api_app import get_store
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

# === 验证函数 ===

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

# === 依赖注入函数 ===

def get_store() -> MCPStore:
    """获取MCPStore实例的依赖注入函数"""
    # 从api_app模块获取当前的store实例
    from .api_app import get_store as get_app_store
    return get_app_store()
