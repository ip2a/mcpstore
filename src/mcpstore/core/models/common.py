"""
MCPStore 通用响应模型

提供统一的响应格式，减少重复的响应模型定义。
"""

from pydantic import BaseModel, Field
from typing import Optional, Any, List, Dict, Generic, TypeVar

# 泛型类型变量
T = TypeVar('T')

class BaseResponse(BaseModel):
    """统一的基础响应模型"""
    success: bool = Field(..., description="操作是否成功")
    message: Optional[str] = Field(None, description="响应消息")

class APIResponse(BaseResponse):
    """通用API响应模型"""
    data: Optional[Any] = Field(None, description="响应数据")
    metadata: Optional[Dict[str, Any]] = Field(None, description="元数据信息")
    execution_info: Optional[Dict[str, Any]] = Field(None, description="执行信息")

class ListResponse(BaseResponse, Generic[T]):
    """列表响应模型"""
    items: List[T] = Field(..., description="数据项列表")
    total: int = Field(..., description="总数量")

class DataResponse(BaseResponse, Generic[T]):
    """单个数据项响应模型"""
    data: T = Field(..., description="数据项")

class RegistrationResponse(BaseResponse):
    """注册操作响应模型"""
    client_id: str = Field(..., description="客户端ID")
    service_names: List[str] = Field(..., description="服务名列表")
    config: Dict[str, Any] = Field(..., description="配置信息")

class ExecutionResponse(BaseResponse):
    """执行操作响应模型"""
    result: Optional[Any] = Field(None, description="执行结果")
    error: Optional[str] = Field(None, description="错误信息")

class ConfigResponse(BaseResponse):
    """配置响应模型"""
    client_id: str = Field(..., description="客户端ID")
    config: Dict[str, Any] = Field(..., description="配置信息")

class HealthResponse(BaseResponse):
    """健康检查响应模型"""
    service_name: str = Field(..., description="服务名称")
    status: str = Field(..., description="健康状态")
    last_check: Optional[str] = Field(None, description="最后检查时间")

# 这些别名已被删除，直接使用新的统一响应模型
