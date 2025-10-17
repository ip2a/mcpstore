"""
MCPStore Lifecycle Management Module
Lifecycle management module

Responsible for service lifecycle, health monitoring, content management and intelligent reconnection
"""

from .config import ServiceLifecycleConfig
from .content_manager import ServiceContentManager

# 事件驱动架构统一导出：仅保留核心组件
__all__ = [
    'ServiceContentManager',
    'ServiceLifecycleConfig',
]

# For backward compatibility, also export some commonly used types
try:
    from mcpstore.core.models.service import ServiceConnectionState, ServiceStateMetadata
    __all__.extend(['ServiceConnectionState', 'ServiceStateMetadata'])
except ImportError:
    pass
