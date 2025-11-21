"""
响应构造器

提供便捷的响应构造方法，确保：
- 响应格式统一
- 元数据自动生成
- 类型安全

创建日期: 2025-10-01
"""

import time
import uuid
from datetime import datetime
from typing import Any, List, Dict, Optional, Union
from math import ceil

from .response import APIResponse, ErrorDetail, ResponseMeta, Pagination
from .error_codes import ErrorCode


class ResponseBuilder:
    """响应构造器
    
    使用示例：
        # 成功响应
        response = ResponseBuilder.success(
            message="Service retrieved",
            data={"name": "weather"},
            execution_time_ms=150
        )
        
        # 错误响应
        response = ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message="Service not found",
            details={"service_name": "weather"}
        )
    """
    
    @staticmethod
    def _generate_request_id() -> str:
        """生成唯一请求ID"""
        return f"req_{uuid.uuid4().hex[:16]}"
    
    @staticmethod
    def _get_timestamp() -> str:
        """获取ISO 8601格式时间戳"""
        return datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"
    
    @staticmethod
    def _create_meta(execution_time_ms: int, request_id: Optional[str] = None) -> ResponseMeta:
        """创建元数据"""
        return ResponseMeta(
            timestamp=ResponseBuilder._get_timestamp(),
            request_id=request_id or ResponseBuilder._generate_request_id(),
            execution_time_ms=execution_time_ms,
            api_version="1.0.0"
        )
    
    @staticmethod
    def success(
        message: str,
        data: Optional[Union[Dict, List]] = None,
        execution_time_ms: Optional[int] = None,
        request_id: Optional[str] = None,
        pagination: Optional[Dict] = None
    ) -> APIResponse:
        """构造成功响应
        
        Args:
            message: 响应消息
            data: 响应数据（Dict或List）
            execution_time_ms: 执行时间（毫秒）
            request_id: 请求ID（自动生成）
            pagination: 分页信息字典（仅data为List时）
            
        Returns:
            APIResponse对象
        """
        # 自动计算执行时间
        if execution_time_ms is None:
            execution_time_ms = 0
        
        # 创建元数据
        meta = ResponseBuilder._create_meta(execution_time_ms, request_id)
        
        # 处理分页
        pagination_obj = None
        if pagination and isinstance(data, list):
            pagination_obj = Pagination(**pagination)
        
        return APIResponse(
            success=True,
            message=message,
            data=data,
            errors=None,
            meta=meta,
            pagination=pagination_obj
        )
    
    @staticmethod
    def error(
        code: Union[ErrorCode, str],
        message: str,
        field: Optional[str] = None,
        details: Optional[Dict] = None,
        execution_time_ms: Optional[int] = None,
        request_id: Optional[str] = None
    ) -> APIResponse:
        """构造错误响应（单个错误）
        
        Args:
            code: 错误码（ErrorCode或字符串）
            message: 错误消息
            field: 相关字段（可选）
            details: 详细信息（可选）
            execution_time_ms: 执行时间（毫秒）
            request_id: 请求ID（自动生成）
            
        Returns:
            APIResponse对象
        """
        if execution_time_ms is None:
            execution_time_ms = 0
        
        meta = ResponseBuilder._create_meta(execution_time_ms, request_id)
        
        # 如果code是ErrorCode枚举，转换为字符串
        code_str = code.value if isinstance(code, ErrorCode) else code
        
        error = ErrorDetail(
            code=code_str,
            message=message,
            field=field,
            details=details
        )
        
        return APIResponse(
            success=False,
            message=message,
            data=None,
            errors=[error],
            meta=meta,
            pagination=None
        )
    
    @staticmethod
    def errors(
        message: str,
        errors: List[Dict],
        execution_time_ms: Optional[int] = None,
        request_id: Optional[str] = None
    ) -> APIResponse:
        """构造错误响应（多个错误）
        
        Args:
            message: 总体错误消息
            errors: 错误列表，每个元素包含code, message等
            execution_time_ms: 执行时间（毫秒）
            request_id: 请求ID（自动生成）
            
        Returns:
            APIResponse对象
        """
        if execution_time_ms is None:
            execution_time_ms = 0
        
        meta = ResponseBuilder._create_meta(execution_time_ms, request_id)
        
        error_objects = [ErrorDetail(**e) for e in errors]
        
        return APIResponse(
            success=False,
            message=message,
            data=None,
            errors=error_objects,
            meta=meta,
            pagination=None
        )
    
    @staticmethod
    def paginated_list(
        message: str,
        items: List[Any],
        page: int,
        page_size: int,
        total: int,
        execution_time_ms: Optional[int] = None,
        request_id: Optional[str] = None
    ) -> APIResponse:
        """构造分页列表响应
        
        Args:
            message: 响应消息
            items: 当前页的数据列表
            page: 当前页码
            page_size: 每页大小
            total: 总记录数
            execution_time_ms: 执行时间（毫秒）
            request_id: 请求ID（自动生成）
            
        Returns:
            APIResponse对象
        """
        total_pages = ceil(total / page_size) if page_size > 0 else 0
        
        pagination = Pagination(
            page=page,
            page_size=page_size,
            total=total,
            total_pages=total_pages,
            has_next=page < total_pages,
            has_prev=page > 1
        )
        
        return ResponseBuilder.success(
            message=message,
            data=items,
            execution_time_ms=execution_time_ms,
            request_id=request_id,
            pagination=pagination.dict()
        )


class TimedResponseBuilder:
    """带计时的响应构造器
    
    使用with语句自动计算执行时间：
        with TimedResponseBuilder() as builder:
            # ... 执行操作 ...
            result = some_operation()
            
            return builder.success(
                message="Operation completed",
                data=result
            )
    """
    
    def __init__(self):
        self.start_time = None
        self.request_id = ResponseBuilder._generate_request_id()
    
    def __enter__(self):
        self.start_time = time.time()
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        pass
    
    def _get_execution_time(self) -> int:
        """获取执行时间（毫秒）"""
        if self.start_time is None:
            return 0
        return int((time.time() - self.start_time) * 1000)
    
    def success(self, message: str, data: Optional[Union[Dict, List]] = None, **kwargs) -> APIResponse:
        """构造成功响应（自动计时）"""
        return ResponseBuilder.success(
            message=message,
            data=data,
            execution_time_ms=self._get_execution_time(),
            request_id=self.request_id,
            **kwargs
        )
    
    def error(self, code: Union[ErrorCode, str], message: str, **kwargs) -> APIResponse:
        """构造错误响应（自动计时）"""
        return ResponseBuilder.error(
            code=code,
            message=message,
            execution_time_ms=self._get_execution_time(),
            request_id=self.request_id,
            **kwargs
        )
    
    def paginated_list(self, message: str, items: List, page: int, page_size: int, total: int) -> APIResponse:
        """构造分页响应（自动计时）"""
        return ResponseBuilder.paginated_list(
            message=message,
            items=items,
            page=page,
            page_size=page_size,
            total=total,
            execution_time_ms=self._get_execution_time(),
            request_id=self.request_id
        )

