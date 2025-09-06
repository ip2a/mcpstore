# src/mcpstore/adapters/autogen_adapter.py
from __future__ import annotations

import inspect
from typing import List, TYPE_CHECKING, Callable, Any

from pydantic import BaseModel
from .common import create_args_schema, enhance_description, build_sync_executor, attach_signature_from_schema

if TYPE_CHECKING:
    from ..core.context.base_context import MCPStoreContext
    from ..core.models.tool import ToolInfo






class AutoGenAdapter:
    """
    Adapter that produces plain Python functions suitable for AutoGen tool registration.
    """
    def __init__(self, context: 'MCPStoreContext'):
        self._context = context

    def list_tools(self) -> List[Callable[..., Any]]:
        return self._context._sync_helper.run_async(self.list_tools_async())

    async def list_tools_async(self) -> List[Callable[..., Any]]:
        tools: List[Callable[..., Any]] = []
        mcp_tools: List['ToolInfo'] = await self._context.list_tools_async()
        for t in mcp_tools:
            args_schema = create_args_schema(t)
            fn = build_sync_executor(self._context, t.name, args_schema)
            attach_signature_from_schema(fn, args_schema)
            tools.append(fn)
        return tools

