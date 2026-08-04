from __future__ import annotations

from typing import Any, List


class CrewAIAdapter:
    """CrewAI-compatible adapter that reuses LangChain tool objects."""

    def __init__(self, context: Any, instance_id: str | None = None):
        self._context = context
        self._instance_id = instance_id

    def list_tools(self) -> List[object]:
        if self._instance_id is None:
            return self._context.for_langchain().list_tools()
        return self._context.for_langchain(self._instance_id).list_tools()
