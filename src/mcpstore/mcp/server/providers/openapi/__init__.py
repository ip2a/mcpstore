"""OpenAPI provider for MCPStore.

This module provides OpenAPI integration for MCPStore through the Provider pattern.

Example:
    ```python
    from mcpstore.mcp import MCPStore
    from mcpstore.mcp.server.providers.openapi import OpenAPIProvider
    import httpx

    client = httpx.AsyncClient(base_url="https://api.example.com")
    provider = OpenAPIProvider(openapi_spec=spec, client=client)
    mcp = MCPStore("API Server", providers=[provider])
    ```
"""

from mcpstore.mcp.server.providers.openapi.components import (
    OpenAPIResource,
    OpenAPIResourceTemplate,
    OpenAPITool,
)
from mcpstore.mcp.server.providers.openapi.provider import OpenAPIProvider
from mcpstore.mcp.server.providers.openapi.routing import (
    ComponentFn,
    MCPType,
    RouteMap,
    RouteMapFn,
)

__all__ = [
    "ComponentFn",
    "MCPType",
    "OpenAPIProvider",
    "OpenAPIResource",
    "OpenAPIResourceTemplate",
    "OpenAPITool",
    "RouteMap",
    "RouteMapFn",
]
