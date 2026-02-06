"""OpenAPI component implementations - backwards compatibility stub.

This module is deprecated. Import from mcpstore.mcp.server.providers.openapi instead.
"""

from __future__ import annotations

import warnings

warnings.warn(
    "mcpstore.mcp.server.openapi.components is deprecated. "
    "Import from mcpstore.mcp.server.providers.openapi instead.",
    DeprecationWarning,
    stacklevel=2,
)

from mcpstore.mcp.server.providers.openapi import (  # noqa: E402
    OpenAPIResource,
    OpenAPIResourceTemplate,
    OpenAPITool,
)

# Export public symbols
__all__ = [
    "OpenAPIResource",
    "OpenAPIResourceTemplate",
    "OpenAPITool",
]
