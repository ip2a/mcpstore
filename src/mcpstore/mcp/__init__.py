"""mcpstore.mcp - MCP 高层封装（对标 fastmcp）。"""

import warnings
from importlib.metadata import version as _version

from mcpstore.mcp.settings import Settings

settings = Settings()

from mcpstore.mcp.server.server import MCPKit, MCPStore
from mcpstore.mcp.server.context import Context
import mcpstore.mcp.server  # noqa: F401

from mcpstore.mcp.client import Client
from . import client

__version__ = _version("mcpstore")


# ensure deprecation warnings are displayed by default
if settings.deprecation_warnings:
    warnings.simplefilter("default", DeprecationWarning)


__all__ = [
    "Client",
    "Context",
    "MCPKit",
    "MCPStore",
    "settings",
]
