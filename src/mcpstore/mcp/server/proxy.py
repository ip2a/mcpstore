"""Backwards compatibility - import from mcpstore.mcp.server.providers.proxy instead.

This module re-exports all proxy-related classes from their new location
at mcpstore.mcp.server.providers.proxy. Direct imports from this module are
deprecated and will be removed in a future version.
"""

from __future__ import annotations

import warnings

warnings.warn(
    "mcpstore.mcp.server.proxy is deprecated. Use mcpstore.mcp.server.providers.proxy instead.",
    DeprecationWarning,
    stacklevel=2,
)

# Re-export everything from the new location
from mcpstore.mcp.server.providers.proxy import (  # noqa: E402
    ClientFactoryT,
    MCPStoreProxy,
    ProxyClient,
    ProxyPrompt,
    ProxyProvider,
    ProxyResource,
    ProxyTemplate,
    ProxyTool,
    StatefulProxyClient,
)

__all__ = [
    "ClientFactoryT",
    "MCPStoreProxy",
    "ProxyClient",
    "ProxyPrompt",
    "ProxyProvider",
    "ProxyResource",
    "ProxyTemplate",
    "ProxyTool",
    "StatefulProxyClient",
]
