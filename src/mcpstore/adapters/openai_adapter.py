# src/mcpstore/adapters/openai_adapter.py
"""
OpenAI 适配器模块

将 MCPStore 工具转换为 OpenAI function calling 格式。
兼容 langchain-openai 的 bind_tools 方法和直接 OpenAI API 调用。
"""
from __future__ import annotations

import json
from typing import List, Dict, Any, TYPE_CHECKING

# 导入公共函数
from .common import (
    enhance_description,
    create_args_schema,
    build_sync_executor,
    build_async_executor,
    is_nullable,
)

if TYPE_CHECKING:
    from ..core.context.base_context import MCPStoreContext
    from ..core.models.tool import ToolInfo


class OpenAIAdapter:
    """
    将 MCPStore 工具转换为 OpenAI function calling 格式的适配器。
    兼容 langchain-openai 的 bind_tools 方法和直接 OpenAI API。
    """

    def __init__(self, context: 'MCPStoreContext'):
        self._context = context

    def list_tools(self) -> List[Dict[str, Any]]:
        """获取所有 MCPStore 工具并转换为 OpenAI function 格式（同步版本）。"""
        return self._context._run_async_via_bridge(
            self.list_tools_async(), op_name="openai_adapter.list_tools"
        )

    async def list_tools_async(self) -> List[Dict[str, Any]]:
        """获取所有 MCPStore 工具并转换为 OpenAI function 格式（异步版本）。"""
        mcp_tools_info = await self._context.list_tools_async()
        openai_tools = []

        for tool_info in mcp_tools_info:
            openai_tool = self._convert_to_openai_format(tool_info)
            openai_tools.append(openai_tool)

        return openai_tools

    def _convert_to_openai_format(self, tool_info: 'ToolInfo') -> Dict[str, Any]:
        """
        将 MCPStore ToolInfo 转换为 OpenAI function calling 格式。

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
        input_schema = tool_info.inputSchema or {}
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
                "name": tool_info.name,
                "description": enhanced_description,
                "parameters": openai_parameters
            }
        }

        return openai_tool

    def get_callable_tools(self) -> List[Dict[str, Any]]:
        """
        获取带可调用函数的工具。

        Returns:
            包含 'tool'（OpenAI 格式）和 'callable'（执行函数）的字典列表
        """
        return self._context._run_async_via_bridge(
            self.get_callable_tools_async(), op_name="openai_adapter.get_callable_tools"
        )

    async def get_callable_tools_async(self) -> List[Dict[str, Any]]:
        """获取带可调用函数的工具（异步版本）。"""
        mcp_tools_info = await self._context.list_tools_async()
        callable_tools = []

        for tool_info in mcp_tools_info:
            # 转换为 OpenAI 格式
            openai_tool = self._convert_to_openai_format(tool_info)

            # 使用公共函数创建参数 schema
            args_schema = create_args_schema(tool_info)

            # 使用公共函数创建可调用函数
            sync_executor = build_sync_executor(self._context, tool_info.name, args_schema)
            async_executor = build_async_executor(self._context, tool_info.name, args_schema)

            callable_tools.append({
                "tool": openai_tool,
                "callable": sync_executor,
                "async_callable": async_executor,
                "name": tool_info.name,
                "schema": args_schema
            })

        return callable_tools

    def create_tool_registry(self) -> Dict[str, Any]:
        """
        创建工具注册表，便于按名称执行工具。

        Returns:
            工具名到执行器和元数据的映射字典
        """
        return self._context._run_async_via_bridge(
            self.create_tool_registry_async(), op_name="openai_adapter.create_tool_registry"
        )

    async def create_tool_registry_async(self) -> Dict[str, Any]:
        """创建工具注册表（异步版本）。"""
        callable_tools = await self.get_callable_tools_async()
        registry = {}

        for tool_data in callable_tools:
            tool_name = tool_data["name"]
            registry[tool_name] = {
                "openai_format": tool_data["tool"],
                "execute": tool_data["callable"],
                "execute_async": tool_data["async_callable"],
                "schema": tool_data["schema"]
            }

        return registry

    def execute_tool_call(self, tool_call: Dict[str, Any]) -> str:
        """
        执行来自 OpenAI 响应格式的工具调用。

        Args:
            tool_call: 包含 'name' 和 'arguments' 的 OpenAI 工具调用格式

        Returns:
            str: 工具执行结果
        """
        return self._context._run_async_via_bridge(
            self.execute_tool_call_async(tool_call), op_name="openai_adapter.execute_tool_call"
        )

    async def execute_tool_call_async(self, tool_call: Dict[str, Any]) -> str:
        """执行工具调用（异步版本）。"""
        tool_name = None
        try:
            tool_name = tool_call.get("name") or tool_call.get("function", {}).get("name")
            arguments = tool_call.get("arguments") or tool_call.get("function", {}).get("arguments", {})

            if not tool_name:
                raise ValueError("Tool name not found in tool_call")

            # 如果 arguments 是字符串，尝试解析为 JSON
            if isinstance(arguments, str):
                try:
                    arguments = json.loads(arguments)
                except json.JSONDecodeError:
                    raise ValueError("Tool arguments JSON parse failed")

            # 调用工具
            result = await self._context.call_tool_async(tool_name, arguments)

            # 提取实际结果
            if hasattr(result, 'result') and result.result is not None:
                actual_result = result.result
            elif hasattr(result, 'success') and result.success:
                actual_result = getattr(result, 'data', str(result))
            else:
                actual_result = str(result)

            # 格式化输出
            if isinstance(actual_result, (dict, list)):
                return json.dumps(actual_result, ensure_ascii=False)
            return str(actual_result)

        except Exception as e:
            error_msg = f"Tool '{tool_name}' execution failed: {str(e)}"
            return error_msg

    def batch_execute_tool_calls(self, tool_calls: List[Dict[str, Any]]) -> List[str]:
        """
        批量执行多个工具调用。

        Args:
            tool_calls: OpenAI 工具调用格式列表

        Returns:
            List[str]: 工具执行结果列表
        """
        return self._context._run_async_via_bridge(
            self.batch_execute_tool_calls_async(tool_calls), op_name="openai_adapter.batch_execute_tool_calls"
        )

    async def batch_execute_tool_calls_async(self, tool_calls: List[Dict[str, Any]]) -> List[str]:
        """批量执行工具调用（异步版本）。"""
        results = []
        for tool_call in tool_calls:
            try:
                result = await self.execute_tool_call_async(tool_call)
                results.append(result)
            except Exception as e:
                results.append(f"Error executing tool call: {str(e)}")
        return results
