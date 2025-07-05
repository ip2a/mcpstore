# src/mcpstore/adapters/langchain_adapter.py

import json
from typing import Type, List, TYPE_CHECKING
from langchain_core.tools import Tool
from pydantic import BaseModel, create_model
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
        """(数据转换) 根据 ToolInfo 的 inputSchema 动态创建 Pydantic 模型。"""
        schema_properties = tool_info.inputSchema.get("properties", {})
        type_mapping = {
            "string": str, "number": float, "integer": int, 
            "boolean": bool, "array": list, "object": dict
        }
        
        fields = {
            name: (type_mapping.get(prop.get("type", "string"), str), ...)
            for name, prop in schema_properties.items()
        }
        
        return create_model(
            f'{tool_info.name.capitalize().replace("_", "")}Input',
            **fields
        )

    def _create_tool_function(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        (后端守卫) 创建一个健壮的同步执行函数，以应对 LangChain 不同的调用方式。
        """
        def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # 优先处理关键字参数 (e.g., func(query='北京'))
                if kwargs:
                    tool_input = kwargs
                # 其次处理位置参数
                elif args:
                    # 如果第一个位置参数是字典，直接使用 (e.g., func({'query':'北京'}))
                    if isinstance(args[0], dict):
                        tool_input = args[0]
                    # 如果是单个值，智能地映射到 schema 的第一个字段 (e.g., func('北京'))
                    else:
                        schema_fields = args_schema.model_json_schema()['properties']
                        first_field_name = next(iter(schema_fields))
                        tool_input = {first_field_name: args[0]}

                # 使用 Pydantic 模型严格验证参数，如果名称或类型不匹配会在此处报错
                validated_args = args_schema(**tool_input)
                # 调用 mcpstore 的核心方法（使用同步版本）
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
                return f"执行工具 '{tool_name}' 时出错: {e}。收到的参数为: args={args}, kwargs={kwargs}"
        return _tool_executor

    async def _create_tool_coroutine(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        (后端守卫) 创建一个健壮的异步执行函数，以应对 LangChain 不同的调用方式。
        """
        async def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # 优先处理关键字参数 (e.g., func(query='北京'))
                if kwargs:
                    tool_input = kwargs
                # 其次处理位置参数
                elif args:
                    # 如果第一个位置参数是字典，直接使用 (e.g., func({'query':'北京'}))
                    if isinstance(args[0], dict):
                        tool_input = args[0]
                    # 如果是单个值，智能地映射到 schema 的第一个字段 (e.g., func('北京'))
                    else:
                        schema_fields = args_schema.model_json_schema()['properties']
                        first_field_name = next(iter(schema_fields))
                        tool_input = {first_field_name: args[0]}
                
                # 使用 Pydantic 模型严格验证参数，如果名称或类型不匹配会在此处报错
                validated_args = args_schema(**tool_input)
                # 调用 mcpstore 的核心方法（使用异步版本，因为这个函数本身就是异步的）
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
                return f"执行工具 '{tool_name}' 时出错: {e}。收到的参数为: args={args}, kwargs={kwargs}"
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

            langchain_tools.append(
                Tool(
                    name=tool_info.name,
                    description=enhanced_description,
                    func=sync_func,  # 提供同步函数
                    coroutine=async_coroutine,  # 提供异步函数
                    args_schema=args_schema,
                )
            )
        return langchain_tools
