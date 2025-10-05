"""
MCPStore Lifecycle Management Module
Lifecycle management module

Responsible for service lifecycle, health monitoring, content management and intelligent reconnection
"""

from .config import ServiceLifecycleConfig
from .content_manager import ServiceContentManager
from .health_bridge import HealthStatusBridge
from .health_manager import get_health_manager, HealthStatus, HealthCheckResult
# Main exports - maintain backward compatibility
from .manager import ServiceLifecycleManager
from .smart_reconnection import SmartReconnectionManager

# 🆕 事件驱动架构：UnifiedServiceStateManager 已被废弃

__all__ = [
    'ServiceLifecycleManager',
    'ServiceContentManager',
    'get_health_manager',
    'HealthStatus',
    'HealthCheckResult',
    'SmartReconnectionManager',
    'ServiceLifecycleConfig',
    'HealthStatusBridge',
]

# For backward compatibility, also export some commonly used types
try:
    from mcpstore.core.models.service import ServiceConnectionState, ServiceStateMetadata
    __all__.extend(['ServiceConnectionState', 'ServiceStateMetadata'])
except ImportError:
    pass
