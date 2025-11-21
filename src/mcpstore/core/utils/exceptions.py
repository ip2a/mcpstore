"""
MCP Store exception class definitions (Legacy)

This module is deprecated. Import from mcpstore.core.exceptions instead.
"""

import warnings

# Import from unified exception system
from mcpstore.core.exceptions import (
    MCPStoreException as MCPStoreError,
    ServiceNotFoundException as ServiceNotFoundError,
    ConfigurationException as InvalidConfigError,
    ServiceUnavailableError as DeleteServiceError,
    ConfigurationException as ConfigurationError,
    ServiceConnectionError,
    ToolExecutionError,
)

# Deprecation warning
warnings.warn(
    "mcpstore.core.utils.exceptions is deprecated. "
    "Import from mcpstore.core.exceptions instead.",
    DeprecationWarning,
    stacklevel=2
)

__all__ = [
    "MCPStoreError",
    "ServiceNotFoundError",
    "InvalidConfigError",
    "DeleteServiceError",
    "ConfigurationError",
    "ServiceConnectionError",
    "ToolExecutionError",
]
