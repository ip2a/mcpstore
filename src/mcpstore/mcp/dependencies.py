"""Dependency injection exports for MCPStore.

This module re-exports dependency injection symbols from Docket and MCPStore
to provide a clean, centralized import location for all dependency-related
functionality.

DI features (Depends, CurrentContext, CurrentMCPStore) work without pydocket
using a vendored DI engine. Only task-related dependencies (CurrentDocket,
CurrentWorker) and background task execution require mcpstore[tasks].
"""

# Try docket first for isinstance compatibility, fall back to vendored
try:
    from docket import Depends
except ImportError:
    from mcpstore.mcp._vendor.docket_di import Depends


from mcpstore.mcp.server.dependencies import (
    CurrentAccessToken,
    CurrentContext,
    CurrentDocket,
    CurrentMCPStore,
    CurrentHeaders,
    CurrentRequest,
    CurrentWorker,
    Progress,
    ProgressLike,
)

__all__ = [
    "CurrentAccessToken",
    "CurrentContext",
    "CurrentDocket",
    "CurrentMCPStore",
    "CurrentHeaders",
    "CurrentRequest",
    "CurrentWorker",
    "Depends",
    "Progress",
    "ProgressLike",
]
