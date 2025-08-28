"""
Compatibility proxy module.
The real implementation was moved to mcpstore.core.integration.fastmcp_integration.
This file re-exports symbols to preserve backward compatibility.
"""

from .integration.fastmcp_integration import (
    FastMCPServiceManager,
    get_fastmcp_service_manager,
)

__all__ = [
    "FastMCPServiceManager",
    "get_fastmcp_service_manager",
]

