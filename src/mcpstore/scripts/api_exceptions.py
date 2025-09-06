"""
MCPStore API Unified Exception Handling
Provides comprehensive exception handling and error response formatting
"""

import logging
import traceback
from typing import Optional, Dict, Any, Union, List
from datetime import datetime
import uuid

from fastapi import Request, HTTPException
from fastapi.responses import JSONResponse
from fastapi.exceptions import RequestValidationError
from starlette.exceptions import HTTPException as StarletteHTTPException
from pydantic import ValidationError

from mcpstore.core.models.common import APIResponse

# 设置日志记录器
logger = logging.getLogger(__name__)

# === 错误代码定义 ===

class ErrorCode:
    """错误代码常量"""
    # 通用错误
    INTERNAL_ERROR = "INTERNAL_ERROR"
    VALIDATION_ERROR = "VALIDATION_ERROR"
    NOT_FOUND = "NOT_FOUND"
    UNAUTHORIZED = "UNAUTHORIZED"
    FORBIDDEN = "FORBIDDEN"
    
    # 服务相关错误
    SERVICE_NOT_FOUND = "SERVICE_NOT_FOUND"
    SERVICE_ALREADY_EXISTS = "SERVICE_ALREADY_EXISTS"
    SERVICE_INITIALIZATION_FAILED = "SERVICE_INITIALIZATION_FAILED"
    SERVICE_OPERATION_FAILED = "SERVICE_OPERATION_FAILED"
    
    # Agent相关错误
    AGENT_NOT_FOUND = "AGENT_NOT_FOUND"
    AGENT_ALREADY_EXISTS = "AGENT_ALREADY_EXISTS"
    AGENT_OPERATION_FAILED = "AGENT_OPERATION_FAILED"
    
    # 工具相关错误
    TOOL_NOT_FOUND = "TOOL_NOT_FOUND"
    TOOL_EXECUTION_FAILED = "TOOL_EXECUTION_FAILED"
    TOOL_TIMEOUT = "TOOL_TIMEOUT"
    
    # 配置相关错误
    CONFIG_ERROR = "CONFIG_ERROR"
    CONFIG_NOT_FOUND = "CONFIG_NOT_FOUND"
    CONFIG_UPDATE_FAILED = "CONFIG_UPDATE_FAILED"
    
    # 数据空间相关错误
    WORKSPACE_NOT_FOUND = "WORKSPACE_NOT_FOUND"
    WORKSPACE_ALREADY_EXISTS = "WORKSPACE_ALREADY_EXISTS"
    DATASPACE_ERROR = "DATASPACE_ERROR"
    
    # LangChain相关错误
    LANGCHAIN_ADAPTER_ERROR = "LANGCHAIN_ADAPTER_ERROR"
    TOOL_CONVERSION_ERROR = "TOOL_CONVERSION_ERROR"

# === 异常类定义 ===

class MCPStoreException(Exception):
    """MCPStore基础异常类"""
    
    def __init__(
        self,
        message: str,
        error_code: str = ErrorCode.INTERNAL_ERROR,
        status_code: int = 500,
        details: Optional[Dict[str, Any]] = None,
        stack_trace: Optional[str] = None
    ):
        self.message = message
        self.error_code = error_code
        self.status_code = status_code
        self.details = details or {}
        self.stack_trace = stack_trace
        self.timestamp = datetime.utcnow()
        self.error_id = str(uuid.uuid4())[:8]
        super().__init__(self.message)

class ServiceNotFoundException(MCPStoreException):
    """服务未找到异常"""
    
    def __init__(self, service_name: str, details: Optional[Dict[str, Any]] = None):
        super().__init__(
            message=f"Service '{service_name}' not found",
            error_code=ErrorCode.SERVICE_NOT_FOUND,
            status_code=404,
            details={"service_name": service_name, **(details or {})}
        )

class AgentNotFoundException(MCPStoreException):
    """Agent未找到异常"""
    
    def __init__(self, agent_id: str, details: Optional[Dict[str, Any]] = None):
        super().__init__(
            message=f"Agent '{agent_id}' not found",
            error_code=ErrorCode.AGENT_NOT_FOUND,
            status_code=404,
            details={"agent_id": agent_id, **(details or {})}
        )

class ToolNotFoundException(MCPStoreException):
    """工具未找到异常"""
    
    def __init__(self, tool_name: str, details: Optional[Dict[str, Any]] = None):
        super().__init__(
            message=f"Tool '{tool_name}' not found",
            error_code=ErrorCode.TOOL_NOT_FOUND,
            status_code=404,
            details={"tool_name": tool_name, **(details or {})}
        )

class ServiceOperationException(MCPStoreException):
    """服务操作异常"""
    
    def __init__(self, message: str, service_name: str, operation: str, details: Optional[Dict[str, Any]] = None):
        super().__init__(
            message=message,
            error_code=ErrorCode.SERVICE_OPERATION_FAILED,
            status_code=500,
            details={
                "service_name": service_name,
                "operation": operation,
                **(details or {})
            }
        )

class ValidationException(MCPStoreException):
    """验证异常"""
    
    def __init__(self, message: str, field: Optional[str] = None, details: Optional[Dict[str, Any]] = None):
        super().__init__(
            message=message,
            error_code=ErrorCode.VALIDATION_ERROR,
            status_code=400,
            details={"field": field, **(details or {})} if field else (details or {})
        )

class ConfigurationException(MCPStoreException):
    """配置异常"""
    
    def __init__(self, message: str, config_path: Optional[str] = None, details: Optional[Dict[str, Any]] = None):
        super().__init__(
            message=message,
            error_code=ErrorCode.CONFIG_ERROR,
            status_code=500,
            details={"config_path": config_path, **(details or {})}
        )

# === 错误响应格式化 ===

def format_error_response(
    error: Union[MCPStoreException, Exception],
    include_stack_trace: bool = False
) -> Dict[str, Any]:
    """格式化错误响应"""
    
    if isinstance(error, MCPStoreException):
        response = {
            "success": False,
            "error": {
                "code": error.error_code,
                "message": error.message,
                "error_id": error.error_id,
                "timestamp": error.timestamp.isoformat(),
                "details": error.details
            }
        }
        
        if include_stack_trace and error.stack_trace:
            response["error"]["stack_trace"] = error.stack_trace
            
    else:
        # 标准异常处理
        response = {
            "success": False,
            "error": {
                "code": ErrorCode.INTERNAL_ERROR,
                "message": str(error),
                "error_id": str(uuid.uuid4())[:8],
                "timestamp": datetime.utcnow().isoformat(),
                "details": {}
            }
        }
        
        if include_stack_trace:
            response["error"]["stack_trace"] = traceback.format_exc()
    
    return response

# === 异常处理器 ===

async def mcpstore_exception_handler(request: Request, exc: MCPStoreException):
    """MCPStore异常处理器"""
    logger.error(
        f"MCPStore error [{exc.error_id}]: {exc.message}",
        extra={
            "error_code": exc.error_code,
            "status_code": exc.status_code,
            "details": exc.details,
            "error_id": exc.error_id,
            "path": request.url.path,
            "method": request.method
        }
    )
    
    response_data = format_error_response(exc, include_stack_trace=False)
    return JSONResponse(
        status_code=exc.status_code,
        content=response_data
    )

async def validation_exception_handler(request: Request, exc: RequestValidationError):
    """请求验证异常处理器"""
    errors = []
    for error in exc.errors():
        field = " -> ".join([str(loc) for loc in error["loc"] if loc != "body"])
        errors.append({
            "field": field,
            "message": error["msg"],
            "type": error["type"]
        })
    
    logger.warning(
        f"Validation error: {len(errors)} errors",
        extra={
            "errors": errors,
            "path": request.url.path,
            "method": request.method
        }
    )
    
    response_data = {
        "success": False,
        "error": {
            "code": ErrorCode.VALIDATION_ERROR,
            "message": "Request validation failed",
            "error_id": str(uuid.uuid4())[:8],
            "timestamp": datetime.utcnow().isoformat(),
            "details": {
                "validation_errors": errors
            }
        }
    }
    
    return JSONResponse(
        status_code=422,
        content=response_data
    )

async def http_exception_handler(request: Request, exc: HTTPException):
    """HTTP异常处理器"""
    logger.warning(
        f"HTTP error: {exc.status_code} - {exc.detail}",
        extra={
            "status_code": exc.status_code,
            "path": request.url.path,
            "method": request.method
        }
    )
    
    response_data = {
        "success": False,
        "error": {
            "code": "HTTP_ERROR",
            "message": exc.detail,
            "error_id": str(uuid.uuid4())[:8],
            "timestamp": datetime.utcnow().isoformat(),
            "details": {
                "status_code": exc.status_code
            }
        }
    }
    
    return JSONResponse(
        status_code=exc.status_code,
        content=response_data
    )

async def general_exception_handler(request: Request, exc: Exception):
    """通用异常处理器"""
    error_id = str(uuid.uuid4())[:8]
    logger.error(
        f"Unhandled exception [{error_id}]: {str(exc)}",
        extra={
            "error_id": error_id,
            "path": request.url.path,
            "method": request.method,
            "stack_trace": traceback.format_exc()
        },
        exc_info=True
    )
    
    response_data = {
        "success": False,
        "error": {
            "code": ErrorCode.INTERNAL_ERROR,
            "message": "Internal server error",
            "error_id": error_id,
            "timestamp": datetime.utcnow().isoformat(),
            "details": {
                "type": type(exc).__name__
            }
        }
    }
    
    return JSONResponse(
        status_code=500,
        content=response_data
    )

# === 异常处理装饰器 ===

def handle_api_exceptions(func):
    """API异常处理装饰器（增强版）"""
    import functools
    
    @functools.wraps(func)
    async def wrapper(*args, **kwargs):
        try:
            result = await func(*args, **kwargs)
            
            # 如果结果已经是APIResponse，直接返回
            if isinstance(result, APIResponse):
                return result
                
            # 否则包装为APIResponse
            return APIResponse(success=True, data=result)
            
        except MCPStoreException:
            # MCPStore异常已经包含足够信息，直接抛出
            raise
            
        except HTTPException:
            # HTTPException应该直接传递，不要包装
            raise
            
        except RequestValidationError:
            # FastAPI验证错误，让全局处理器处理
            raise
            
        except ValidationError as e:
            # Pydantic验证错误
            raise ValidationException(
                message=f"Data validation error: {str(e)}",
                details={"validation_errors": e.errors()}
            )
            
        except ValueError as e:
            # 值错误
            raise ValidationException(message=str(e))
            
        except KeyError as e:
            # 键错误
            raise ValidationException(
                message=f"Missing required field: {str(e)}",
                field=str(e)
            )
            
        except AttributeError as e:
            # 属性错误
            raise MCPStoreException(
                message=f"Attribute error: {str(e)}",
                error_code=ErrorCode.INTERNAL_ERROR,
                details={"attribute": str(e)}
            )
            
        except Exception as e:
            # 其他所有异常
            error_id = str(uuid.uuid4())[:8]
            logger.error(
                f"Unhandled API exception [{error_id}]: {str(e)}",
                extra={
                    "error_id": error_id,
                    "function": func.__name__,
                    "stack_trace": traceback.format_exc()
                },
                exc_info=True
            )
            
            raise MCPStoreException(
                message=f"Internal server error [{error_id}]",
                error_code=ErrorCode.INTERNAL_ERROR,
                details={
                    "function": func.__name__,
                    "type": type(e).__name__
                },
                stack_trace=traceback.format_exc()
            )
    
    return wrapper

# === 错误监控和报告 ===

class ErrorMonitor:
    """错误监控器"""
    
    def __init__(self):
        self.error_counts: Dict[str, int] = {}
        self.recent_errors: List[Dict[str, Any]] = []
        self.max_recent_errors = 100
    
    def record_error(self, error: Union[MCPStoreException, Exception], context: Optional[Dict[str, Any]] = None):
        """记录错误"""
        error_code = getattr(error, 'error_code', ErrorCode.INTERNAL_ERROR)
        
        # 更新错误计数
        self.error_counts[error_code] = self.error_counts.get(error_code, 0) + 1
        
        # 记录最近错误
        error_info = {
            "error_id": getattr(error, 'error_id', str(uuid.uuid4())[:8]),
            "error_code": error_code,
            "message": str(error),
            "timestamp": datetime.utcnow().isoformat(),
            "context": context or {}
        }
        
        self.recent_errors.append(error_info)
        
        # 保持最近错误列表在限制范围内
        if len(self.recent_errors) > self.max_recent_errors:
            self.recent_errors = self.recent_errors[-self.max_recent_errors:]
    
    def get_error_stats(self) -> Dict[str, Any]:
        """获取错误统计"""
        return {
            "total_errors": sum(self.error_counts.values()),
            "error_counts": self.error_counts,
            "recent_errors": self.recent_errors[-10:],  # 最近10个错误
            "unique_error_codes": len(self.error_counts)
        }
    
    def clear_stats(self):
        """清除统计信息"""
        self.error_counts.clear()
        self.recent_errors.clear()

# 全局错误监控器实例
error_monitor = ErrorMonitor()