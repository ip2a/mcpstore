# src/mcpstore/adapters/langchain_adapter.py
"""
LangChain adapter module.

Converts MCPStore tools into the LangChain tool format, with sync and async execution.
"""

from __future__ import annotations

import asyncio
import json
import logging
from dataclasses import asdict, is_dataclass
from typing import Any, Type, List

from pydantic import BaseModel

# Shared helpers
from .common import (
    build_tool_error_payload,
    to_tool_call_view,
    create_args_schema,
    enhance_description,
    process_tool_args,
    tool_instance_id,
    tool_name,
)

_LANGCHAIN_IMPORT_ERROR: ImportError | None

try:
    from langchain_core.tools import Tool, StructuredTool
except ImportError as e:
    _LANGCHAIN_IMPORT_ERROR = e
else:
    _LANGCHAIN_IMPORT_ERROR = None

logger = logging.getLogger(__name__)


def _require_langchain() -> None:
    if _LANGCHAIN_IMPORT_ERROR is not None:
        raise ImportError(
            "The `langchain_core` package is not installed. "
            "Install the LangChain dependencies before using LangChainAdapter."
        ) from _LANGCHAIN_IMPORT_ERROR


class LangChainAdapter:
    """
    Adapter between MCPStore and LangChain.

    Converts MCPStore native objects into objects LangChain can consume directly.
    """

    def __init__(self, context: Any, instance_id: str | None = None, response_format: str = "text"):
        _require_langchain()
        self._context = context
        self._instance_id = instance_id
        # Preferred tool output format
        self._response_format = response_format if response_format in ("text", "content_and_artifact") else "text"

    @staticmethod
    def _serialize_unknown(obj):
        """Serialize an object of unknown type."""
        if obj is None:
            return None
        if hasattr(obj, "model_dump"):
            try:
                return obj.model_dump()
            except Exception:
                pass
        if hasattr(obj, "dict"):
            try:
                return obj.dict()
            except Exception:
                pass
        if is_dataclass(obj):
            try:
                return asdict(obj)
            except Exception:
                pass
        if hasattr(obj, "__dict__"):
            try:
                return {k: v for k, v in obj.__dict__.items() if not k.startswith("_")}
            except Exception:
                pass
        return str(obj)

    def _normalize_structured_value(self, value):
        """Ensure structured/data values are always basic types LangChain can consume."""
        if value is None:
            return None
        if isinstance(value, (str, int, float, bool)):
            return value
        if isinstance(value, (dict, list)):
            return value
        try:
            return json.loads(json.dumps(value, default=self._serialize_unknown, ensure_ascii=False))
        except Exception:
            return str(value)

    def _format_error_output(
        self,
        tool_name: str,
        message: str,
        *,
        tool_input: dict[str, Any] | None = None,
        view=None,
    ):
        payload = build_tool_error_payload(
            tool_name,
            message,
            tool_input=tool_input,
            view=view,
        )
        payload = self._normalize_structured_value(payload)
        if self._response_format == "content_and_artifact":
            return {
                "text": message,
                "artifacts": getattr(view, "artifacts", []) if view is not None else [],
                "structured": payload,
                "data": payload,
            }
        return json.dumps(payload, ensure_ascii=False)

    def _create_tool_function(
        self,
        instance_id: str,
        tool_name: str,
        args_schema: Type[BaseModel],
    ):
        """
        Build a robust sync executor that handles every argument passing style.
        """
        adapter_self = self  # captured by the closure

        def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # Process arguments with the shared helper
                tool_input = process_tool_args(args_schema, args, kwargs)

                # Invoke the MCPStore core method
                if adapter_self._instance_id is None:
                    result = adapter_self._context.call_tool(tool_name, tool_input)
                else:
                    result = adapter_self._context.call_tool(instance_id, tool_name, tool_input)
                view = to_tool_call_view(result)

                if view.is_error:
                    return adapter_self._format_error_output(
                        tool_name,
                        view.error_message or view.text or "Tool execution failed",
                        tool_input=tool_input,
                        view=view,
                    )

                if adapter_self._response_format == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = adapter_self._normalize_structured_value(view.structured)
                    data = adapter_self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                if view.text:
                    return view.text
                actual = view.structured if view.structured is not None else view.data
                actual = adapter_self._normalize_structured_value(actual)
                if isinstance(actual, (dict, list)):
                    return json.dumps(actual, ensure_ascii=False)
                return "" if actual is None else str(actual)

            except Exception as e:
                return adapter_self._format_error_output(
                    tool_name,
                    f"Tool '{tool_name}' execution failed: {str(e)}",
                    tool_input=tool_input,
                )

        return _tool_executor

    def _create_tool_coroutine(
        self,
        instance_id: str,
        tool_name: str,
        args_schema: Type[BaseModel],
    ):
        """
        Build a robust async executor that handles every argument passing style.
        """
        adapter_self = self  # captured by the closure

        async def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # Process arguments with the shared helper
                tool_input = process_tool_args(args_schema, args, kwargs)

                # Invoke the MCPStore core method (run the sync version via to_thread)
                if adapter_self._instance_id is None:
                    result = await asyncio.to_thread(
                        adapter_self._context.call_tool,
                        tool_name,
                        tool_input,
                    )
                else:
                    result = await asyncio.to_thread(
                        adapter_self._context.call_tool,
                        instance_id,
                        tool_name,
                        tool_input,
                    )
                view = to_tool_call_view(result)

                if view.is_error:
                    return adapter_self._format_error_output(
                        tool_name,
                        view.error_message or view.text or "Tool execution failed",
                        tool_input=tool_input,
                        view=view,
                    )

                if adapter_self._response_format == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = adapter_self._normalize_structured_value(view.structured)
                    data = adapter_self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                if view.text:
                    return view.text
                actual = view.structured if view.structured is not None else view.data
                actual = adapter_self._normalize_structured_value(actual)
                if isinstance(actual, (dict, list)):
                    return json.dumps(actual, ensure_ascii=False)
                return "" if actual is None else str(actual)

            except Exception as e:
                return adapter_self._format_error_output(
                    tool_name,
                    f"Tool '{tool_name}' execution failed: {str(e)}",
                    tool_input=tool_input,
                )

        return _tool_executor

    def list_tools(self) -> List[Tool]:
        """List all available MCPStore tools as a LangChain Tool list (sync version)."""
        if self._instance_id is None:
            return self._build_langchain_tools(self._context.list_tools())
        return self._build_langchain_tools(self._context.list_tools(self._instance_id))


    def _build_langchain_tools(self, mcp_tools_info: List[Any]) -> List[Tool]:
        langchain_tools = []
        for tool_info in mcp_tools_info:
            # Use the shared helpers
            enhanced_description = enhance_description(tool_info)
            args_schema = create_args_schema(tool_info)
            name = tool_name(tool_info)
            instance_id = tool_instance_id(tool_info)

            # Create the sync and async callables
            sync_func = self._create_tool_function(instance_id, name, args_schema)
            async_coroutine = self._create_tool_coroutine(instance_id, name, args_schema)

            # Create the LangChain StructuredTool
            lc_tool = StructuredTool(
                name=name,
                description=enhanced_description,
                func=sync_func,
                coroutine=async_coroutine,
                args_schema=args_schema,
            )

            langchain_tools.append(lc_tool)

        return langchain_tools


class SessionAwareLangChainAdapter(LangChainAdapter):
    """LangChain adapter that executes tools through a Rust-backed session."""

    def __init__(
        self,
        context: Any,
        session: Any,
        instance_id: str,
        response_format: str = "text",
    ):
        super().__init__(context, instance_id, response_format=response_format)
        self._session = session

    def _create_tool_function(
        self,
        instance_id: str,
        tool_name: str,
        args_schema: Type[BaseModel],
    ):
        adapter_self = self

        def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                tool_input = process_tool_args(args_schema, args, kwargs)
                result = adapter_self._session.call_tool(instance_id, tool_name, tool_input)
                view = to_tool_call_view(result)

                if view.is_error:
                    return adapter_self._format_error_output(
                        tool_name,
                        view.error_message or view.text or "Tool execution failed",
                        tool_input=tool_input,
                        view=view,
                    )

                if adapter_self._response_format == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = adapter_self._normalize_structured_value(view.structured)
                    data = adapter_self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                if view.text:
                    return view.text
                actual = view.structured if view.structured is not None else view.data
                actual = adapter_self._normalize_structured_value(actual)
                if isinstance(actual, (dict, list)):
                    return json.dumps(actual, ensure_ascii=False)
                return "" if actual is None else str(actual)
            except Exception as e:
                return adapter_self._format_error_output(
                    tool_name,
                    f"Tool '{tool_name}' execution failed: {str(e)}",
                    tool_input=tool_input,
                )

        return _tool_executor

    def _create_tool_coroutine(
        self,
        instance_id: str,
        tool_name: str,
        args_schema: Type[BaseModel],
    ):
        adapter_self = self

        async def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                tool_input = process_tool_args(args_schema, args, kwargs)
                result = await asyncio.to_thread(adapter_self._session.call_tool,
                    instance_id,
                    tool_name,
                    tool_input,
                )
                view = to_tool_call_view(result)

                if view.is_error:
                    return adapter_self._format_error_output(
                        tool_name,
                        view.error_message or view.text or "Tool execution failed",
                        tool_input=tool_input,
                        view=view,
                    )

                if adapter_self._response_format == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = adapter_self._normalize_structured_value(view.structured)
                    data = adapter_self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                if view.text:
                    return view.text
                actual = view.structured if view.structured is not None else view.data
                actual = adapter_self._normalize_structured_value(actual)
                if isinstance(actual, (dict, list)):
                    return json.dumps(actual, ensure_ascii=False)
                return "" if actual is None else str(actual)
            except Exception as e:
                return adapter_self._format_error_output(
                    tool_name,
                    f"Tool '{tool_name}' execution failed: {str(e)}",
                    tool_input=tool_input,
                )

        return _tool_executor

    def list_tools(self) -> List[Tool]:
        tools = [
            tool
            for tool in self._session.list_tools()
            if tool_instance_id(tool) == self._instance_id
        ]
        return self._build_langchain_tools(tools)
