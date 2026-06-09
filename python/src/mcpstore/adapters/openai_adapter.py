# src/mcpstore/adapters/openai_adapter.py
"""
OpenAI 适配器模块

将 MCPStore 工具转换为 OpenAI function calling 格式。
兼容 langchain-openai 的 bind_tools 方法和直接 OpenAI API 调用。
"""
from __future__ import annotations

import json
from typing import List, Dict, Any, Tuple

# 导入公共函数
from .common import (
    build_tool_error_payload,
    call_tool_response_helper,
    enhance_description,
    is_nullable,
    tool_input_schema,
    tool_name,
)

class OpenAIAdapter:
    """
    将 MCPStore 工具转换为 OpenAI function calling 格式的适配器。
    兼容 langchain-openai 的 bind_tools 方法和直接 OpenAI API。
    """

    def __init__(self, context: Any):
        self._context = context

    def list_tools(self) -> List[Dict[str, Any]]:
        """获取所有 MCPStore 工具并转换为 OpenAI function 格式（同步版本）。"""
        return [
            self._convert_to_openai_format(tool_info)
            for tool_info in self._context.list_tools()
        ]

    async def list_tools_async(self) -> List[Dict[str, Any]]:
        """获取所有 MCPStore 工具并转换为 OpenAI function 格式（异步版本）。"""
        mcp_tools_info = await self._context.list_tools_async()
        openai_tools = []

        for tool_info in mcp_tools_info:
            openai_tool = self._convert_to_openai_format(tool_info)
            openai_tools.append(openai_tool)

        return openai_tools

    def _convert_to_openai_format(self, tool_info: Any) -> Dict[str, Any]:
        """
        将 MCPStore 工具元数据转换为 OpenAI function calling 格式。

        OpenAI function 格式:
        {
            "type": "function",
            "function": {
                "name": "function_name",
                "description": "Function description",
                "parameters": {
                    "type": "object",
                    "properties": {...},
                    "required": [...]
                }
            }
        }
        """
        # 增强描述
        enhanced_description = enhance_description(tool_info)

        # 获取输入参数 schema
        input_schema = tool_input_schema(tool_info)
        properties = input_schema.get("properties", {})
        required = input_schema.get("required", [])

        # 转换参数 schema 为 OpenAI 格式
        openai_parameters = {
            "type": "object",
            "properties": {},
            "required": required
        }

        # 透传顶层 additionalProperties
        if "additionalProperties" in input_schema:
            openai_parameters["additionalProperties"] = input_schema["additionalProperties"]

        def _process_schema(p: Dict[str, Any]) -> Dict[str, Any]:
            """递归处理 JSON Schema 节点为 OpenAI 兼容格式。"""
            out: Dict[str, Any] = {}
            declared_type = p.get("type", "string")

            # 使用公共函数检查 nullability
            nullable = is_nullable(p)

            if nullable:
                base_type = (
                    declared_type
                    if isinstance(declared_type, str)
                    else next((t for t in declared_type if t != "null"), "string")
                )
                out["anyOf"] = [{"type": base_type}, {"type": "null"}]
            else:
                out["type"] = declared_type

            if "enum" in p:
                out["enum"] = p["enum"]
            if "default" in p:
                out["default"] = p["default"]

            # 数组处理
            if (
                declared_type == "array" or (isinstance(declared_type, list) and "array" in declared_type)
            ) and "items" in p:
                out["items"] = _process_schema(p["items"]) if isinstance(p["items"], dict) else p["items"]
                for k in ("minItems", "maxItems", "uniqueItems"):
                    if k in p:
                        out[k] = p[k]

            # 对象处理
            is_object_type = (
                declared_type == "object"
                or (isinstance(declared_type, list) and "object" in declared_type)
            )
            if is_object_type and "properties" in p:
                out["properties"] = {}
                for child_name, child_schema in p["properties"].items():
                    if isinstance(child_schema, dict):
                        out["properties"][child_name] = _process_schema(child_schema)
                    else:
                        out["properties"][child_name] = child_schema
                if "required" in p:
                    out["required"] = p["required"]
                if "additionalProperties" in p:
                    out["additionalProperties"] = p["additionalProperties"]

            return out

        # 处理每个参数
        for param_name, param_info in properties.items():
            openai_param: Dict[str, Any] = {"description": param_info.get("description", "")}
            # 合并处理后的 schema（type/anyOf, enum/default, 嵌套 items/properties）
            openai_param.update(_process_schema(param_info))
            openai_parameters["properties"][param_name] = openai_param

        # 如果没有参数，创建空参数结构
        if not properties:
            openai_parameters = {
                "type": "object",
                "properties": {},
                "required": []
            }

        # 构建 OpenAI function 格式
        openai_tool = {
            "type": "function",
            "function": {
                "name": tool_name(tool_info),
                "description": enhanced_description,
                "parameters": openai_parameters
            }
        }

        return openai_tool

    def execute_tool_call(self, tool_call: Dict[str, Any]) -> str:
        """
        执行来自 OpenAI 响应格式的工具调用。

        Args:
            tool_call: 包含 'name' 和 'arguments' 的 OpenAI 工具调用格式

        Returns:
            str: 工具执行结果
        """
        tool_name = None
        try:
            tool_name, arguments = self._parse_tool_call(tool_call)
            result = self._context.call_tool(tool_name, arguments)
            return self._format_tool_result(tool_name, arguments, result)
        except Exception as e:
            return f"Tool '{tool_name}' execution failed: {str(e)}"

    async def execute_tool_call_async(self, tool_call: Dict[str, Any]) -> str:
        """执行工具调用（异步版本）。"""
        tool_name = None
        try:
            tool_name, arguments = self._parse_tool_call(tool_call)
            result = await self._context.call_tool_async(tool_name, arguments)
            return self._format_tool_result(tool_name, arguments, result)
        except Exception as e:
            return f"Tool '{tool_name}' execution failed: {str(e)}"

    @staticmethod
    def _parse_tool_call(tool_call: Dict[str, Any]) -> Tuple[str, Dict[str, Any]]:
        name = tool_call.get("name") or tool_call.get("function", {}).get("name")
        arguments = tool_call.get("arguments") or tool_call.get("function", {}).get("arguments", {})

        if not name:
            raise ValueError("Tool name not found in tool_call")

        if isinstance(arguments, str):
            try:
                arguments = json.loads(arguments)
            except json.JSONDecodeError:
                raise ValueError("Tool arguments JSON parse failed")

        if not isinstance(arguments, dict):
            raise ValueError("Tool arguments must be a dict")

        return name, arguments

    @staticmethod
    def _format_tool_result(name: str, arguments: Dict[str, Any], result: Any) -> str:
        view = call_tool_response_helper(result)
        if view.is_error:
            payload = build_tool_error_payload(
                name,
                view.error_message or view.text or "Tool execution failed",
                tool_input=arguments,
                view=view,
            )
            return json.dumps(payload, ensure_ascii=False)
        actual_result = view.structured if view.structured is not None else view.data
        if actual_result is None:
            actual_result = view.text

        if isinstance(actual_result, (dict, list)):
            return json.dumps(actual_result, ensure_ascii=False)
        return str(actual_result)
