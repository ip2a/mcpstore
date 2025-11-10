# src/mcpstore/adapters/langchain_adapter.py

import json
import logging
from typing import Type, List, TYPE_CHECKING

from langchain_core.tools import Tool, StructuredTool, ToolException
from pydantic import BaseModel, create_model, Field, ConfigDict
import warnings

from ..core.utils.async_sync_helper import get_global_helper

# Use TYPE_CHECKING and string hints to avoid circular imports
if TYPE_CHECKING:
    from ..core.context import MCPStoreContext
    from ..core.models.tool import ToolInfo

logger = logging.getLogger(__name__)

class LangChainAdapter:
    """
    Adapter (bridge) between MCPStore and LangChain.
    It converts mcpstore's native objects to objects that LangChain can directly use.
    """
    def __init__(self, context: 'MCPStoreContext', response_format: str = "text"):
        self._context = context
        self._sync_helper = get_global_helper()
        # Adapter-only rendering preference for tool outputs
        self._response_format = response_format if response_format in ("text", "content_and_artifact") else "text"

    def _build_artifacts(self, contents: list) -> list:
        """Convert non-text ContentBlocks into artifact dicts.
        Only used when response_format == "content_and_artifact".
        """
        artifacts = []
        try:
            for block in contents or []:
                if hasattr(block, 'text'):
                    continue
                # Generic mapping with best-effort fields
                item = {"type": getattr(block, "type", block.__class__.__name__.lower())}
                for attr in ("uri", "mime", "mime_type", "name", "filename", "size", "bytes", "width", "height"):
                    if hasattr(block, attr):
                        item[attr] = getattr(block, attr)
                artifacts.append(item)
        except Exception:
            pass
        return artifacts

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
        """(Data Conversion) Create Pydantic model from ToolInfo, avoiding BaseModel attribute name collisions (e.g., 'schema')."""
        schema_properties = tool_info.inputSchema.get("properties", {})
        required_fields = tool_info.inputSchema.get("required", [])

        type_mapping = {
            "string": str, "number": float, "integer": int,
            "boolean": bool, "array": list, "object": dict
        }

        # Reserved names that should not be used as field identifiers
        reserved_names = set(dir(BaseModel)) | {
            "schema", "model_json_schema", "model_dump", "dict", "json",
            "copy", "parse_obj", "parse_raw", "construct", "validate",
            "schema_json", "__fields__", "__root__", "Config", "model_config",
        }

        def sanitize_name(original: str) -> str:
            safe = original
            if not safe.isidentifier() or safe in reserved_names or safe.startswith("_"):
                safe = f"{safe}_"
            return safe

        # Intelligently build field definitions with alias mapping
        fields = {}
        for original_name, prop in schema_properties.items():
            field_type = type_mapping.get(prop.get("type", "string"), str)

            # Handle default values
            default_value = prop.get("default", ...)
            if original_name not in required_fields and default_value == ...:
                # Prefer omission; if needed, default to None (will be excluded on dump)
                default_value = None

            # Detect JSON Schema nullability/Optional
            def _is_nullable(p: dict) -> bool:
                try:
                    if p.get("nullable") is True:
                        return True
                    t = p.get("type")
                    if isinstance(t, list) and "null" in t:
                        return True
                    any_of = p.get("anyOf") or []
                    if isinstance(any_of, list) and any((isinstance(x, dict) and x.get("type") == "null") for x in any_of):
                        return True
                    one_of = p.get("oneOf") or []
                    if isinstance(one_of, list) and any((isinstance(x, dict) and x.get("type") == "null") for x in one_of):
                        return True
                    if default_value is None:
                        return True
                except Exception:
                    pass
                return False

            is_nullable = _is_nullable(prop)

            # If nullable and no explicit default, prefer None
            if is_nullable and default_value == ...:
                default_value = None

            safe_name = sanitize_name(original_name)
            field_kwargs = {"description": prop.get("description", "")}
            if safe_name != original_name:
                field_kwargs["validation_alias"] = original_name
                field_kwargs["serialization_alias"] = original_name

            # Apply Optional typing if nullable
            try:
                if is_nullable and field_type is not Any:
                    from typing import Optional as _Optional
                    field_type = _Optional[field_type]  # type: ignore
            except Exception:
                pass

            # Build field definition
            if default_value != ...:
                fields[safe_name] = (field_type, Field(default=default_value, **field_kwargs))
            else:
                fields[safe_name] = (field_type, Field(**field_kwargs))

        # Ensure at least one field to avoid empty model
        if not fields:
            fields["input"] = (str, Field(description="Tool input"))

        # Determine open schema (additionalProperties)
        additional_properties = tool_info.inputSchema.get("additionalProperties", False)
        allow_extra = bool(additional_properties)

        with warnings.catch_warnings():
            # 忽略 pydantic 关于字段名冲突的警告（已通过 sanitize_name 处理）
            warnings.filterwarnings(
                "ignore",
                category=UserWarning,
                module="pydantic",
            )
            base = BaseModel
            if allow_extra:
                base = type("OpenArgsBase", (BaseModel,), {"model_config": ConfigDict(extra="allow")})
            return create_model(
                f'{tool_info.name.capitalize().replace("_", "")}Input',
                __base__=base,
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
                    if field_name not in tool_input and 'default' in field_info:
                        tool_input[field_name] = field_info['default']

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
                result = self._context.call_tool(
                    tool_name,
                    validated_args.model_dump(exclude_none=True, exclude_unset=True, exclude_defaults=True)
                )

                # Bridge FastMCP -> LangChain: extract TextContent into string
                # If tool signals error, raise ToolException with textual message
                is_error = bool(getattr(result, 'is_error', False) or getattr(result, 'isError', False))
                contents = getattr(result, 'content', []) or []
                text_blocks = []
                try:
                    for block in contents:
                        if hasattr(block, 'text'):
                            text_blocks.append(block.text)
                except Exception:
                    pass

                if not text_blocks:
                    output = ""
                elif len(text_blocks) == 1:
                    output = text_blocks[0]
                else:
                    output = "\n".join(text_blocks)

                if is_error:
                    raise ToolException(output)

                # Optional artifacts output for LangChain-only adapter
                if getattr(self, "_response_format", "text") == "content_and_artifact":
                    artifacts = self._build_artifacts(contents)
                    return {"text": output, "artifacts": artifacts}

                return output
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
                    if field_name not in tool_input and 'default' in field_info:
                        tool_input[field_name] = field_info['default']

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
                result = await self._context.call_tool_async(
                    tool_name,
                    validated_args.model_dump(exclude_none=True, exclude_unset=True, exclude_defaults=True)
                )

                # Bridge FastMCP -> LangChain (async): extract TextContent into string
                is_error = bool(getattr(result, 'is_error', False) or getattr(result, 'isError', False))
                contents = getattr(result, 'content', []) or []
                text_blocks = []
                try:
                    for block in contents:
                        if hasattr(block, 'text'):
                            text_blocks.append(block.text)
                except Exception:
                    pass

                if not text_blocks:
                    output = ""
                elif len(text_blocks) == 1:
                    output = text_blocks[0]
                else:
                    output = "\n".join(text_blocks)

                if is_error:
                    raise ToolException(output)

                if getattr(self, "_response_format", "text") == "content_and_artifact":
                    artifacts = self._build_artifacts(contents)
                    return {"text": output, "artifacts": artifacts}

                return output
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
        """
        Get all available mcpstore tools and convert them to LangChain Tool list (asynchronous version).

        Raises:
            RuntimeError: 如果没有可用的工具（所有服务都未连接成功）
        """
        mcp_tools_info = await self._context.list_tools_async()

        # 🆕 检查工具是否为空，提供友好的错误提示
        if not mcp_tools_info:
            logger.warning("[LIST_TOOLS] empty=True")
            # 检查服务状态，给出更详细的提示
            services = await self._context.list_services_async()
            if not services:
                raise RuntimeError(
                    "无可用工具：没有已添加的MCP服务。"
                    "请先使用 add_service() 添加服务。"
                )
            else:
                # 有服务但没有工具，说明服务未成功连接
                failed_services = [s.name for s in services if s.status.value != 'healthy']
                if failed_services:
                    raise RuntimeError(
                        f"无可用工具：以下服务未成功连接: {', '.join(failed_services)}。"
                        f"请检查服务配置和依赖是否正确，或使用 wait_service() 等待服务就绪。"
                        f"\n提示：可以使用 list_services() 查看服务状态详情。"
                    )
                else:
                    raise RuntimeError(
                        "无可用工具：服务已连接但未提供工具。"
                        "请检查服务是否正常工作。"
                    )

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

            # Read per-tool overrides (e.g., return_direct) from context
            try:
                return_direct_flag = self._context._get_tool_override(tool_info.service_name, tool_info.name, "return_direct", False)
            except Exception:
                return_direct_flag = False

            if param_count >= 1:
                # Multi-parameter tools use StructuredTool
                lc_tool = StructuredTool(
                    name=tool_info.name,
                    description=enhanced_description,
                    func=sync_func,
                    coroutine=async_coroutine,
                    args_schema=args_schema,
                )
                # Set return_direct if supported
                try:
                    setattr(lc_tool, 'return_direct', bool(return_direct_flag))
                except Exception:
                    pass
                langchain_tools.append(lc_tool)
            else:
                # Single-parameter or no-parameter tools use regular Tool
                lc_tool = Tool(
                    name=tool_info.name,
                    description=enhanced_description,
                    func=sync_func,
                    coroutine=async_coroutine,
                    args_schema=args_schema,
                )
                try:
                    setattr(lc_tool, 'return_direct', bool(return_direct_flag))
                except Exception:
                    pass
                langchain_tools.append(lc_tool)
        return langchain_tools


class SessionAwareLangChainAdapter(LangChainAdapter):
    """
    Session-aware LangChain adapter

    This enhanced adapter creates LangChain tools that are bound to a specific session,
    ensuring state persistence across multiple tool calls in LangChain agent workflows.

    Key features:
    - Tools automatically use session-bound execution
    - State preservation across tool calls (e.g., browser stays open)
    - Seamless integration with existing LangChain workflows
    - Backward compatible with standard LangChainAdapter
    """

    def __init__(self, context: 'MCPStoreContext', session: 'Session', response_format: str = "text"):
        """
        Initialize session-aware adapter

        Args:
            context: MCPStoreContext instance (for tool discovery)
            session: Session object that tools will be bound to
            response_format: Same as LangChainAdapter ("text" or "content_and_artifact")
        """
        super().__init__(context, response_format=response_format)
        self._session = session

        logger.debug(f"Initialized session-aware adapter for session '{session.session_id}'")

    def _create_tool_function(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        Create session-bound tool function

        This overrides the parent method to route tool execution through the session,
        ensuring state persistence across multiple tool calls.
        """
        def _session_tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # 🎯 Reuse parent's intelligent parameter processing
                schema_info = args_schema.model_json_schema()
                schema_fields = schema_info.get('properties', {})
                field_names = list(schema_fields.keys())

                # Intelligent parameter processing (same as parent)
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

                # Intelligently fill missing required parameters (same as parent)
                for field_name, field_info in schema_fields.items():
                    if field_name not in tool_input and 'default' in field_info:
                        tool_input[field_name] = field_info['default']

                # Validate parameters (same as parent)
                try:
                    validated_args = args_schema(**tool_input)
                except Exception as validation_error:
                    filtered_input = {}
                    for field_name in field_names:
                        if field_name in tool_input:
                            filtered_input[field_name] = tool_input[field_name]
                    validated_args = args_schema(**filtered_input)

                # 🎯 KEY DIFFERENCE: Use session-bound execution instead of context.call_tool
                logger.debug(f"[SESSION_LANGCHAIN] Executing tool '{tool_name}' via session '{self._session.session_id}'")
                result = self._session.use_tool(
                    tool_name,
                    validated_args.model_dump(exclude_none=True, exclude_unset=True, exclude_defaults=True)
                )

                # Bridge FastMCP -> LangChain (session sync)
                is_error = bool(getattr(result, 'is_error', False) or getattr(result, 'isError', False))
                contents = getattr(result, 'content', []) or []
                text_blocks = []
                try:
                    for block in contents:
                        if hasattr(block, 'text'):
                            text_blocks.append(block.text)
                except Exception:
                    pass

                if not text_blocks:
                    output = ""
                elif len(text_blocks) == 1:
                    output = text_blocks[0]
                else:
                    output = "\n".join(text_blocks)

                if is_error:
                    raise ToolException(output)

                if getattr(self, "_response_format", "text") == "content_and_artifact":
                    artifacts = self._build_artifacts(contents)
                    return {"text": output, "artifacts": artifacts}

                return output

            except Exception as e:
                error_msg = f"Tool execution failed: {str(e)}"
                logger.error(f"[SESSION_LANGCHAIN] {error_msg}")
                return error_msg

        return _session_tool_executor

    def _create_async_tool_function(self, tool_name: str, args_schema: Type[BaseModel]):
        """
        Create session-bound async tool function
        """
        async def _session_async_tool_executor(*args, **kwargs):
            tool_input = {}
            try:
                # 🎯 Same parameter processing as sync version
                schema_info = args_schema.model_json_schema()
                schema_fields = schema_info.get('properties', {})
                field_names = list(schema_fields.keys())

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

                for field_name, field_info in schema_fields.items():
                    if field_name not in tool_input and 'default' in field_info:
                        tool_input[field_name] = field_info['default']

                try:
                    validated_args = args_schema(**tool_input)
                except Exception as validation_error:
                    filtered_input = {}
                    for field_name in field_names:
                        if field_name in tool_input:
                            filtered_input[field_name] = tool_input[field_name]
                    validated_args = args_schema(**filtered_input)

                # 🎯 KEY DIFFERENCE: Use session-bound async execution
                logger.debug(f"[SESSION_LANGCHAIN] Executing tool '{tool_name}' via session '{self._session.session_id}' (async)")
                result = await self._session.use_tool_async(
                    tool_name,
                    validated_args.model_dump(exclude_none=True, exclude_unset=True, exclude_defaults=True)
                )

                # Bridge FastMCP -> LangChain (session async)
                is_error = bool(getattr(result, 'is_error', False) or getattr(result, 'isError', False))
                contents = getattr(result, 'content', []) or []
                text_blocks = []
                try:
                    for block in contents:
                        if hasattr(block, 'text'):
                            text_blocks.append(block.text)
                except Exception:
                    pass

                if not text_blocks:
                    output = ""
                elif len(text_blocks) == 1:
                    output = text_blocks[0]
                else:
                    output = "\n".join(text_blocks)

                if is_error:
                    raise ToolException(output)

                if getattr(self, "_response_format", "text") == "content_and_artifact":
                    artifacts = self._build_artifacts(contents)
                    return {"text": output, "artifacts": artifacts}

                return output

            except Exception as e:
                error_msg = f"Async tool execution failed: {str(e)}"
                logger.error(f"[SESSION_LANGCHAIN] {error_msg}")
                return error_msg

        return _session_async_tool_executor

    async def list_tools_async(self) -> List[Tool]:
        """
        Create session-bound LangChain tools (async version)

        Returns:
            List of LangChain Tool objects bound to the session
        """
        logger.debug(f"Creating session-bound tools for session '{self._session.session_id}'")

        # Use parent's tool discovery logic
        mcpstore_tools = await self._context.list_tools_async()
        langchain_tools = []

        for tool_info in mcpstore_tools:
            # Create args schema (same as parent)
            args_schema = self._create_args_schema(tool_info)

            # Enhance description (same as parent)
            enhanced_description = self._enhance_description(tool_info)

            # 🎯 Create session-bound functions
            sync_func = self._create_tool_function(tool_info.name, args_schema)
            async_coroutine = self._create_async_tool_function(tool_info.name, args_schema)

            # Create LangChain tool with session binding
            langchain_tools.append(
                StructuredTool(
                    name=tool_info.name,
                    description=enhanced_description + f" [Session: {self._session.session_id}]",
                    func=sync_func,
                    coroutine=async_coroutine,
                    args_schema=args_schema,
                )
            )

        logger.debug(f"Created {len(langchain_tools)} session-bound tools")
        return langchain_tools

    def list_tools(self) -> List[Tool]:
        """
        Create session-bound LangChain tools (sync version)

        Returns:
            List of LangChain Tool objects bound to the session
        """
        return self._context._sync_helper.run_async(self.list_tools_async())
