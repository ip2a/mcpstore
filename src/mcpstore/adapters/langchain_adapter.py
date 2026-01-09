# src/mcpstore/adapters/langchain_adapter.py

import json
import keyword
import logging
import re
import warnings
from dataclasses import asdict, is_dataclass
from typing import Type, List, TYPE_CHECKING

from langchain_core.tools import Tool, StructuredTool, ToolException
from pydantic import BaseModel, create_model, Field, ConfigDict

from .common import call_tool_response_helper
from ..core.bridge import get_async_bridge

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
        # Use the unified async bridge to avoid legacy helpers and loop conflicts
        self._bridge = get_async_bridge()
        # Adapter-only rendering preference for tool outputs
        self._response_format = response_format if response_format in ("text", "content_and_artifact") else "text"

    @staticmethod
    def _serialize_unknown(obj):
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
        """
        确保 structured/data 字段始终是 LangChain 能消费的基础类型。
        """
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
            line = f"- {param_name} ({param_type}): {param_desc}"
            # If array of objects, describe nested shape to help the LLM
            try:
                if (param_type == "array" or (isinstance(param_type, list) and "array" in param_type)) and isinstance(param_info.get("items"), dict):
                    items = param_info["items"]
                    if items.get("type") == "object" and "properties" in items:
                        nested = []
                        for nkey, nprop in items["properties"].items():
                            ntype = nprop.get("type", "string")
                            ndesc = nprop.get("description", "")
                            nested.append(f"    - {param_name}[].{nkey} ({ntype}) {ndesc}")
                        if nested:
                            line += "\n" + "\n".join(nested)
            except Exception:
                pass
            param_descriptions.append(line)

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
            """
            Convert any parameter name to a valid Python identifier.
            - Replace non-alphanumeric/underscore characters with underscores
            - Add prefix if starts with digit
            - Add suffix if Python keyword or reserved name
            """
            # 1. Replace all invalid characters with underscores
            safe = re.sub(r'[^a-zA-Z0-9_]', '_', original)

            # 2. If starts with digit, add prefix
            if safe and safe[0].isdigit():
                safe = f"param_{safe}"

            # 3. If Python keyword or reserved name, add suffix
            if keyword.iskeyword(safe) or safe in reserved_names or safe.startswith("_"):
                safe = f"{safe}_"

            # 4. Ensure not empty and is valid identifier
            if not safe or not safe.isidentifier():
                safe = "param_"

            return safe

        # Intelligently build field definitions with alias mapping
        fields = {}
        for original_name, prop in schema_properties.items():
            field_type = type_mapping.get(prop.get("type", "string"), str)

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
                except Exception:
                    pass
                return False

            is_nullable = _is_nullable(prop)
            is_required = original_name in required_fields

            # Make non-required fields truly optional by defaulting to None (sentinel)
            default_value = prop.get("default", ...)
            if not is_required and default_value == ...:
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

            # Preserve nested schema hints (arrays/objects) so model_json_schema() includes them
            try:
                declared_type = prop.get("type")
                is_array = declared_type == "array" or (isinstance(declared_type, list) and "array" in declared_type)
                is_object = declared_type == "object" or (isinstance(declared_type, list) and "object" in declared_type)
                json_extra: dict[str, Any] = {}
                if is_array and "items" in prop:
                    json_extra["items"] = prop["items"]
                    for k in ("minItems", "maxItems", "uniqueItems"):
                        if k in prop:
                            json_extra[k] = prop[k]
                if is_object and "properties" in prop:
                    json_extra["properties"] = prop["properties"]
                    if "required" in prop:
                        json_extra["required"] = prop["required"]
                    if "additionalProperties" in prop:
                        json_extra["additionalProperties"] = prop["additionalProperties"]
                if json_extra:
                    field_kwargs["json_schema_extra"] = json_extra
            except Exception:
                pass

            # Build field definition
            if default_value != ...:
                fields[safe_name] = (field_type, Field(default=default_value, **field_kwargs))
            else:
                fields[safe_name] = (field_type, Field(**field_kwargs))

        # [FIX] Allow empty model, don't force adding fields
        # For truly no-parameter tools, create empty BaseModel

        # Determine open schema (additionalProperties)
        additional_properties = tool_info.inputSchema.get("additionalProperties", False)
        allow_extra = bool(additional_properties)

        with warnings.catch_warnings():
            # Ignore pydantic warnings about field name conflicts (handled via sanitize_name)
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

                # [FIX] Handle no-parameter tools
                if not field_names:
                    # Truly no-parameter tools, ignore all input parameters
                    tool_input = {}
                else:
                    # Intelligent parameter processing for tools with parameters
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
                    validated_args.model_dump(
                        by_alias=True,          # Use original parameter names
                        exclude_unset=True,     # Don't send unset parameters
                        exclude_none=False,     # Preserve explicit None values
                        exclude_defaults=False  # Preserve default values (service may require them)
                    )
                )

                view = call_tool_response_helper(result)

                if view.is_error:
                    raise ToolException(view.error_message or view.text or "Tool execution failed")

                if getattr(self, "_response_format", "text") == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = self._normalize_structured_value(view.structured)
                    data = self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                return view.text
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
                # Get model field information
                schema_info = args_schema.model_json_schema()
                schema_fields = schema_info.get('properties', {})
                field_names = list(schema_fields.keys())

                # [FIX] Handle no-parameter tools (same logic as sync version)
                if not field_names:
                    # Truly no-parameter tools, ignore all input parameters
                    tool_input = {}
                else:
                    # Intelligent parameter processing
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

                    # Intelligently fill missing required parameters
                    for field_name, field_info in schema_fields.items():
                        if field_name not in tool_input and 'default' in field_info:
                            tool_input[field_name] = field_info['default']

                # Use Pydantic model to validate parameters
                try:
                    validated_args = args_schema(**tool_input)
                except Exception as validation_error:
                    filtered_input = {}
                    for field_name in field_names:
                        if field_name in tool_input:
                            filtered_input[field_name] = tool_input[field_name]
                    validated_args = args_schema(**filtered_input)

                # Call mcpstore core method (async version)
                result = await self._context.call_tool_async(
                    tool_name,
                    validated_args.model_dump(
                        by_alias=True,          # Use original parameter names
                        exclude_unset=True,     # Don't send unset parameters
                        exclude_none=False,     # Preserve explicit None values
                        exclude_defaults=False  # Preserve default values (service may require them)
                    )
                )

                view = call_tool_response_helper(result)

                if view.is_error:
                    raise ToolException(view.error_message or view.text or "Tool execution failed")

                if getattr(self, "_response_format", "text") == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = self._normalize_structured_value(view.structured)
                    data = self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                return view.text
            except Exception as e:
                error_msg = f"Tool '{tool_name}' execution failed: {str(e)}"
                if args or kwargs:
                    error_msg += f"\nParameter info: args={args}, kwargs={kwargs}"
                if tool_input:
                    error_msg += f"\nProcessed parameters: {tool_input}"
                return error_msg
        return _tool_executor

    def list_tools(self) -> List[Tool]:
        """Get all available mcpstore tools and convert them to LangChain Tool list (synchronous version)."""
        return self._bridge.run(self.list_tools_async(), op_name="LangChainAdapter.list_tools")

    async def list_tools_async(self) -> List[Tool]:
        """
        Get all available mcpstore tools and convert them to LangChain Tool list (asynchronous version).

        Raises:
            RuntimeError: If no tools available (all services failed to connect)
        """
        mcp_tools_info = await self._context.list_tools_async()

        # [CHECK] If tools are empty, provide friendly error message
        if not mcp_tools_info:
            logger.warning("[LIST_TOOLS] empty=True")
            # Check service status, provide more detailed hints
            services = await self._context.list_services_async()
            if not services:
                raise RuntimeError(
                    "No available tools: No MCP services have been added. "
                    "Please add services using add_service() first."
                )
            else:
                # Services exist but no tools, indicates services failed to connect
                failed_services = [s.name for s in services if s.status.value != 'healthy']
                if failed_services:
                    raise RuntimeError(
                        f"No available tools: The following services failed to connect: {', '.join(failed_services)}. "
                        f"Please check service configuration and dependencies, or use wait_service() to wait for services to be ready. "
                        f"\nTip: You can use list_services() to view detailed service status."
                    )
                else:
                    raise RuntimeError(
                        "No available tools: Services are connected but provide no tools. "
                        "Please check if services are working properly."
                    )

        langchain_tools = []
        for tool_info in mcp_tools_info:
            enhanced_description = self._enhance_description(tool_info)
            args_schema = self._create_args_schema(tool_info)

            # Create synchronous and asynchronous functions
            sync_func = self._create_tool_function(tool_info.name, args_schema)
            async_coroutine = await self._create_tool_coroutine(tool_info.name, args_schema)

            # [FIX] Determine parameter count based on original schema, not converted
            schema_properties = tool_info.inputSchema.get("properties", {})
            original_param_count = len(schema_properties)

            # Read per-tool overrides (e.g., return_direct) from context
            try:
                return_direct_flag = self._context._get_tool_override(tool_info.service_name, tool_info.name, "return_direct", False)
            except Exception:
                return_direct_flag = False

            # [CRITICAL FIX] For no-parameter tools, also use StructuredTool
            # Although they have no parameters, StructuredTool's parameter processing is more reliable
            # Tool type has special handling for empty dict {}, which may cause parameter conversion issues
            if original_param_count >= 1:
                # Multi-parameter tools use StructuredTool
                lc_tool = StructuredTool(
                    name=tool_info.name,
                    description=enhanced_description,
                    func=sync_func,
                    coroutine=async_coroutine,
                    args_schema=args_schema,
                )
            else:
                # [FIX] No-parameter tools also use StructuredTool to avoid parameter conversion issues
                # This ensures {} is correctly handled and not converted to []
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
                # [REUSE] Parent's intelligent parameter processing
                schema_info = args_schema.model_json_schema()
                schema_fields = schema_info.get('properties', {})
                field_names = list(schema_fields.keys())

                # [FIX] Handle no-parameter tools (same logic as parent class)
                if not field_names:
                    # Truly no-parameter tools, ignore all input parameters
                    tool_input = {}
                else:
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

                # [KEY] Use session-bound execution instead of context.call_tool
                logger.debug(f"[SESSION_LANGCHAIN] Executing tool '{tool_name}' via session '{self._session.session_id}'")
                result = self._session.use_tool(
                    tool_name,
                    validated_args.model_dump(
                        by_alias=True,          # Use original parameter names
                        exclude_unset=True,     # Don't send unset parameters
                        exclude_none=False,     # Preserve explicit None values
                        exclude_defaults=False  # Preserve default values (service may require them)
                    )
                )

                view = call_tool_response_helper(result)

                if view.is_error:
                    raise ToolException(view.error_message or view.text or "Tool execution failed")

                if getattr(self, "_response_format", "text") == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = self._normalize_structured_value(view.structured)
                    data = self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                return view.text

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
                # [SAME] Parameter processing as sync version
                schema_info = args_schema.model_json_schema()
                schema_fields = schema_info.get('properties', {})
                field_names = list(schema_fields.keys())

                # [FIX] Handle no-parameter tools (same logic as sync version)
                if not field_names:
                    # Truly no-parameter tools, ignore all input parameters
                    tool_input = {}
                else:
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

                # [KEY] Use session-bound async execution
                logger.debug(f"[SESSION_LANGCHAIN] Executing tool '{tool_name}' via session '{self._session.session_id}' (async)")
                result = await self._session.use_tool_async(
                    tool_name,
                    validated_args.model_dump(
                        by_alias=True,          # Use original parameter names
                        exclude_unset=True,     # Don't send unset parameters
                        exclude_none=False,     # Preserve explicit None values
                        exclude_defaults=False  # Preserve default values (service may require them)
                    )
                )

                view = call_tool_response_helper(result)

                if view.is_error:
                    raise ToolException(view.error_message or view.text or "Tool execution failed")

                if getattr(self, "_response_format", "text") == "content_and_artifact":
                    response = {"text": view.text, "artifacts": view.artifacts}
                    structured = self._normalize_structured_value(view.structured)
                    data = self._normalize_structured_value(view.data)
                    if structured is not None:
                        response["structured"] = structured
                    if data is not None:
                        response["data"] = data
                    return response

                return view.text

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

            # [CREATE] Session-bound functions
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
        return self._context._bridge.run(self.list_tools_async(), op_name="LangChainAdapter.list_tools_for_session")
