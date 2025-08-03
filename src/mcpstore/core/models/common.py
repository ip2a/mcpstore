"""
MCPStore Common Response Models

Provides unified response format, reducing duplicate response model definitions.
"""

from typing import Optional, Any, List, Dict, Generic, TypeVar

from pydantic import BaseModel, Field

# Generic type variable
T = TypeVar('T')

class BaseResponse(BaseModel):
    """Unified base response model"""
    success: bool = Field(..., description="Whether operation was successful")
    message: Optional[str] = Field(None, description="Response message")

class APIResponse(BaseResponse):
    """Common API response model"""
    data: Optional[Any] = Field(None, description="Response data")
    metadata: Optional[Dict[str, Any]] = Field(None, description="Metadata information")
    execution_info: Optional[Dict[str, Any]] = Field(None, description="Execution information")

class ListResponse(BaseResponse, Generic[T]):
    """List response model"""
    items: List[T] = Field(..., description="Data item list")
    total: int = Field(..., description="Total count")

class DataResponse(BaseResponse, Generic[T]):
    """Single data item response model"""
    data: T = Field(..., description="Data item")

class RegistrationResponse(BaseResponse):
    """Registration operation response model"""
    client_id: str = Field(..., description="Client ID")
    service_names: List[str] = Field(..., description="Service name list")
    config: Dict[str, Any] = Field(..., description="Configuration information")

class ExecutionResponse(BaseResponse):
    """Execution operation response model"""
    result: Optional[Any] = Field(None, description="Execution result")
    error: Optional[str] = Field(None, description="Error information")

class ConfigResponse(BaseResponse):
    """Configuration response model"""
    client_id: str = Field(..., description="Client ID")
    config: Dict[str, Any] = Field(..., description="Configuration information")

class HealthResponse(BaseResponse):
    """健康检查响应模型"""
    service_name: str = Field(..., description="服务名称")
    status: str = Field(..., description="健康状态")
    last_check: Optional[str] = Field(None, description="最后检查时间")

# 这些别名已被删除，直接使用新的统一响应模型
