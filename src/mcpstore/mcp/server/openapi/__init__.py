"""OpenAPI server implementation for MCPStore.

.. deprecated::
    This module is deprecated. Import from mcpstore.mcp.server.providers.openapi instead.

The recommended approach is to use OpenAPIProvider with MCPStore:

    from mcpstore.mcp import MCPStore
    from mcpstore.mcp.server.providers.openapi import OpenAPIProvider
    import httpx

    client = httpx.AsyncClient(base_url="https://api.example.com")
    provider = OpenAPIProvider(openapi_spec=spec, client=client)

    mcp = MCPStore("My API Server")
    mcp.add_provider(provider)

MCPStoreOpenAPI is still available but deprecated.
"""

import warnings

warnings.warn(
    "mcpstore.mcp.server.openapi is deprecated. "
    "Import from mcpstore.mcp.server.providers.openapi instead.",
    DeprecationWarning,
    stacklevel=2,
)

# Re-export from new canonical location
from mcpstore.mcp.server.providers.openapi import (  # noqa: E402
    ComponentFn as ComponentFn,
    MCPType as MCPType,
    OpenAPIProvider as OpenAPIProvider,
    OpenAPIResource as OpenAPIResource,
    OpenAPIResourceTemplate as OpenAPIResourceTemplate,
    OpenAPITool as OpenAPITool,
    RouteMap as RouteMap,
    RouteMapFn as RouteMapFn,
)

# Keep MCPStoreOpenAPI for backwards compat (it has its own deprecation warning)
from mcpstore.mcp.server.openapi.server import MCPStoreOpenAPI as MCPStoreOpenAPI  # noqa: E402

__all__ = [
    "ComponentFn",
    "MCPStoreOpenAPI",
    "MCPType",
    "OpenAPIProvider",
    "OpenAPIResource",
    "OpenAPIResourceTemplate",
    "OpenAPITool",
    "RouteMap",
    "RouteMapFn",
]
