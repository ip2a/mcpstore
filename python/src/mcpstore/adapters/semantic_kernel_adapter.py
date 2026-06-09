from __future__ import annotations

from typing import Any, Callable, List

from .common import (
    attach_signature_from_schema,
    build_sync_executor,
    create_args_schema,
    tool_name,
)


class SemanticKernelAdapter:
    """Produces Python callables suitable for Semantic Kernel native functions."""

    def __init__(self, context: Any):
        self._context = context

    def list_tools(self) -> List[Callable[..., Any]]:
        return self._build_functions(self._context.list_tools())

    async def list_tools_async(self) -> List[Callable[..., Any]]:
        return self._build_functions(await self._context.list_tools_async())

    def get_functions(self) -> List[Callable[..., Any]]:
        return self.list_tools()

    async def get_functions_async(self) -> List[Callable[..., Any]]:
        return await self.list_tools_async()

    def _build_functions(self, mcp_tools: List[Any]) -> List[Callable[..., Any]]:
        functions: List[Callable[..., Any]] = []
        for tool_info in mcp_tools:
            name = tool_name(tool_info)
            args_schema = create_args_schema(tool_info)
            fn = build_sync_executor(self._context, name, args_schema)
            attach_signature_from_schema(fn, args_schema)
            functions.append(fn)
        return functions
