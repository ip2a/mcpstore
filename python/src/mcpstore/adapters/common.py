# src/mcpstore/adapters/common.py
"""
Adapter shared utility module.

Provides tool functions shared by all adapters to avoid code duplication:
- is_nullable: check whether a JSON Schema property is nullable
- process_tool_args: normalize tool argument conversion
- create_args_schema: build a Pydantic arguments model
- to_tool_call_view: normalize tool call results
"""
from __future__ import annotations

import inspect
import asyncio
import json
import keyword
import warnings
from typing import Callable, Any, Type, List, Dict, Optional

from pydantic import BaseModel, create_model, Field, ConfigDict

__all__ = [
    # Shared utility functions
    'is_nullable',
    'process_tool_args',
    'enhance_description',
    'create_args_schema',
    'to_tool_call_view',
    'build_tool_error_payload',
    'service_name',
    'service_status_value',
    'tool_name',
    'tool_instance_id',
    'tool_service_name',
    'tool_input_schema',
    # Executor builders
    'build_sync_executor',
    'build_async_executor',
    'attach_signature_from_schema',
    # Data classes
    'ToolCallView',
]


# ============================================================================
# Data classes
# ============================================================================

class ToolCallView(BaseModel):
    """Standardized view over an MCPStore CallToolResult."""

    text: str = ""
    content: List[Any] = Field(default_factory=list)
    artifacts: List[Dict[str, Any]] = Field(default_factory=list)
    structured: Any = None
    data: Any = None
    is_error: bool = False
    error_message: Optional[str] = None


def _read_field(data: Any, *names: str, default: Any = None) -> Any:
    """Read a field through both object attributes and dict keys."""
    if isinstance(data, dict):
        for name in names:
            if name in data:
                return data[name]
        return default

    for name in names:
        if hasattr(data, name):
            return getattr(data, name)

    info = getattr(data, "info", None)
    if callable(info):
        try:
            return _read_field(info(), *names, default=default)
        except Exception:
            pass

    return default


def service_name(service_info: Any) -> str:
    """Read the service name from mapping or object-shaped SDK records."""
    value = _read_field(service_info, "service_name", default="")
    return value if isinstance(value, str) else str(value or "")


def service_status_value(service_info: Any) -> str:
    """Read the service status value, normalized to a string."""
    status = _read_field(service_info, "status", default="")
    if isinstance(status, dict):
        value = status.get("value") or status.get("status") or status.get("name")
        return value if isinstance(value, str) else str(value or "")
    value = getattr(status, "value", None)
    if isinstance(value, str):
        return value
    return status if isinstance(status, str) else str(status or "")


def tool_name(tool_info: Any) -> str:
    """Read the tool name."""
    value = _read_field(tool_info, "name", default="")
    return value if isinstance(value, str) else str(value or "")


def tool_service_name(tool_info: Any) -> str:
    """Read the name of the service that owns the tool."""
    value = _read_field(tool_info, "service_name", default="")
    return value if isinstance(value, str) else str(value or "")


def tool_instance_id(tool_info: Any) -> str:
    """Read the required owning instance ID from a tool record."""
    value = _read_field(tool_info, "instance_id", default="")
    instance_id = value if isinstance(value, str) else str(value or "")
    if not instance_id:
        raise ValueError("Tool record is missing instance_id")
    return instance_id


def tool_input_schema(tool_info: Any) -> Dict[str, Any]:
    """Read the tool input schema."""
    schema = _read_field(tool_info, "inputSchema", "input_schema", default={}) or {}
    return schema if isinstance(schema, dict) else {}


# ============================================================================
# JSON Schema helpers
# ============================================================================

def is_nullable(prop: Dict[str, Any]) -> bool:
    """
    Check whether a JSON Schema property is nullable.

    Supports the following nullability representations:
    - nullable: true
    - type: ["string", "null"]
    - anyOf: [{"type": "string"}, {"type": "null"}]
    - oneOf: [{"type": "string"}, {"type": "null"}]
    - default: null

    Args:
        prop: The JSON Schema property definition.

    Returns:
        bool: Whether the property is nullable.
    """
    try:
        # Explicit nullable flag
        if prop.get("nullable") is True:
            return True

        # type array contains "null"
        t = prop.get("type")
        if isinstance(t, list) and "null" in t:
            return True

        # anyOf contains a null type
        any_of = prop.get("anyOf") or []
        if isinstance(any_of, list) and any(
            (isinstance(x, dict) and x.get("type") == "null") for x in any_of
        ):
            return True

        # oneOf contains a null type
        one_of = prop.get("oneOf") or []
        if isinstance(one_of, list) and any(
            (isinstance(x, dict) and x.get("type") == "null") for x in one_of
        ):
            return True

        # default value is null
        if prop.get("default", object()) is None:
            return True

    except Exception:
        pass

    return False


# ============================================================================
# Argument processing
# ============================================================================

def process_tool_args(
    args_schema: Type[BaseModel],
    args: tuple,
    kwargs: dict
) -> Dict[str, Any]:
    """
    Normalize tool argument conversion.

    Converts every supported calling convention (positional arguments,
    keyword arguments, or a single dict) into the standard tool input dict.
    Supports no-argument tools and open-schema tools.

    Args:
        args_schema: The Pydantic arguments model.
        args: Tuple of positional arguments.
        kwargs: Dict of keyword arguments.

    Returns:
        Dict[str, Any]: The normalized tool input dict.
    """
    tool_input: Dict[str, Any] = {}

    try:
        # Read model field information
        schema_info = args_schema.model_json_schema()
        schema_fields = schema_info.get('properties', {})
        field_names = list(schema_fields.keys())
        allow_extra = bool(schema_info.get("additionalProperties", False))

        # Handle no-argument tools / open-schema tools
        if not field_names:
            if allow_extra:
                if kwargs:
                    tool_input = dict(kwargs)
                elif args and len(args) == 1 and isinstance(args[0], dict):
                    tool_input = dict(args[0])
                else:
                    tool_input = {}
            else:
                tool_input = {}
        else:
            # Argument handling when fields are declared
            if kwargs:
                tool_input = dict(kwargs)
            elif args:
                if len(args) == 1:
                    if isinstance(args[0], dict):
                        tool_input = dict(args[0])
                    else:
                        # A single positional argument maps to the first field
                        tool_input = {field_names[0]: args[0]}
                else:
                    # Multiple positional arguments map to fields in order
                    for i, arg_value in enumerate(args):
                        if i < len(field_names):
                            tool_input[field_names[i]] = arg_value

    except Exception:
        # Keep direct kwargs usable when adapter schema coercion fails.
        tool_input = dict(kwargs) if kwargs else {}

    return tool_input


# ============================================================================
# Result processing
# ============================================================================

def _extract_text_blocks(contents: list) -> List[str]:
    """Extract text from content blocks."""
    blocks: List[str] = []
    for block in contents or []:
        if isinstance(block, dict):
            block_type = block.get("type")
            if block_type is not None and block_type != "text":
                continue
            text = block.get("text")
        else:
            block_type = getattr(block, "type", None)
            if block_type is not None and block_type != "text":
                continue
            text = getattr(block, "text", None)
        if isinstance(text, str):
            blocks.append(text)
    return blocks


def _extract_artifacts(contents: list) -> List[Dict[str, Any]]:
    """Extract artifacts (non-text content) from content blocks."""
    artifacts: List[Dict[str, Any]] = []
    for block in contents or []:
        if isinstance(block, dict):
            block_type = block.get("type")
            if block_type == "text" or (block_type is None and "text" in block):
                continue
            artifact = dict(block)
            artifact.setdefault("type", artifact.get("type", "artifact"))
            artifacts.append(artifact)
            continue

        block_type = getattr(block, "type", None)
        if block_type == "text" or (block_type is None and hasattr(block, "text")):
            continue
        artifact = {"type": getattr(block, "type", block.__class__.__name__.lower())}
        for attr in (
            "uri",
            "mime",
            "mime_type",
            "mimeType",
            "name",
            "filename",
            "size",
            "data",
            "bytes",
            "width",
            "height",
        ):
            if hasattr(block, attr):
                value = getattr(block, attr)
                if value is not None:
                    artifact[attr] = value
        artifacts.append(artifact)
    return artifacts


def to_tool_call_view(result: Any) -> ToolCallView:
    """
    Convert an MCPStore CallToolResult into a normalized ToolCallView.

    Args:
        result: The tool call result.

    Returns:
        ToolCallView: The normalized result view.
    """
    contents = _read_field(result, "content", default=[]) or []
    text_blocks = _extract_text_blocks(contents)
    artifacts = _extract_artifacts(contents)
    text_output = "\n".join(text_blocks).strip()

    structured = _read_field(result, "structured_content", "structuredContent", default=None)
    data = _read_field(result, "data", "result", default=None)
    if data is None and artifacts:
        data = {"artifacts": artifacts}

    is_error = bool(_read_field(result, "is_error", "isError", default=False))
    error_message = _read_field(result, "error", "error_message", "message", default=None)
    if is_error and not error_message:
        error_message = text_output or "Tool execution failed"

    return ToolCallView(
        text=text_output,
        content=list(contents),
        artifacts=artifacts,
        structured=structured,
        data=data,
        is_error=is_error,
        error_message=error_message,
    )


def build_tool_error_payload(
    tool_name: str,
    message: str,
    *,
    tool_input: Optional[Dict[str, Any]] = None,
    view: Optional[ToolCallView] = None,
) -> Dict[str, Any]:
    """Build the unified tool error payload."""
    base: Any = None
    if view is not None:
        base = view.structured if view.structured is not None else view.data

    if isinstance(base, dict):
        payload = dict(base)
    else:
        payload = {}

    payload.setdefault("ok", False)
    payload.setdefault("is_error", True)
    payload.setdefault("tool_name", tool_name)
    payload.setdefault("message", message)

    if tool_input:
        payload.setdefault("arguments", dict(tool_input))

    return payload


# ============================================================================
# Schema building
# ============================================================================

# Type mapping table
TYPE_MAPPING = {
    "string": str,
    "number": float,
    "integer": int,
    "boolean": bool,
    "array": list,
    "object": dict,
}

# Reserved field names (avoid clashes with BaseModel attributes)
RESERVED_NAMES = set(dir(BaseModel)) | {
    "schema", "model_json_schema", "model_dump", "dict", "json",
    "copy", "parse_obj", "parse_raw", "construct", "validate",
    "schema_json", "__fields__", "__root__", "Config", "model_config",
}


def _is_valid_field_name(name: str) -> bool:
    """Check whether a field name is valid (identifier, non-keyword, not reserved)."""
    return (
        bool(name)
        and name.isidentifier()
        and not keyword.iskeyword(name)
        and name not in RESERVED_NAMES
        and not name.startswith("_")
    )


def enhance_description(tool_info: Any) -> str:
    """Enhance the tool description (currently returns the original as-is)."""
    description = _read_field(tool_info, "description", default="")
    return description if isinstance(description, str) else str(description or "")


def create_args_schema(tool_info: Any) -> Type[BaseModel]:
    """
    Build a Pydantic arguments model from tool info.

    Args:
        tool_info: The tool info object.

    Returns:
        Type[BaseModel]: The Pydantic model class.
    """
    input_schema = tool_input_schema(tool_info)
    props = input_schema.get("properties", {})

    fields: Dict[str, Any] = {}
    has_invalid_field = False

    for original_name, prop in props.items():
        if not _is_valid_field_name(original_name):
            has_invalid_field = True
            break

        field_type = TYPE_MAPPING.get(prop.get("type", "string"), str)

        # Check nullability with the shared helper
        nullable = is_nullable(prop)

        # Read the default value
        default_value = prop.get("default", ...)

        # Apply Optional typing
        if nullable and field_type is not Any:
            try:
                from typing import Optional as _Optional
                field_type = _Optional[field_type]  # type: ignore
            except Exception:
                pass

        field_kwargs: Dict[str, Any] = {"description": prop.get("description", "")}

        # Preserve nested schema hints (arrays/objects)
        try:
            declared_type = prop.get("type")
            is_array = declared_type == "array" or (isinstance(declared_type, list) and "array" in declared_type)
            is_object = declared_type == "object" or (isinstance(declared_type, list) and "object" in declared_type)
            json_extra: Dict[str, Any] = {}

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

        # Build the field definition
        if default_value != ...:
            fields[original_name] = (field_type, Field(default=default_value, **field_kwargs))
        else:
            fields[original_name] = (field_type, Field(**field_kwargs))

    # Check whether extra properties are allowed
    additional_properties = input_schema.get("additionalProperties", False)
    allow_extra = bool(additional_properties)

    # Build the model
    model_name = f"{tool_name(tool_info).capitalize().replace('_', '')}Input"

    with warnings.catch_warnings():
        warnings.filterwarnings("ignore", category=UserWarning, module="pydantic")

        if (not fields or has_invalid_field) and allow_extra:
            # No fields but an open object: create a permissive model that allows extras
            base = type("OpenArgsBase", (BaseModel,), {"model_config": ConfigDict(extra="allow")})
            return create_model(model_name, __base__=base)

        # Regular model
        base = BaseModel
        if allow_extra:
            base = type("OpenArgsBase", (BaseModel,), {"model_config": ConfigDict(extra="allow")})

        return create_model(model_name, __base__=base, **fields)


# ============================================================================
# Executor builders
# ============================================================================

def build_sync_executor(
    context: Any,
    instance_id: str | None,
    tool_name: str,
    args_schema: Type[BaseModel]
) -> Callable[..., Any]:
    """
    Build a synchronous tool executor.

    Args:
        context: The MCPStore context.
        tool_name: The tool name.
        args_schema: The arguments model.

    Returns:
        Callable: The synchronous executor function.
    """
    def _executor(**kwargs):
        tool_input = {}
        try:
            tool_input = dict(kwargs)
            if instance_id is None:
                result = context.call_tool(tool_name, tool_input)
            else:
                result = context.call_tool(instance_id, tool_name, tool_input)
            view = to_tool_call_view(result)
            if view.is_error:
                payload = build_tool_error_payload(
                    tool_name,
                    view.error_message or view.text or "Tool execution failed",
                    tool_input=tool_input,
                    view=view,
                )
                return json.dumps(payload, ensure_ascii=False)
            actual = view.structured if view.structured is not None else view.data
            if actual is None:
                actual = view.text
            if isinstance(actual, (dict, list)):
                return json.dumps(actual, ensure_ascii=False)
            return str(actual)
        except Exception as e:
            return f"Tool '{tool_name}' execution failed: {e}\nProcessed parameters: {tool_input}"

    _executor.__name__ = tool_name
    _executor.__doc__ = "Auto-generated MCPStore tool wrapper"
    return _executor


def build_async_executor(
    context: Any,
    instance_id: str | None,
    tool_name: str,
    args_schema: Type[BaseModel]
) -> Callable[..., Any]:
    """
    Build an asynchronous tool executor.

    Args:
        context: The MCPStore context.
        tool_name: The tool name.
        args_schema: The arguments model.

    Returns:
        Callable: The asynchronous executor function.
    """
    async def _executor(**kwargs):
        tool_input = {}
        try:
            tool_input = dict(kwargs)
            if instance_id is None:
                result = await asyncio.to_thread(context.call_tool, tool_name, tool_input)
            else:
                result = await asyncio.to_thread(context.call_tool, instance_id, tool_name, tool_input)
            view = to_tool_call_view(result)
            if view.is_error:
                payload = build_tool_error_payload(
                    tool_name,
                    view.error_message or view.text or "Tool execution failed",
                    tool_input=tool_input,
                    view=view,
                )
                return json.dumps(payload, ensure_ascii=False)
            actual = view.structured if view.structured is not None else view.data
            if actual is None:
                actual = view.text
            if isinstance(actual, (dict, list)):
                return json.dumps(actual, ensure_ascii=False)
            return str(actual)
        except Exception as e:
            return f"Tool '{tool_name}' execution failed: {e}\nProcessed parameters: {tool_input}"

    _executor.__name__ = tool_name
    _executor.__doc__ = "Auto-generated async MCPStore tool wrapper"
    return _executor


def attach_signature_from_schema(fn: Callable[..., Any], args_schema: Type[BaseModel]) -> None:
    """
    Attach an inspect.Signature to the function based on args_schema.

    Args:
        fn: The target function.
        args_schema: The arguments model.
    """
    schema_props = args_schema.model_json_schema().get('properties', {})
    params = [inspect.Parameter(k, inspect.Parameter.KEYWORD_ONLY) for k in schema_props.keys()]
    fn.__signature__ = inspect.Signature(parameters=params)  # type: ignore
