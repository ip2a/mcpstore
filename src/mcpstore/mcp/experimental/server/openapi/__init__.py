"""Deprecated: Import from mcpstore.mcp.server.providers.openapi instead."""

import warnings

# Deprecated in 2.14 when OpenAPI support was promoted out of experimental
warnings.warn(
    "Importing from mcpstore.mcp.experimental.server.openapi is deprecated. "
    "Import from mcpstore.mcp.server.providers.openapi instead.",
    DeprecationWarning,
    stacklevel=2,
)

# Import from canonical location
from mcpstore.mcp.server.openapi.server import MCPStoreOpenAPI as MCPStoreOpenAPI  # noqa: E402
from mcpstore.mcp.server.providers.openapi import (  # noqa: E402
    ComponentFn as ComponentFn,
    MCPType as MCPType,
    OpenAPIResource as OpenAPIResource,
    OpenAPIResourceTemplate as OpenAPIResourceTemplate,
    OpenAPITool as OpenAPITool,
    RouteMap as RouteMap,
    RouteMapFn as RouteMapFn,
)
from mcpstore.mcp.server.providers.openapi.routing import (  # noqa: E402
    DEFAULT_ROUTE_MAPPINGS as DEFAULT_ROUTE_MAPPINGS,
    _determine_route_type as _determine_route_type,
)

__all__ = [
    "DEFAULT_ROUTE_MAPPINGS",
    "ComponentFn",
    "MCPStoreOpenAPI",
    "MCPType",
    "OpenAPIResource",
    "OpenAPIResourceTemplate",
    "OpenAPITool",
    "RouteMap",
    "RouteMapFn",
    "_determine_route_type",
]
