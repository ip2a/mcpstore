# src/mcpstore/adapters/langchain_adapter.py

import json
from typing import Type, List, TYPE_CHECKING

from langchain_core.tools import Tool, StructuredTool
from pydantic import BaseModel, create_model, Field

from ..core.utils.async_sync_helper import get_global_helper

# Use TYPE_CHECKING and string hints to avoid circular imports
if TYPE_CHECKING:
    from ..core.context import MCPStoreContext
    from ..core.models.tool import ToolInfo

class LangChainAdapter:
    """
    Adapter (bridge) between MCPStore and LangChain.
    It converts mcpstore's native objects to objects that LangChain can directly use.
    """
    def __init__(self, context: 'MCPStoreContext'):
        self._context = context
        self._sync_helper = get_global_helper()

    def _enhance_description(self, tool_info: 'ToolInfo') -> str:
        """
        (Frontend Defense) Enhance tool description, clearly guide LLM to use correct parameters in Prompt.
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
        
        # Append parameter descriptions to main description
        enhanced_desc = base_description + "\n\nParameter descriptions:\n" + "\n".join(param_descriptions)
        return enhanced_desc

    def _create_args_schema(self, tool_info: 'ToolInfo') -> Type[BaseModel]:
        """(Data Conversion) Dynamically create Pydantic model based on ToolInfo's inputSchema, intelligently handle various parameter cases."""
        schema_properties = tool_info.inputSchema.get("properties", {})
        required_fields = tool_info.inputSchema.get("required", [])

        type_mapping = {
            "string": str, "number": float, "integer": int,
            "boolean": bool, "array": list, "object": dict
        }

        # Intelligently build field definitions
        fields = {}
        for name, prop in schema_properties.items():
            field_type = type_mapping.get(prop.get("type", "string"), str)

            # Handle default values
            default_value = prop.get("default", ...)
            if name not in required_fields and default_value == ...:
                # Provide reasonable default values for non-required fields
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

            # Build field definition
            if default_value != ...:
                fields[name] = (field_type, Field(default=default_value, description=prop.get("description", "")))
            else:
                fields[name] = (field_type, ...)

        # Ensure at least one field to avoid empty model
        if not fields:
            fields["input"] = (str, Field(description="Tool input"))

        return create_model(
            f'{tool_info.name.capitalize().replace("_", "")}Input',
            **fields
        )

    def _create_tool_function(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        (Backend Guard) Create a robust synchronous execution function, intelligently handle various parameter passing methods.
        """
        def _tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # Get model field information
                schema_info = args_schema.model_json_schema()
                schema_fields = schema_info.get('properties', {})
                field_names = list(schema_fields.keys())

                # Intelligent parameter processing
                if kwargs:
                    # Keyword argument method (recommended)
                    tool_input = kwargs
                elif args:
                    if len(args) == 1:
                        # Single parameter processing
                        if isinstance(args[0], dict):
                            # Dictionary parameter
                            tool_input = args[0]
                        else:
                            # Single value parameter, map to first field
                            if field_names:
                                tool_input = {field_names[0]: args[0]}
                    else:
                        # Multiple positional parameters, map to fields in order
                        for i, arg_value in enumerate(args):
                            if i < len(field_names):
                                tool_input[field_names[i]] = arg_value

                # Intelligently fill missing required parameters
                for field_name, field_info in schema_fields.items():
                    if field_name not in tool_input:
                        # Check if there's a default value
                        if 'default' in field_info:
                            tool_input[field_name] = field_info['default']
                        # Provide intelligent default values for common optional parameters
                        elif field_name.lower() in ['retry', 'retry_on_error', 'retry_on_auth_error']:
                            tool_input[field_name] = True
                        elif field_name.lower() in ['timeout', 'max_retries']:
                            tool_input[field_name] = 30 if 'timeout' in field_name.lower() else 3

                # Use Pydantic model to validate parameters
                try:
                    validated_args = args_schema(**tool_input)
                except Exception as validation_error:
                    # If validation fails, try more lenient processing
                    filtered_input = {}
                    for field_name in field_names:
                        if field_name in tool_input:
                            filtered_input[field_name] = tool_input[field_name]
                    validated_args = args_schema(**filtered_input)

                # Call mcpstore's core method
                result = self._context.call_tool(tool_name, validated_args.model_dump())

                # Extract actual result
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
                # Provide more detailed error information for debugging
                error_msg = f"Tool '{tool_name}' execution failed: {str(e)}"
                if args or kwargs:
                    error_msg += f"\nParameter info: args={args}, kwargs={kwargs}"
                if tool_input:
                    error_msg += f"\nProcessed parameters: {tool_input}"
                return error_msg
        return _tool_executor

    async def _create_tool_coroutine(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        (Backend Guard) Create a robust asynchronous execution function, intelligently handle various parameter passing methods.
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
                result = await self._context.call_tool_async(tool_name, validated_args.model_dump())

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
        """Get all available mcpstore tools and convert them to LangChain Tool list (synchronous version)."""
        return self._sync_helper.run_async(self.list_tools_async())

    async def list_tools_async(self) -> List[Tool]:
        """Get all available mcpstore tools and convert them to LangChain Tool list (asynchronous version)."""
        mcp_tools_info = await self._context.list_tools_async()
        langchain_tools = []
        for tool_info in mcp_tools_info:
            enhanced_description = self._enhance_description(tool_info)
            args_schema = self._create_args_schema(tool_info)

            # Create synchronous and asynchronous functions
            sync_func = self._create_tool_function(tool_info.name, args_schema)
            async_coroutine = await self._create_tool_coroutine(tool_info.name, args_schema)

            # Intelligently select Tool type
            schema_properties = tool_info.inputSchema.get("properties", {})
            param_count = len(schema_properties)

            if param_count > 1:
                # Multi-parameter tools use StructuredTool
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
                # Single-parameter or no-parameter tools use regular Tool
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
