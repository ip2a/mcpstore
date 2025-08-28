"""
Integration layer modules for external systems (FastMCP, HTTP transport, OpenAPI, etc.).

This package consolidates previously scattered integration files under a single namespace
without changing any public APIs. Original modules under mcpstore.core keep thin proxy
re-exports to maintain full backward compatibility.
"""

from .fastmcp_integration import FastMCPServiceManager, get_fastmcp_service_manager

__all__ = [
    "FastMCPServiceManager",
    "get_fastmcp_service_manager",
]

