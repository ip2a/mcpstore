# src/mcpstore/adapters/autogen_adapter.py
from __future__ import annotations

from typing import List, Callable, Any

from .common import (
    attach_signature_from_schema,
    build_sync_executor,
    create_args_schema,
    tool_name,
)

class AutoGenAdapter:
    """
    Adapter that produces plain Python functions suitable for AutoGen tool registration.
    """
    def __init__(self, context: Any):
        self._context = context

    def list_tools(self) -> List[Callable[..., Any]]:
        tools: List[Callable[..., Any]] = []
        for t in self._context.list_tools():
            args_schema = create_args_schema(t)
            fn = build_sync_executor(self._context, tool_name(t), args_schema)
            attach_signature_from_schema(fn, args_schema)
            tools.append(fn)
        return tools

    async def list_tools_async(self) -> List[Callable[..., Any]]:
        tools: List[Callable[..., Any]] = []
        mcp_tools: List[Any] = await self._context.list_tools_async()
        for t in mcp_tools:
            args_schema = create_args_schema(t)
            fn = build_sync_executor(self._context, tool_name(t), args_schema)
            attach_signature_from_schema(fn, args_schema)
            tools.append(fn)
        return tools
