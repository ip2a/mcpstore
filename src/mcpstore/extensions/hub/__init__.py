"""
MCPStore Hub Module
Hub module - Distributed service packaging and management functionality

Implementation Plan 1: Hub Service Functionality (Basic Distributed Architecture)
- Service packaging: Package cached services into independent Hub services
- Distributed architecture: Each Hub runs in an independent process
- Basic routing: Support /mcp global access
- Process management: Hub process startup, stop, monitoring

Design principles:
- Based on existing service cache, no duplicate service registration
- Use FastMCP as MCP server implementation
- Complete process isolation
- Seamless integration with existing MCPStore architecture
"""

from .builder import HubServicesBuilder, HubToolsBuilder
from .package import HubPackage
from .process import HubProcess
from .server import HubServerGenerator
from .types import HubConfig, HubStatus

__all__ = [
    # Core classes
    'HubServicesBuilder',
    'HubToolsBuilder', 
    'HubPackage',
    'HubProcess',
    'HubServerGenerator',
    
    # Types
    'HubConfig',
    'HubStatus'
]

__version__ = "1.0.0"
__description__ = "MCPStore Hub Module - Distributed service packaging and management"
