"""
MCPStore - Intelligent Agent Tool Service Store
Provides simple and easy-to-use MCP tool management and invocation functionality
"""

from mcpstore.config.config import LoggingConfig
from mcpstore.core.store import MCPStore

__version__ = "0.5.0"
__all__ = ["MCPStore", "LoggingConfig"]
