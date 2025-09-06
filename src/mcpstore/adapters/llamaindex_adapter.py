# src/mcpstore/adapters/llamaindex_adapter.py
from __future__ import annotations

import json
from typing import List, TYPE_CHECKING, Callable

from pydantic import BaseModel
from .common import enhance_description, create_args_schema, build_sync_executor

# TYPE_CHECKING to avoid runtime circular imports
if TYPE_CHECKING:
    from ..core.context.base_context import MCPStoreContext
    from ..core.models.tool import ToolInfo




class LlamaIndexAdapter:
    """
    Adapter from MCPStore ToolInfo -> LlamaIndex FunctionTool list.
    """
    def __init__(self, context: 'MCPStoreContext'):
        self._context = context

    def list_tools(self) -> List[object]:
        return self._context._sync_helper.run_async(self.list_tools_async())

    async def list_tools_async(self) -> List[object]:
        try:
            from llama_index.core.tools import FunctionTool
        except Exception as e:
            raise ImportError("LlamaIndex adapter requires 'llama-index' (llama_index). Install: pip install llama-index") from e

        mcp_tools: List['ToolInfo'] = await self._context.list_tools_async()
        tools: List[object] = []
        for t in mcp_tools:
            args_schema = create_args_schema(t)
            sync_fn = build_sync_executor(self._context, t.name, args_schema)
            desc = enhance_description(t)
            # LlamaIndex primarily accepts sync functions; name/description can be set via from_defaults
            li_tool = FunctionTool.from_defaults(fn=sync_fn, name=t.name, description=desc)
            tools.append(li_tool)
        return tools

