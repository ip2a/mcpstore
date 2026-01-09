"""
MCPStore Lifecycle Management Module
Lifecycle management module

Responsible for service lifecycle, health monitoring, content management and intelligent reconnection
"""

from .config import ServiceLifecycleConfig
from .content_manager import ServiceContentManager

# Event-driven architecture unified export: only keep core components
__all__ = [
    'ServiceContentManager',
    'ServiceLifecycleConfig',
]

# 导出常用类型
try:
    from mcpstore.core.models.service import ServiceConnectionState, ServiceStateMetadata
    __all__.extend(['ServiceConnectionState', 'ServiceStateMetadata'])
except ImportError:
    pass
