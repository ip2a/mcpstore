# src/mcpstore/adapters/common.py
from __future__ import annotations

import inspect
import json
from typing import TYPE_CHECKING, Callable, Any, Type

from pydantic import BaseModel, create_model, Field, ConfigDict
import warnings
import re

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

    # Build reserved names set (avoid BaseModel attributes like 'schema')
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

    fields: dict[str, tuple[type, Any]] = {}
    for original_name, prop in props.items():
        field_type = type_mapping.get(prop.get("type", "string"), str)
        default_value = prop.get("default", ...)

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

        if original_name not in required and default_value == ...:
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

        # If schema is nullable and no explicit default provided, prefer None
        if is_nullable and default_value == ...:
            default_value = None

        # Apply Optional typing if nullable
        try:
            if is_nullable and field_type is not Any:
                from typing import Optional as _Optional
                field_type = _Optional[field_type]  # type: ignore
        except Exception:
            pass

        safe_name = sanitize_name(original_name)
        field_kwargs = {"description": prop.get("description", "")}
        # If we renamed, keep external alias stable
        if safe_name != original_name:
            field_kwargs["validation_alias"] = original_name
            field_kwargs["serialization_alias"] = original_name

        if default_value != ...:
            fields[safe_name] = (field_type, Field(default=default_value, **field_kwargs))
        else:
            fields[safe_name] = (field_type, Field(**field_kwargs))

    # Detect whether schema allows additionalProperties
    additional_properties = tool_info.inputSchema.get("additionalProperties", False)
    allow_extra = bool(additional_properties)  # dict/True both视为允许

    if not fields and allow_extra:
        # No declared fields but open object: create permissive model with extra=allow
        base = type("OpenArgsBase", (BaseModel,), {"model_config": ConfigDict(extra="allow")})
        with warnings.catch_warnings():
            # 忽略 pydantic 关于字段名冲突的警告（已通过 sanitize_name 处理）
            warnings.filterwarnings(
                "ignore",
                category=UserWarning,
                module="pydantic",
            )
            return create_model(f"{tool_info.name.capitalize().replace('_', '')}Input", __base__=base)

    if not fields:
        fields["input"] = (str, Field(description="Tool input"))

    # Suppress specific Pydantic warning about shadowing BaseModel attributes
    with warnings.catch_warnings():
        # 忽略 pydantic 关于字段名冲突的警告（已通过 sanitize_name 处理）
        warnings.filterwarnings(
            "ignore",
            category=UserWarning,
            module="pydantic",
        )
        # Create model; if open schema, allow extras
        base = BaseModel
        if allow_extra:
            base = type("OpenArgsBase", (BaseModel,), {"model_config": ConfigDict(extra="allow")})
        return create_model(f"{tool_info.name.capitalize().replace('_', '')}Input", __base__=base, **fields)


def build_sync_executor(context: 'MCPStoreContext', tool_name: str, args_schema: Type[BaseModel]) -> Callable[..., Any]:
    def _executor(**kwargs):
        tool_input = {}
        try:
            schema_info = args_schema.model_json_schema()
            schema_fields = schema_info.get('properties', {})
            field_names = list(schema_fields.keys())
            allow_extra = bool(schema_info.get('additionalProperties', False))
            tool_input = dict(kwargs) if allow_extra else {k: v for k, v in kwargs.items() if k in field_names}
            try:
                validated = args_schema(**tool_input)
            except Exception:
                filtered = {k: kwargs[k] for k in field_names if k in kwargs}
                validated = args_schema(**filtered)
            result = context.call_tool(
                tool_name,
                validated.model_dump(exclude_none=True, exclude_unset=True, exclude_defaults=True)
            )
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
        result = await context.call_tool_async(
            tool_name,
            validated.model_dump(exclude_none=True, exclude_unset=True, exclude_defaults=True)
        )
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

