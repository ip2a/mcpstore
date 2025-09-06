# src/mcpstore/adapters/common.py
from __future__ import annotations

import json
import inspect
from typing import TYPE_CHECKING, Callable, Any, Type

from pydantic import BaseModel, create_model, Field

if TYPE_CHECKING:
    from ..core.context.base_context import MCPStoreContext
    from ..core.models.tool import ToolInfo


def enhance_description(tool_info: 'ToolInfo') -> str:
    base_description = tool_info.description or ""
    schema_properties = tool_info.inputSchema.get("properties", {})
    if not schema_properties:
        return base_description
    param_lines = []
    for name, info in schema_properties.items():
        param_type = info.get("type", "string")
        param_desc = info.get("description", "")
        param_lines.append(f"- {name} ({param_type}): {param_desc}")
    return base_description + ("\n\nParameter descriptions:\n" + "\n".join(param_lines))


def create_args_schema(tool_info: 'ToolInfo') -> Type[BaseModel]:
    props = tool_info.inputSchema.get("properties", {})
    required = tool_info.inputSchema.get("required", [])
    type_mapping = {
        "string": str, "number": float, "integer": int,
        "boolean": bool, "array": list, "object": dict
    }
    fields: dict[str, tuple[type, Any]] = {}
    for name, prop in props.items():
        field_type = type_mapping.get(prop.get("type", "string"), str)
        default_value = prop.get("default", ...)
        if name not in required and default_value == ...:
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
        if default_value != ...:
            fields[name] = (field_type, Field(default=default_value, description=prop.get("description", "")))
        else:
            fields[name] = (field_type, ...)
    if not fields:
        fields["input"] = (str, Field(description="Tool input"))
    return create_model(f"{tool_info.name.capitalize().replace('_', '')}Input", **fields)


def build_sync_executor(context: 'MCPStoreContext', tool_name: str, args_schema: Type[BaseModel]) -> Callable[..., Any]:
    def _executor(**kwargs):
        tool_input = {}
        try:
            schema_info = args_schema.model_json_schema()
            schema_fields = schema_info.get('properties', {})
            field_names = list(schema_fields.keys())
            tool_input = {k: v for k, v in kwargs.items() if k in field_names}
            try:
                validated = args_schema(**tool_input)
            except Exception:
                filtered = {k: kwargs[k] for k in field_names if k in kwargs}
                validated = args_schema(**filtered)
            result = context.call_tool(tool_name, validated.model_dump())
            actual = getattr(result, 'result', None)
            if actual is None and getattr(result, 'success', False):
                actual = getattr(result, 'data', str(result))
            if isinstance(actual, (dict, list)):
                return json.dumps(actual, ensure_ascii=False)
            return str(actual)
        except Exception as e:
            return f"Tool '{tool_name}' execution failed: {e}\nProcessed parameters: {tool_input}"
    _executor.__name__ = tool_name
    _executor.__doc__ = "Auto-generated MCPStore tool wrapper"
    return _executor


def build_async_executor(context: 'MCPStoreContext', tool_name: str, args_schema: Type[BaseModel]) -> Callable[..., Any]:
    async def _executor(**kwargs):
        validated = args_schema(**kwargs)
        result = await context.call_tool_async(tool_name, validated.model_dump())
        actual = getattr(result, 'result', None)
        if actual is None and getattr(result, 'success', False):
            actual = getattr(result, 'data', str(result))
        if isinstance(actual, (dict, list)):
            return json.dumps(actual, ensure_ascii=False)
        return str(actual)
    _executor.__name__ = tool_name
    _executor.__doc__ = "Auto-generated MCPStore tool wrapper (async)"
    return _executor


def attach_signature_from_schema(fn: Callable[..., Any], args_schema: Type[BaseModel]) -> None:
    """Attach an inspect.Signature to function based on args_schema for better introspection."""
    schema_props = args_schema.model_json_schema().get('properties', {})
    params = [inspect.Parameter(k, inspect.Parameter.KEYWORD_ONLY) for k in schema_props.keys()]
    fn.__signature__ = inspect.Signature(parameters=params)  # type: ignore

