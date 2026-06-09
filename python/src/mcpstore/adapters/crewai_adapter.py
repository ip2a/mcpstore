from __future__ import annotations

from typing import Any, List


class CrewAIAdapter:
    """CrewAI-compatible adapter that reuses LangChain tool objects."""

    def __init__(self, context: Any):
        self._context = context

    def list_tools(self) -> List[object]:
        return self._context.for_langchain().list_tools()

    async def list_tools_async(self) -> List[object]:
        return await self._context.for_langchain().list_tools_async()
