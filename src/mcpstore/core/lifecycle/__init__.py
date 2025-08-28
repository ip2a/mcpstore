"""
MCPStore Lifecycle Management Module
Lifecycle management module

Responsible for service lifecycle, health monitoring, content management and intelligent reconnection
"""

# Main exports - maintain backward compatibility
from .manager import ServiceLifecycleManager
from .content_manager import ServiceContentManager
from .health_manager import get_health_manager, HealthStatus, HealthCheckResult
from .smart_reconnection import SmartReconnectionManager
from .config import ServiceLifecycleConfig
from .health_bridge import HealthStatusBridge
from .unified_state_manager import UnifiedServiceStateManager

__all__ = [
    'ServiceLifecycleManager',
    'ServiceContentManager',
    'get_health_manager',
    'HealthStatus',
    'HealthCheckResult',
    'SmartReconnectionManager',
    'ServiceLifecycleConfig',
    'HealthStatusBridge',
    'UnifiedServiceStateManager'
]

# For backward compatibility, also export some commonly used types
try:
    from mcpstore.core.models.service import ServiceConnectionState, ServiceStateMetadata
    __all__.extend(['ServiceConnectionState', 'ServiceStateMetadata'])
except ImportError:
    pass
