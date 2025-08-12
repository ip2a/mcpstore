"""
Registry Types
Type definitions related to the registry module

Contains all type definitions used in the registry module for unified management and import.
"""

from typing import Dict, Any, Optional, List, Set, TypeVar, Protocol
from datetime import datetime

# Re-export model types for unified import
try:
    from ..models.service import ServiceConnectionState, ServiceStateMetadata
except ImportError:
    # If model import fails, provide placeholders
    ServiceConnectionState = None
    ServiceStateMetadata = None

# Define a protocol representing any session type with call_tool method
class SessionProtocol(Protocol):
    """Session protocol - defines interface that sessions must implement"""
    async def call_tool(self, tool_name: str, tool_args: Dict[str, Any]) -> Any:
        """Call tool method"""
        ...

# Session type variable
SessionType = TypeVar('SessionType')

# Registration-related type aliases
AgentId = str
ServiceName = str
ToolName = str
ClientId = str

# Registration data structure types
SessionsDict = Dict[AgentId, Dict[ServiceName, Any]]
ToolCacheDict = Dict[AgentId, Dict[ToolName, Any]]
ToolToSessionDict = Dict[AgentId, Dict[ToolName, Any]]
ServiceHealthDict = Dict[AgentId, Dict[ServiceName, datetime]]

class RegistryTypes:
    """Registry type collection - for unified management of all types"""

    # Basic types
    AgentId = AgentId
    ServiceName = ServiceName
    ToolName = ToolName
    ClientId = ClientId
    
    # Protocol types
    SessionProtocol = SessionProtocol
    SessionType = SessionType
    
    # 数据结构类型
    SessionsDict = SessionsDict
    ToolCacheDict = ToolCacheDict
    ToolToSessionDict = ToolToSessionDict
    ServiceHealthDict = ServiceHealthDict
    
    # 模型类型
    ServiceConnectionState = ServiceConnectionState
    ServiceStateMetadata = ServiceStateMetadata

__all__ = [
    'SessionProtocol',
    'SessionType',
    'AgentId',
    'ServiceName', 
    'ToolName',
    'ClientId',
    'SessionsDict',
    'ToolCacheDict',
    'ToolToSessionDict',
    'ServiceHealthDict',
    'RegistryTypes',
    'ServiceConnectionState',
    'ServiceStateMetadata'
]
