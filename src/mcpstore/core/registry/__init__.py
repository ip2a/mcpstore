"""
MCPStore Registry Module
Registry module - Unified management of service registration, tool resolution, Schema management and other functions

Refactoring notes:
- Unified previously scattered registration-related files into registry/ module
- Maintains 100% backward compatibility, all existing import paths remain valid
- Centralized function management for easier maintenance and extension

Module structure:
- core_registry.py: Core service registry (original registry.py)
- tool_resolver.py: Tool name resolver
- types.py: Registration-related type definitions
"""

__all__ = [
    # Core registry
    'ServiceRegistry',
    'SessionProtocol',
    'SessionType',

    # Tool resolution
    'ToolNameResolver',
    'ToolResolution',

    # Type definitions
    'RegistryTypes',

    # Compatibility exports
    'ServiceConnectionState',
    'ServiceStateMetadata'
]

# Main exports - maintain backward compatibility
from .core_registry import ServiceRegistry, SessionProtocol, SessionType
# SchemaManager removed in single-source mode; no longer exported
from .tool_resolver import ToolNameResolver, ToolResolution
from .types import RegistryTypes

# For backward compatibility, also export some commonly used types
try:
    from ..models.service import ServiceConnectionState, ServiceStateMetadata
    __all__.extend(['ServiceConnectionState', 'ServiceStateMetadata'])
except ImportError:
    pass

# Version information
__version__ = "1.0.0"
__author__ = "MCPStore Team"
__description__ = "Registry module for MCPStore - Service registration, tool resolution, and schema management"
