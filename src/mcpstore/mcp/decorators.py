"""Shared decorator utilities for MCPStore."""

from __future__ import annotations

import inspect
from typing import TYPE_CHECKING, Any, Protocol, runtime_checkable

if TYPE_CHECKING:
    from mcpstore.mcp.prompts.function_prompt import PromptMeta
    from mcpstore.mcp.resources.function_resource import ResourceMeta
    from mcpstore.mcp.server.tasks.config import TaskConfig
    from mcpstore.mcp.tools.function_tool import ToolMeta

    MCPStoreMeta = ToolMeta | ResourceMeta | PromptMeta


def resolve_task_config(task: bool | TaskConfig | None) -> bool | TaskConfig:
    """Resolve task config, defaulting None to False."""
    return task if task is not None else False


@runtime_checkable
class HasMCPStoreMeta(Protocol):
    """Protocol for callables decorated with MCPStore metadata."""

    __mcpstore__: Any


def get_mcpstore_meta(fn: Any) -> Any | None:
    """Extract MCPStore metadata from a function, handling bound methods and wrappers."""
    if hasattr(fn, "__mcpstore__"):
        return fn.__mcpstore__
    if hasattr(fn, "__func__") and hasattr(fn.__func__, "__mcpstore__"):
        return fn.__func__.__mcpstore__
    try:
        unwrapped = inspect.unwrap(fn)
        if unwrapped is not fn and hasattr(unwrapped, "__mcpstore__"):
            return unwrapped.__mcpstore__
    except ValueError:
        pass
    return None
