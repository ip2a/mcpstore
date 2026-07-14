# src/mcpstore/adapters/langchain_adapter.py
"""
LangChain 适配器模块

将 MCPStore 工具转换为 LangChain 工具格式，支持同步和异步执行。
"""

from __future__ import annotations

import json
import logging
from dataclasses import asdict, is_dataclass
from typing import Any, Type, List

from pydantic import BaseModel

# 导入公共函数
from .common import (
    build_tool_error_payload,
    call_tool_response_helper,
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
    MCPStore 与 LangChain 之间的适配器。
    将 mcpstore 的原生对象转换为 LangChain 可直接使用的对象。
    """

    def __init__(self, context: Any, instance_id: str, response_format: str = "text"):
        _require_langchain()
        self._context = context
        self._instance_id = instance_id
        # 工具输出格式偏好
        self._response_format = response_format if response_format in ("text", "content_and_artifact") else "text"

    @staticmethod
    def _serialize_unknown(obj):
        """序列化未知类型对象。"""
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
        """确保 structured/data 字段始终是 LangChain 能消费的基础类型。"""
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
        创建健壮的同步执行函数，智能处理各种参数传递方式。
        """
        adapter_self = self  # 闭包捕获

        def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # 使用公共函数处理参数
                tool_input = process_tool_args(args_schema, args, kwargs)

                # 调用 mcpstore 核心方法
                result = adapter_self._context.call_tool(instance_id, tool_name, tool_input)
                view = call_tool_response_helper(result)

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
        创建健壮的异步执行函数，智能处理各种参数传递方式。
        """
        adapter_self = self  # 闭包捕获

        async def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # 使用公共函数处理参数
                tool_input = process_tool_args(args_schema, args, kwargs)

                # 调用 mcpstore 核心方法（异步版本）
                result = await adapter_self._context.call_tool_async(
                    instance_id,
                    tool_name,
                    tool_input,
                )
                view = call_tool_response_helper(result)

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
        """获取所有可用的 mcpstore 工具并转换为 LangChain Tool 列表（同步版本）。"""
        return self._build_langchain_tools(self._context.list_tools(self._instance_id))

    async def list_tools_async(self) -> List[Tool]:
        """
        获取所有可用的 mcpstore 工具并转换为 LangChain Tool 列表（异步版本）。

        Raises:
            RuntimeError: 如果没有可用工具（所有服务连接失败）
        """
        mcp_tools_info = await self._context.list_tools_async(self._instance_id)

        # 检查工具是否为空，提供友好的错误信息
        if not mcp_tools_info:
            logger.warning("[LIST_TOOLS] empty=True")
            raise RuntimeError(
                f"Instance {self._instance_id!r} exposes no available tools"
            )

        return self._build_langchain_tools(mcp_tools_info)

    def _build_langchain_tools(self, mcp_tools_info: List[Any]) -> List[Tool]:
        langchain_tools = []
        for tool_info in mcp_tools_info:
            # 使用公共函数
            enhanced_description = enhance_description(tool_info)
            args_schema = create_args_schema(tool_info)
            name = tool_name(tool_info)
            instance_id = tool_instance_id(tool_info)

            # 创建同步和异步函数
            sync_func = self._create_tool_function(instance_id, name, args_schema)
            async_coroutine = self._create_tool_coroutine(instance_id, name, args_schema)

            # 创建 LangChain StructuredTool
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
                view = call_tool_response_helper(result)

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
                result = await adapter_self._session.call_tool_async(
                    instance_id,
                    tool_name,
                    tool_input,
                )
                view = call_tool_response_helper(result)

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

    async def list_tools_async(self) -> List[Tool]:
        tools = [
            tool
            for tool in await self._session.list_tools_async()
            if tool_instance_id(tool) == self._instance_id
        ]
        return self._build_langchain_tools(tools)
