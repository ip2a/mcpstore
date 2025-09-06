# src/mcpstore/adapters/crewai_adapter.py
from __future__ import annotations

from typing import List, TYPE_CHECKING

if TYPE_CHECKING:
    from ..core.context.base_context import MCPStoreContext

class CrewAIAdapter:
    """
    CrewAI often consumes LangChain Tool objects directly.
    We reuse the LangChain adapter to maximize compatibility and avoid extra deps.
    """
    def __init__(self, context: 'MCPStoreContext'):
        self._context = context

    def list_tools(self) -> List[object]:
        # Defer import and reuse for_langchain output
        lc_adapter = self._context.for_langchain()
        return lc_adapter.list_tools()

    async def list_tools_async(self) -> List[object]:
        lc_adapter = self._context.for_langchain()
        return await lc_adapter.list_tools_async()

