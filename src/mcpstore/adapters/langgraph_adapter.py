# src/mcpstore/adapters/langgraph_adapter.py
from __future__ import annotations

from typing import List, TYPE_CHECKING

if TYPE_CHECKING:
    from ..core.context.base_context import MCPStoreContext

class LangGraphAdapter:
    """
    LangGraph uses LangChain tool ecosystem under the hood.
    We reuse the LangChain adapter output for zero extra dependency.
    """
    def __init__(self, context: 'MCPStoreContext'):
        self._context = context

    def list_tools(self) -> List[object]:
        lc_adapter = self._context.for_langchain()
        return lc_adapter.list_tools()

    async def list_tools_async(self) -> List[object]:
        lc_adapter = self._context.for_langchain()
        return await lc_adapter.list_tools_async()

