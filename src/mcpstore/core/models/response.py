"""
MCPStore API 响应模型

统一的API响应架构，提供：
- 统一的响应结构
- 标准化的错误处理
- 完整的追踪信息
- 类型安全的数据模型

创建日期: 2025-10-01
"""

from typing import Optional, Any, List, Dict, Union

from pydantic import BaseModel, Field, ConfigDict


class ErrorDetail(BaseModel):
    """错误详情模型
    
    用于描述单个错误的详细信息。支持：
    - 标准错误码（用于程序判断）
    - 人类可读消息（用于显示）
    - 相关字段（用于表单验证）
    - 额外详情（用于调试）
    
    示例：
        # 通用错误
        ErrorDetail(
            code="SERVICE_NOT_FOUND",
            message="Service 'weather' does not exist",
            details={"service_name": "weather"}
        )
        
        # 验证错误
        ErrorDetail(
            code="INVALID_PARAMETER",
            message="Field 'url' is required",
            field="url",
            details={"provided": None, "expected": "string"}
        )
    """
    
    code: str = Field(
        ...,
        description="标准错误码。大写下划线格式，用于程序判断错误类型",
        json_schema_extra={"example": "SERVICE_NOT_FOUND"},
        pattern="^[A-Z_]+$"
    )
    
    message: str = Field(
        ...,
        description="人类可读的错误消息。可用于直接显示给用户",
        json_schema_extra={"example": "The requested service does not exist"}
    )
    
    field: Optional[str] = Field(
        None,
        description="相关字段名。用于表单验证错误，指明哪个字段出错",
        json_schema_extra={"example": "service_name"}
    )
    
    details: Optional[Dict[str, Any]] = Field(
        None,
        description="错误的额外详情信息。包含有助于调试的上下文",
        json_schema_extra={"example": {"service_name": "weather", "attempted_operation": "get_status"}}
    )
    
    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "code": "INVALID_PARAMETER",
                "message": "The 'url' parameter is required but was not provided",
                "field": "url",
                "details": {
                    "provided_value": None,
                    "expected_type": "string",
                    "parameter_name": "url"
                }
            }
        }
    )


class ResponseMeta(BaseModel):
    """响应元数据模型
    
    包含所有追踪、性能、版本信息。用于：
    - 请求追踪（request_id）
    - 性能监控（execution_time_ms）
    - 时间记录（timestamp）
    - 版本管理（api_version）
    
    所有字段都是必需的，确保元数据完整性。
    """
    
    timestamp: str = Field(
        ...,
        description="响应生成的ISO 8601时间戳（UTC）",
        json_schema_extra={"example": "2025-10-01T12:00:00.000Z"}
    )
    
    request_id: str = Field(
        ...,
        description="唯一请求标识符。用于追踪和日志关联。格式：req_[16位随机字符]",
        json_schema_extra={"example": "req_a1b2c3d4e5f6g7h8"},
        min_length=20,
        max_length=20
    )
    
    execution_time_ms: int = Field(
        ...,
        description="服务端执行时间（毫秒）。从接收请求到生成响应的耗时",
        json_schema_extra={"example": 150},
        ge=0
    )
    
    api_version: str = Field(
        default="1.0.0",
        description="API版本号。遵循语义化版本规范",
        json_schema_extra={"example": "1.0.0"}
    )
    
    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "timestamp": "2025-10-01T12:00:00.000Z",
                "request_id": "req_a1b2c3d4e5f6g7h8",
                "execution_time_ms": 150,
                "api_version": "2.0.0"
            }
        }
    )


class Pagination(BaseModel):
    """分页信息模型
    
    仅在返回列表数据且支持分页时使用。
    提供完整的分页导航信息。
    
    计算规则：
    - total_pages = ceil(total / page_size)
    - has_next = page < total_pages
    - has_prev = page > 1
    """
    
    page: int = Field(
        ...,
        description="当前页码（从1开始）",
        json_schema_extra={"example": 1},
        ge=1
    )
    
    page_size: int = Field(
        ...,
        description="每页记录数",
        json_schema_extra={"example": 20},
        ge=1,
        le=100
    )
    
    total: int = Field(
        ...,
        description="总记录数",
        json_schema_extra={"example": 100},
        ge=0
    )
    
    total_pages: int = Field(
        ...,
        description="总页数",
        json_schema_extra={"example": 5},
        ge=0
    )
    
    has_next: bool = Field(
        ...,
        description="是否有下一页",
        json_schema_extra={"example": True}
    )
    
    has_prev: bool = Field(
        ...,
        description="是否有上一页",
        json_schema_extra={"example": False}
    )
    
    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "page": 1,
                "page_size": 20,
                "total": 100,
                "total_pages": 5,
                "has_next": True,
                "has_prev": False
            }
        }
    )


class APIResponse(BaseModel):
    """统一API响应模型
    
    设计原则：
    - 所有API接口统一使用此模型
    - 成功时返回data，失败时返回errors
    - meta包含追踪和性能信息
    - pagination仅在data为列表时使用
    
    示例：
        # 成功响应
        APIResponse(
            success=True,
            message="Service retrieved successfully",
            data={"name": "weather", "status": "healthy"},
            meta=ResponseMeta(...)
        )
        
        # 失败响应
        APIResponse(
            success=False,
            message="Service not found",
            data=None,
            errors=[ErrorDetail(code="SERVICE_NOT_FOUND", ...)]
        )
    """
    
    # 核心字段（必需）
    success: bool = Field(
        ..., 
        description="操作是否成功。true=成功, false=失败"
    )
    
    message: str = Field(
        ..., 
        description="人类可读的响应消息。成功时描述操作结果，失败时描述错误原因",
        json_schema_extra={"example": "Service retrieved successfully"}
    )
    
    # 数据字段（可选）
    data: Optional[Union[Dict[str, Any], List[Any]]] = Field(
        None,
        description="响应数据。成功时包含实际数据，失败时为null。类型严格限制为Dict或List",
        json_schema_extra={"example": {"name": "weather", "status": "healthy"}}
    )
    
    # 错误字段（可选，仅失败时）
    errors: Optional[List[ErrorDetail]] = Field(
        None,
        description="错误详情列表。仅在success=false时存在。支持多个错误（如参数验证）",
        json_schema_extra={"example": [{
            "code": "SERVICE_NOT_FOUND",
            "message": "The requested service does not exist",
            "field": None,
            "details": {"service_name": "weather"}
        }]}
    )
    
    # 元数据字段（可选）
    meta: Optional[ResponseMeta] = Field(
        None,
        description="响应元数据。包含追踪信息、性能指标、API版本等",
        json_schema_extra={"example": {
            "timestamp": "2025-10-01T12:00:00.000Z",
            "request_id": "req_a1b2c3d4e5f6",
            "execution_time_ms": 150,
            "api_version": "2.0.0"
        }}
    )
    
    # 分页字段（可选，仅列表时）
    pagination: Optional[Pagination] = Field(
        None,
        description="分页信息。仅当data为列表且支持分页时存在",
        json_schema_extra={"example": {
            "page": 1,
            "page_size": 20,
            "total": 100,
            "total_pages": 5,
            "has_next": True,
            "has_prev": False
        }}
    )
    
    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "success": True,
                "message": "Operation completed successfully",
                "data": {"result": "ok"},
                "meta": {
                    "timestamp": "2025-10-01T12:00:00.000Z",
                    "request_id": "req_abc123",
                    "execution_time_ms": 150,
                    "api_version": "2.0.0"
                }
            }
        }
    )

