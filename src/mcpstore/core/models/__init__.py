"""
MCPStore 数据模型统一导入模块

提供所有数据模型的统一导入接口，避免重复定义和导入混乱。
"""

# 服务相关模型
from .service import (
    ServiceInfo,
    ServiceInfoResponse,
    ServicesResponse,
    RegisterRequestUnion,
    JsonUpdateRequest,
    ServiceConfig,
    URLServiceConfig,
    CommandServiceConfig,
    MCPServerConfig,
    ServiceConfigUnion,
    AddServiceRequest,
    TransportType
)

# 工具相关模型
from .tool import (
    ToolInfo,
    ToolsResponse,
    ToolExecutionRequest
)

# 客户端相关模型
from .client import (
    ClientRegistrationRequest
)

# 通用响应模型
from .common import (
    BaseResponse,
    APIResponse,
    ListResponse,
    DataResponse,
    RegistrationResponse,
    ExecutionResponse,
    ConfigResponse,
    HealthResponse
)

# 配置管理相关
try:
    from ..unified_config import UnifiedConfigManager, ConfigType, ConfigInfo
except ImportError:
    # 避免循环导入问题
    pass

# 导出所有模型，方便外部导入
__all__ = [
    # 服务模型
    'ServiceInfo',
    'ServiceInfoResponse',
    'ServicesResponse',
    'RegisterRequestUnion',
    'JsonUpdateRequest',
    'ServiceConfig',
    'URLServiceConfig',
    'CommandServiceConfig',
    'MCPServerConfig',
    'ServiceConfigUnion',
    'AddServiceRequest',
    'TransportType',

    # 工具模型
    'ToolInfo',
    'ToolsResponse',
    'ToolExecutionRequest',

    # 客户端模型
    'ClientRegistrationRequest',

    # 通用响应模型
    'BaseResponse',
    'APIResponse',
    'ListResponse',
    'DataResponse',
    'RegistrationResponse',
    'ExecutionResponse',
    'ConfigResponse',
    'HealthResponse'
]
