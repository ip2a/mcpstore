# src/mcpstore/adapters/semantic_kernel_adapter.py
from __future__ import annotations

from typing import List, TYPE_CHECKING, Callable, Any

from pydantic import BaseModel
from .common import create_args_schema, build_sync_executor

if TYPE_CHECKING:
    from ..core.context.base_context import MCPStoreContext
    from ..core.models.tool import ToolInfo




class SemanticKernelAdapter:
    """
    Produce Python callables that can be registered as native functions in Semantic Kernel.
    Caller can register them into Kernel/Plugin as needed.
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
            tools.append(fn)
        return tools

