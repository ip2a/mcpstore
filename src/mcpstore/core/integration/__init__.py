"""
Integration layer modules for external systems (MCPStore, HTTP transport, OpenAPI, etc.).

This package consolidates previously scattered integration files under a single namespace
without changing any public APIs. Original modules under mcpstore.core keep thin proxy
re-exports to maintain full backward compatibility.
"""

from .mcpstore_integration import MCPStoreServiceManager, get_mcpstore_service_manager

__all__ = [
    "MCPStoreServiceManager",
    "get_mcpstore_service_manager",
]

