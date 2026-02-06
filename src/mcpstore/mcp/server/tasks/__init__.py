"""MCP SEP-1686 background tasks support.

This module implements protocol-level background task execution for MCP servers.
"""

from mcpstore.mcp.server.tasks.capabilities import get_task_capabilities
from mcpstore.mcp.server.tasks.config import TaskConfig, TaskMeta, TaskMode
from mcpstore.mcp.server.tasks.keys import (
    build_task_key,
    get_client_task_id_from_key,
    parse_task_key,
)

__all__ = [
    "TaskConfig",
    "TaskMeta",
    "TaskMode",
    "build_task_key",
    "get_client_task_id_from_key",
    "get_task_capabilities",
    "parse_task_key",
]
