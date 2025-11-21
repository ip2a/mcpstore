"""
MCPStore Data Models Unified Import Module

Provides unified import interface for all data models, avoiding duplicate definitions and import confusion.
"""

# ==================== 核心响应架构 ====================
# 响应模型
from .response import (
    APIResponse,
    ErrorDetail,
    ResponseMeta,
    Pagination
)

# 响应构造器
from .response_builder import (
    ResponseBuilder,
    TimedResponseBuilder
)

# 错误码枚举
from .error_codes import ErrorCode

# 响应装饰器
from .response_decorators import (
    timed_response,
    paginated,
    handle_errors,
    api_endpoint
)

# Client-related models
from .client import (
    ClientRegistrationRequest
)

# Common response models (兼容性保留)
from .common import (
    ListResponse,
    DataResponse,
    RegistrationResponse,
    ExecutionResponse,
    ConfigResponse,
    HealthResponse
)
# Service-related models
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
    TransportType,
    ServiceConnectionState,
    ServiceStateMetadata
)
# Tool-related models
from .tool import (
    ToolInfo,
    ToolsResponse,
    ToolExecutionRequest
)

# Configuration management related
try:
    from ..configuration.unified_config import UnifiedConfigManager, ConfigType, ConfigInfo
except ImportError:
    # Avoid circular import issues
    pass

# Export all models for convenient external import
__all__ = [
    # ==================== Response Architecture ====================
    # Response models
    'APIResponse',
    'ErrorDetail',
    'ResponseMeta',
    'Pagination',
    
    # Response builders
    'ResponseBuilder',
    'TimedResponseBuilder',
    
    # Error codes
    'ErrorCode',
    
    # Response decorators
    'timed_response',
    'paginated',
    'handle_errors',
    'api_endpoint',
    
    # ==================== Domain Models ====================
    # Service models
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
    'ServiceConnectionState',
    'ServiceStateMetadata',

    # Tool models
    'ToolInfo',
    'ToolsResponse',
    'ToolExecutionRequest',

    # Client models
    'ClientRegistrationRequest',

    # Common response models (兼容性保留)
    'ListResponse',
    'DataResponse',
    'RegistrationResponse',
    'ExecutionResponse',
    'ConfigResponse',
    'HealthResponse'
]
