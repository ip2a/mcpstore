# src/mcpstore/adapters/langchain_adapter.py

import json
from typing import Type, List, TYPE_CHECKING

from langchain_core.tools import Tool, StructuredTool
from pydantic import BaseModel, create_model, Field

from ..core.async_sync_helper import get_global_helper

# 使用 TYPE_CHECKING 和字符串提示来避免循环导入
if TYPE_CHECKING:
    from ..core.context import MCPStoreContext
    from ..core.models.tool import ToolInfo

class LangChainAdapter:
    """
    MCPStore 与 LangChain 之间的适配器（桥梁）。
    它将 mcpstore 的原生对象转换为 LangChain 可以直接使用的对象。
    """
    def __init__(self, context: 'MCPStoreContext'):
        self._context = context
        self._sync_helper = get_global_helper()

    def _enhance_description(self, tool_info: 'ToolInfo') -> str:
        """
        (前端防御) 增强工具描述，在 Prompt 中明确指导 LLM 使用正确的参数。
        """
        base_description = tool_info.description
        schema_properties = tool_info.inputSchema.get("properties", {})
        
        if not schema_properties:
            return base_description

        param_descriptions = []
        for param_name, param_info in schema_properties.items():
            param_type = param_info.get("type", "string")
            param_desc = param_info.get("description", "")
            param_descriptions.append(
                f"- {param_name} ({param_type}): {param_desc}"
            )
        
        # 将参数说明追加到主描述后
        enhanced_desc = base_description + "\n\n参数说明:\n" + "\n".join(param_descriptions)
        return enhanced_desc

    def _create_args_schema(self, tool_info: 'ToolInfo') -> Type[BaseModel]:
        """(数据转换) 根据 ToolInfo 的 inputSchema 动态创建 Pydantic 模型，智能处理各种参数情况。"""
        schema_properties = tool_info.inputSchema.get("properties", {})
        required_fields = tool_info.inputSchema.get("required", [])

        type_mapping = {
            "string": str, "number": float, "integer": int,
            "boolean": bool, "array": list, "object": dict
        }

        # 智能构建字段定义
        fields = {}
        for name, prop in schema_properties.items():
            field_type = type_mapping.get(prop.get("type", "string"), str)

            # 处理默认值
            default_value = prop.get("default", ...)
            if name not in required_fields and default_value == ...:
                # 为非必需字段提供合理的默认值
                if field_type == bool:
                    default_value = False
                elif field_type == str:
                    default_value = ""
                elif field_type in (int, float):
                    default_value = 0
                elif field_type == list:
                    default_value = []
                elif field_type == dict:
                    default_value = {}

            # 构建字段定义
            if default_value != ...:
                fields[name] = (field_type, Field(default=default_value, description=prop.get("description", "")))
            else:
                fields[name] = (field_type, ...)

        # 确保至少有一个字段，避免空模型
        if not fields:
            fields["input"] = (str, Field(description="Tool input"))

        return create_model(
            f'{tool_info.name.capitalize().replace("_", "")}Input',
            **fields
        )

    def _create_tool_function(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        (后端守卫) 创建一个健壮的同步执行函数，智能处理各种参数传递方式。
        """
        def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # 获取模型字段信息
                schema_info = args_schema.model_json_schema()
                schema_fields = schema_info.get('properties', {})
                field_names = list(schema_fields.keys())

                # 智能参数处理
                if kwargs:
                    # 关键字参数方式 (推荐)
                    tool_input = kwargs
                elif args:
                    if len(args) == 1:
                        # 单个参数处理
                        if isinstance(args[0], dict):
                            # 字典参数
                            tool_input = args[0]
                        else:
                            # 单值参数，映射到第一个字段
                            if field_names:
                                tool_input = {field_names[0]: args[0]}
                    else:
                        # 多个位置参数，按顺序映射到字段
                        for i, arg_value in enumerate(args):
                            if i < len(field_names):
                                tool_input[field_names[i]] = arg_value

                # 智能填充缺失的必需参数
                for field_name, field_info in schema_fields.items():
                    if field_name not in tool_input:
                        # 检查是否有默认值
                        if 'default' in field_info:
                            tool_input[field_name] = field_info['default']
                        # 为常见的可选参数提供智能默认值
                        elif field_name.lower() in ['retry', 'retry_on_error', 'retry_on_auth_error']:
                            tool_input[field_name] = True
                        elif field_name.lower() in ['timeout', 'max_retries']:
                            tool_input[field_name] = 30 if 'timeout' in field_name.lower() else 3

                # 使用 Pydantic 模型验证参数
                try:
                    validated_args = args_schema(**tool_input)
                except Exception as validation_error:
                    # 如果验证失败，尝试更宽松的处理
                    filtered_input = {}
                    for field_name in field_names:
                        if field_name in tool_input:
                            filtered_input[field_name] = tool_input[field_name]
                    validated_args = args_schema(**filtered_input)

                # 调用 mcpstore 的核心方法
                result = self._context.use_tool(tool_name, validated_args.model_dump())

                # 提取实际结果
                if hasattr(result, 'result') and result.result is not None:
                    actual_result = result.result
                elif hasattr(result, 'success') and result.success:
                    actual_result = getattr(result, 'data', str(result))
                else:
                    actual_result = str(result)

                if isinstance(actual_result, (dict, list)):
                    return json.dumps(actual_result, ensure_ascii=False)
                return str(actual_result)
            except Exception as e:
                # 提供更详细的错误信息用于调试
                error_msg = f"工具 '{tool_name}' 执行失败: {str(e)}"
                if args or kwargs:
                    error_msg += f"\n参数信息: args={args}, kwargs={kwargs}"
                if tool_input:
                    error_msg += f"\n处理后参数: {tool_input}"
                return error_msg
        return _tool_executor

    async def _create_tool_coroutine(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        (后端守卫) 创建一个健壮的异步执行函数，智能处理各种参数传递方式。
        """
        async def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # 获取模型字段信息
                schema_info = args_schema.model_json_schema()
                schema_fields = schema_info.get('properties', {})
                field_names = list(schema_fields.keys())

                # 智能参数处理（与同步版本相同的逻辑）
                if kwargs:
                    tool_input = kwargs
                elif args:
                    if len(args) == 1:
                        if isinstance(args[0], dict):
                            tool_input = args[0]
                        else:
                            if field_names:
                                tool_input = {field_names[0]: args[0]}
                    else:
                        for i, arg_value in enumerate(args):
                            if i < len(field_names):
                                tool_input[field_names[i]] = arg_value

                # 智能填充缺失的必需参数
                for field_name, field_info in schema_fields.items():
                    if field_name not in tool_input:
                        if 'default' in field_info:
                            tool_input[field_name] = field_info['default']
                        elif field_name.lower() in ['retry', 'retry_on_error', 'retry_on_auth_error']:
                            tool_input[field_name] = True
                        elif field_name.lower() in ['timeout', 'max_retries']:
                            tool_input[field_name] = 30 if 'timeout' in field_name.lower() else 3

                # 使用 Pydantic 模型验证参数
                try:
                    validated_args = args_schema(**tool_input)
                except Exception as validation_error:
                    filtered_input = {}
                    for field_name in field_names:
                        if field_name in tool_input:
                            filtered_input[field_name] = tool_input[field_name]
                    validated_args = args_schema(**filtered_input)

                # 调用 mcpstore 的核心方法（异步版本）
                result = await self._context.use_tool_async(tool_name, validated_args.model_dump())

                # 提取实际结果
                if hasattr(result, 'result') and result.result is not None:
                    actual_result = result.result
                elif hasattr(result, 'success') and result.success:
                    actual_result = getattr(result, 'data', str(result))
                else:
                    actual_result = str(result)

                if isinstance(actual_result, (dict, list)):
                    return json.dumps(actual_result, ensure_ascii=False)
                return str(actual_result)
            except Exception as e:
                error_msg = f"工具 '{tool_name}' 执行失败: {str(e)}"
                if args or kwargs:
                    error_msg += f"\n参数信息: args={args}, kwargs={kwargs}"
                if tool_input:
                    error_msg += f"\n处理后参数: {tool_input}"
                return error_msg
        return _tool_executor

    def list_tools(self) -> List[Tool]:
        """获取所有可用的 mcpstore 工具，并将其转换为 LangChain Tool 列表（同步版本）。"""
        return self._sync_helper.run_async(self.list_tools_async())

    async def list_tools_async(self) -> List[Tool]:
        """获取所有可用的 mcpstore 工具，并将其转换为 LangChain Tool 列表（异步版本）。"""
        mcp_tools_info = await self._context.list_tools_async()
        langchain_tools = []
        for tool_info in mcp_tools_info:
            enhanced_description = self._enhance_description(tool_info)
            args_schema = self._create_args_schema(tool_info)

            # 创建同步和异步函数
            sync_func = self._create_tool_function(tool_info.name, args_schema)
            async_coroutine = await self._create_tool_coroutine(tool_info.name, args_schema)

            # 智能选择Tool类型
            schema_properties = tool_info.inputSchema.get("properties", {})
            param_count = len(schema_properties)

            if param_count > 1:
                # 多参数工具使用StructuredTool
                langchain_tools.append(
                    StructuredTool(
                        name=tool_info.name,
                        description=enhanced_description,
                        func=sync_func,
                        coroutine=async_coroutine,
                        args_schema=args_schema,
                    )
                )
            else:
                # 单参数或无参数工具使用普通Tool
                langchain_tools.append(
                    Tool(
                        name=tool_info.name,
                        description=enhanced_description,
                        func=sync_func,
                        coroutine=async_coroutine,
                        args_schema=args_schema,
                    )
                )
        return langchain_tools
