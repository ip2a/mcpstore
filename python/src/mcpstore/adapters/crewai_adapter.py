from __future__ import annotations

from typing import Any, List


class CrewAIAdapter:
    """CrewAI-compatible adapter that reuses LangChain tool objects."""

    def __init__(self, context: Any, instance_id: str):
        self._context = context
        self._instance_id = instance_id

    def list_tools(self) -> List[object]:
        return self._context.for_langchain(self._instance_id).list_tools()

    async def list_tools_async(self) -> List[object]:
        return await self._context.for_langchain(self._instance_id).list_tools_async()
