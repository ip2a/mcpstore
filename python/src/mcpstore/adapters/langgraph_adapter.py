from __future__ import annotations

from typing import Any, List


class LangGraphAdapter:
    """LangGraph uses LangChain-compatible tools under the hood."""

    def __init__(self, context: Any, instance_id: str, response_format: str = "text"):
        self._context = context
        self._instance_id = instance_id
        self._response_format = response_format if response_format in ("text", "content_and_artifact") else "text"

    def list_tools(self) -> List[object]:
        return self._context.for_langchain(
            self._instance_id,
            response_format=self._response_format,
        ).list_tools()

    async def list_tools_async(self) -> List[object]:
        return await self._context.for_langchain(
            self._instance_id,
            response_format=self._response_format,
        ).list_tools_async()
