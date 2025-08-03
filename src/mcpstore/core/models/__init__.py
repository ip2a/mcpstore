"""
MCPStore Data Models Unified Import Module

Provides unified import interface for all data models, avoiding duplicate definitions and import confusion.
"""

# Client-related models
from .client import (
    ClientRegistrationRequest
)
# Common response models
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
    from ..unified_config import UnifiedConfigManager, ConfigType, ConfigInfo
except ImportError:
    # Avoid circular import issues
    pass

# Export all models for convenient external import
__all__ = [
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

    # Common response models
    'BaseResponse',
    'APIResponse',
    'ListResponse',
    'DataResponse',
    'RegistrationResponse',
    'ExecutionResponse',
    'ConfigResponse',
    'HealthResponse'
]
