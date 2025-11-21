"""
MCPStore API Unified Exception Handling
Provides comprehensive exception handling and error response formatting
"""

import logging
import traceback
import uuid
from datetime import datetime
from typing import Optional, Dict, Any, Union, List

from fastapi import Request, HTTPException
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from pydantic import ValidationError

# Import unified exception system
from mcpstore.core.exceptions import (
    MCPStoreException,
    ErrorCode,
    ErrorSeverity,
    ServiceNotFoundException,
    ServiceConnectionError,
    ServiceUnavailableError,
    ToolNotFoundException,
    ToolExecutionError,
    ConfigurationException,
    ValidationException,
    AgentNotFoundException,
)

# 导入新的响应模型
from mcpstore.core.models import (
    APIResponse,
    ResponseBuilder,
    ErrorDetail
)

# 设置日志记录器
logger = logging.getLogger(__name__)

# === Exception classes are now imported from mcpstore.core.exceptions ===
# No need to redefine them here

# === 错误响应格式化（使用新架构） ===

def format_error_response(
    error: Union[MCPStoreException, Exception],
    include_stack_trace: bool = False
) -> APIResponse:
    """格式化错误响应（使用新的APIResponse模型）"""
    
    if isinstance(error, MCPStoreException):
        # 构造详情，可能包含堆栈跟踪
        details = {**error.details, "error_id": error.error_id}
        if include_stack_trace and error.stack_trace:
            details["stack_trace"] = error.stack_trace
        
        return ResponseBuilder.error(
            code=error.error_code,
            message=error.message,
            field=error.field,
            details=details
        )
    else:
        # 标准异常处理
        details = {
            "error_id": str(uuid.uuid4())[:8],
            "error_type": type(error).__name__
        }
        if include_stack_trace:
            details["stack_trace"] = traceback.format_exc()
        
        return ResponseBuilder.error(
            code=ErrorCode.INTERNAL_ERROR,
            message=str(error) or "Internal server error",
            details=details
        )

# === 异常处理器 ===

async def mcpstore_exception_handler(request: Request, exc: MCPStoreException):
    """MCPStore异常处理器（使用新响应格式）"""
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
    
    response = format_error_response(exc, include_stack_trace=False)
    return JSONResponse(
        status_code=exc.status_code,
        content=response.dict(exclude_none=True)
    )

async def validation_exception_handler(request: Request, exc: RequestValidationError):
    """请求验证异常处理器（使用新响应格式）"""
    # 转换为ErrorDetail列表
    error_details = []
    for error in exc.errors():
        field = " -> ".join([str(loc) for loc in error["loc"] if loc != "body"])
        error_details.append({
            "code": ErrorCode.INVALID_PARAMETER.value,
            "message": error["msg"],
            "field": field,
            "details": {"type": error["type"]}
        })
    
    logger.warning(
        f"Validation error: {len(error_details)} errors",
        extra={
            "errors": error_details,
            "path": request.url.path,
            "method": request.method
        }
    )
    
    response = ResponseBuilder.errors(
        message=f"Request validation failed ({len(error_details)} errors)",
        errors=error_details
    )
    
    return JSONResponse(
        status_code=422,
        content=response.dict(exclude_none=True)
    )

async def http_exception_handler(request: Request, exc: HTTPException):
    """HTTP异常处理器（使用新响应格式）"""
    logger.warning(
        f"HTTP error: {exc.status_code} - {exc.detail}",
        extra={
            "status_code": exc.status_code,
            "path": request.url.path,
            "method": request.method
        }
    )
    
    # 映射HTTP状态码到错误码
    error_code_map = {
        404: ErrorCode.SERVICE_NOT_FOUND,
        401: ErrorCode.AUTHENTICATION_REQUIRED,
        403: ErrorCode.AUTHORIZATION_FAILED,
        400: ErrorCode.INVALID_REQUEST,
        429: ErrorCode.RATE_LIMIT_EXCEEDED,
    }
    error_code = error_code_map.get(exc.status_code, ErrorCode.INTERNAL_ERROR)
    
    response = ResponseBuilder.error(
        code=error_code,
        message=exc.detail or "HTTP error",
        details={"http_status": exc.status_code}
    )
    
    return JSONResponse(
        status_code=exc.status_code,
        content=response.dict(exclude_none=True)
    )

async def general_exception_handler(request: Request, exc: Exception):
    """通用异常处理器（使用新响应格式）"""
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
    
    response = ResponseBuilder.error(
        code=ErrorCode.INTERNAL_ERROR,
        message="Internal server error",
        details={
            "error_id": error_id,
            "error_type": type(exc).__name__
        }
    )
    
    return JSONResponse(
        status_code=500,
        content=response.dict(exclude_none=True)
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
                
            # 否则包装为成功响应
            return ResponseBuilder.success(
                message="Operation completed successfully",
                data=result if isinstance(result, (dict, list)) else {"result": result}
            )
            
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
        # 处理ErrorCode枚举
        if isinstance(error, MCPStoreException):
            error_code = error.error_code
        else:
            error_code = ErrorCode.INTERNAL_ERROR.value
        
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
