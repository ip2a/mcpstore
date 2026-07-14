from __future__ import annotations

from typing import Any, List

from .common import (
    build_sync_executor,
    create_args_schema,
    enhance_description,
    tool_instance_id,
    tool_name,
)


class LlamaIndexAdapter:
    """Adapter from MCPStore tools to LlamaIndex FunctionTool objects."""

    def __init__(self, context: Any, instance_id: str):
        self._context = context
        self._instance_id = instance_id

    def list_tools(self) -> List[object]:
        return self._build_tools(self._context.list_tools(self._instance_id))

    async def list_tools_async(self) -> List[object]:
        return self._build_tools(await self._context.list_tools_async(self._instance_id))

    def _build_tools(self, mcp_tools: List[Any]) -> List[object]:
        try:
            from llama_index.core.tools import FunctionTool
        except Exception as error:
            raise ImportError(
                "LlamaIndex adapter requires `llama-index`. Install it before using LlamaIndexAdapter."
            ) from error

        tools: List[object] = []
        for tool_info in mcp_tools:
            name = tool_name(tool_info)
            args_schema = create_args_schema(tool_info)
            sync_fn = build_sync_executor(
                self._context,
                tool_instance_id(tool_info),
                name,
                args_schema,
            )
            tools.append(
                FunctionTool.from_defaults(
                    fn=sync_fn,
                    name=name,
                    description=enhance_description(tool_info),
                )
            )
        return tools
