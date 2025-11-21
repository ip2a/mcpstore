"""
响应装饰器（改进建议1-3的实现）

提供三大核心装饰器：
1. @timed_response - 自动计时和包装响应
2. @paginated - 自动分页处理
3. @handle_errors - 统一错误处理

创建日期: 2025-10-01
"""

import time
import logging
from functools import wraps
from typing import Callable, Any, Tuple, Union, Dict, List, Optional
from math import ceil

from .response import APIResponse
from .response_builder import ResponseBuilder
from .error_codes import ErrorCode

logger = logging.getLogger(__name__)


def timed_response(func: Callable) -> Callable:
    """自动计时响应装饰器（改进建议 #1）
    
    功能：
    - 自动计算执行时间
    - 自动生成request_id
    - 自动注入meta信息
    - 支持同步和异步函数
    
    使用示例：
        @timed_response
        async def my_api():
            result = do_work()
            # 直接返回数据，装饰器自动包装
            return {"result": result}
        
        # 或返回完整响应
        @timed_response
        async def my_api2():
            return ResponseBuilder.success(data={"result": "ok"})
    
    优点：
    - 无需手动使用 with TimedResponseBuilder()
    - 代码更简洁
    - 自动处理异常
    """
    
    @wraps(func)
    async def async_wrapper(*args, **kwargs):
        start_time = time.time()
        request_id = ResponseBuilder._generate_request_id()
        
        try:
            result = await func(*args, **kwargs)
            execution_time_ms = int((time.time() - start_time) * 1000)
            
            # 如果返回的已经是APIResponse，注入meta
            if isinstance(result, APIResponse):
                if result.meta is None:
                    result.meta = ResponseBuilder._create_meta(
                        execution_time_ms=execution_time_ms,
                        request_id=request_id
                    )
                return result
            
            # 如果返回的是dict或list，自动包装为成功响应
            if isinstance(result, (dict, list)):
                return ResponseBuilder.success(
                    message="Operation completed successfully",
                    data=result,
                    execution_time_ms=execution_time_ms,
                    request_id=request_id
                )
            
            # 其他类型，转换为data
            return ResponseBuilder.success(
                message="Operation completed successfully",
                data={"result": result},
                execution_time_ms=execution_time_ms,
                request_id=request_id
            )
            
        except Exception as e:
            execution_time_ms = int((time.time() - start_time) * 1000)
            logger.exception(f"Error in {func.__name__}: {e}")
            
            return ResponseBuilder.error(
                code=ErrorCode.INTERNAL_ERROR,
                message=f"An error occurred: {str(e)}",
                details={"function": func.__name__, "error_type": type(e).__name__},
                execution_time_ms=execution_time_ms,
                request_id=request_id
            )
    
    @wraps(func)
    def sync_wrapper(*args, **kwargs):
        start_time = time.time()
        request_id = ResponseBuilder._generate_request_id()
        
        try:
            result = func(*args, **kwargs)
            execution_time_ms = int((time.time() - start_time) * 1000)
            
            if isinstance(result, APIResponse):
                if result.meta is None:
                    result.meta = ResponseBuilder._create_meta(
                        execution_time_ms=execution_time_ms,
                        request_id=request_id
                    )
                return result
            
            if isinstance(result, (dict, list)):
                return ResponseBuilder.success(
                    message="Operation completed successfully",
                    data=result,
                    execution_time_ms=execution_time_ms,
                    request_id=request_id
                )
            
            return ResponseBuilder.success(
                message="Operation completed successfully",
                data={"result": result},
                execution_time_ms=execution_time_ms,
                request_id=request_id
            )
            
        except Exception as e:
            execution_time_ms = int((time.time() - start_time) * 1000)
            logger.exception(f"Error in {func.__name__}: {e}")
            
            return ResponseBuilder.error(
                code=ErrorCode.INTERNAL_ERROR,
                message=f"An error occurred: {str(e)}",
                details={"function": func.__name__, "error_type": type(e).__name__},
                execution_time_ms=execution_time_ms,
                request_id=request_id
            )
    
    # 判断是异步还是同步函数
    import asyncio
    if asyncio.iscoroutinefunction(func):
        return async_wrapper
    else:
        return sync_wrapper


def paginated(
    default_page_size: int = 20,
    max_page_size: int = 100,
    page_param: str = "page",
    page_size_param: str = "page_size"
) -> Callable:
    """自动分页装饰器（改进建议 #3）
    
    功能：
    - 自动提取分页参数
    - 自动计算分页信息
    - 自动包装分页响应
    
    使用示例：
        @paginated(default_page_size=20)
        async def list_services(page: int = 1, page_size: int = 20):
            # 只需返回 items 和 total
            items = get_services(offset=(page-1)*page_size, limit=page_size)
            total = count_services()
            return items, total  # 自动转换为分页响应
    
    优点：
    - 无需手动构造Pagination对象
    - 自动验证分页参数
    - 统一分页逻辑
    
    Args:
        default_page_size: 默认每页大小
        max_page_size: 最大每页大小
        page_param: 页码参数名
        page_size_param: 每页大小参数名
    """
    
    def decorator(func: Callable) -> Callable:
        @wraps(func)
        async def async_wrapper(*args, **kwargs):
            # 提取分页参数
            page = kwargs.get(page_param, 1)
            page_size = kwargs.get(page_size_param, default_page_size)
            
            # 验证分页参数
            page = max(1, int(page))
            page_size = max(1, min(int(page_size), max_page_size))
            
            # 更新参数
            kwargs[page_param] = page
            kwargs[page_size_param] = page_size
            
            try:
                result = await func(*args, **kwargs)
                
                # 期望返回 (items, total) 元组
                if isinstance(result, tuple) and len(result) == 2:
                    items, total = result
                    
                    return ResponseBuilder.paginated_list(
                        message=f"Retrieved {len(items)} items (page {page}/{ceil(total/page_size) if page_size > 0 else 0})",
                        items=items,
                        page=page,
                        page_size=page_size,
                        total=total
                    )
                
                # 如果返回的已经是APIResponse，直接返回
                if isinstance(result, APIResponse):
                    return result
                
                # 其他情况，当作列表处理
                if isinstance(result, list):
                    return ResponseBuilder.paginated_list(
                        message=f"Retrieved {len(result)} items",
                        items=result,
                        page=page,
                        page_size=page_size,
                        total=len(result)
                    )
                
                raise ValueError(f"Paginated function must return (items, total) tuple, got {type(result)}")
                
            except Exception as e:
                logger.exception(f"Error in paginated function {func.__name__}: {e}")
                return ResponseBuilder.error(
                    code=ErrorCode.INTERNAL_ERROR,
                    message=f"Failed to retrieve paginated data: {str(e)}",
                    details={"function": func.__name__}
                )
        
        @wraps(func)
        def sync_wrapper(*args, **kwargs):
            page = kwargs.get(page_param, 1)
            page_size = kwargs.get(page_size_param, default_page_size)
            
            page = max(1, int(page))
            page_size = max(1, min(int(page_size), max_page_size))
            
            kwargs[page_param] = page
            kwargs[page_size_param] = page_size
            
            try:
                result = func(*args, **kwargs)
                
                if isinstance(result, tuple) and len(result) == 2:
                    items, total = result
                    
                    return ResponseBuilder.paginated_list(
                        message=f"Retrieved {len(items)} items (page {page}/{ceil(total/page_size) if page_size > 0 else 0})",
                        items=items,
                        page=page,
                        page_size=page_size,
                        total=total
                    )
                
                if isinstance(result, APIResponse):
                    return result
                
                if isinstance(result, list):
                    return ResponseBuilder.paginated_list(
                        message=f"Retrieved {len(result)} items",
                        items=result,
                        page=page,
                        page_size=page_size,
                        total=len(result)
                    )
                
                raise ValueError(f"Paginated function must return (items, total) tuple, got {type(result)}")
                
            except Exception as e:
                logger.exception(f"Error in paginated function {func.__name__}: {e}")
                return ResponseBuilder.error(
                    code=ErrorCode.INTERNAL_ERROR,
                    message=f"Failed to retrieve paginated data: {str(e)}",
                    details={"function": func.__name__}
                )
        
        import asyncio
        if asyncio.iscoroutinefunction(func):
            return async_wrapper
        else:
            return sync_wrapper
    
    return decorator


def handle_errors(
    error_code: ErrorCode = ErrorCode.INTERNAL_ERROR,
    custom_message: Optional[str] = None
) -> Callable:
    """统一错误处理装饰器（改进建议 #2的辅助）
    
    功能：
    - 捕获函数中的异常
    - 自动转换为标准错误响应
    - 支持自定义错误码和消息
    
    使用示例：
        @handle_errors(error_code=ErrorCode.SERVICE_NOT_FOUND)
        async def get_service(name: str):
            service = find_service(name)
            if not service:
                raise ValueError(f"Service {name} not found")
            return service
    
    优点：
    - 统一错误处理逻辑
    - 自动记录日志
    - 减少重复代码
    
    Args:
        error_code: 默认错误码
        custom_message: 自定义错误消息模板
    """
    
    def decorator(func: Callable) -> Callable:
        @wraps(func)
        async def async_wrapper(*args, **kwargs):
            try:
                result = await func(*args, **kwargs)
                
                # 如果已经是APIResponse，直接返回
                if isinstance(result, APIResponse):
                    return result
                
                # 其他情况，包装为成功响应
                return ResponseBuilder.success(
                    message="Operation completed successfully",
                    data=result if isinstance(result, (dict, list)) else {"result": result}
                )
                
            except Exception as e:
                logger.exception(f"Error in {func.__name__}: {e}")
                
                message = custom_message or str(e) or f"An error occurred in {func.__name__}"
                
                return ResponseBuilder.error(
                    code=error_code,
                    message=message,
                    details={
                        "function": func.__name__,
                        "error_type": type(e).__name__,
                        "error_message": str(e)
                    }
                )
        
        @wraps(func)
        def sync_wrapper(*args, **kwargs):
            try:
                result = func(*args, **kwargs)
                
                if isinstance(result, APIResponse):
                    return result
                
                return ResponseBuilder.success(
                    message="Operation completed successfully",
                    data=result if isinstance(result, (dict, list)) else {"result": result}
                )
                
            except Exception as e:
                logger.exception(f"Error in {func.__name__}: {e}")
                
                message = custom_message or str(e) or f"An error occurred in {func.__name__}"
                
                return ResponseBuilder.error(
                    code=error_code,
                    message=message,
                    details={
                        "function": func.__name__,
                        "error_type": type(e).__name__,
                        "error_message": str(e)
                    }
                )
        
        import asyncio
        if asyncio.iscoroutinefunction(func):
            return async_wrapper
        else:
            return sync_wrapper
    
    return decorator


# ==================== 组合装饰器 ====================

def api_endpoint(
    use_timing: bool = True,
    use_pagination: bool = False,
    use_error_handling: bool = True,
    **kwargs
) -> Callable:
    """组合API端点装饰器
    
    将多个装饰器组合在一起，提供完整的API功能。
    
    使用示例：
        @api_endpoint(use_pagination=True, default_page_size=20)
        async def list_items(page: int = 1, page_size: int = 20):
            items = get_items(page, page_size)
            total = count_items()
            return items, total
    
    Args:
        use_timing: 是否使用自动计时
        use_pagination: 是否使用自动分页
        use_error_handling: 是否使用错误处理
        **kwargs: 传递给各装饰器的参数
    """
    
    def decorator(func: Callable) -> Callable:
        wrapped_func = func
        
        # 按顺序应用装饰器（从里到外）
        if use_error_handling:
            error_kwargs = {k: v for k, v in kwargs.items() if k in ['error_code', 'custom_message']}
            wrapped_func = handle_errors(**error_kwargs)(wrapped_func)
        
        if use_pagination:
            page_kwargs = {k: v for k, v in kwargs.items() if k in ['default_page_size', 'max_page_size', 'page_param', 'page_size_param']}
            wrapped_func = paginated(**page_kwargs)(wrapped_func)
        
        if use_timing:
            wrapped_func = timed_response(wrapped_func)
        
        return wrapped_func
    
    return decorator

