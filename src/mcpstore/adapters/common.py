# src/mcpstore/adapters/common.py
from __future__ import annotations

import inspect
import json
import keyword
import re
from typing import TYPE_CHECKING, Callable, Any, Type

from pydantic import BaseModel, create_model, Field, ConfigDict
import warnings

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
        line = f"- {name} ({param_type}): {param_desc}"
        # If this is an array of objects, append nested shape hints
        try:
            if (param_type == "array" or (isinstance(param_type, list) and "array" in param_type)) and isinstance(info.get("items"), dict):
                items = info["items"]
                if items.get("type") == "object" and "properties" in items:
                    nested = []
                    for nkey, nprop in items["properties"].items():
                        ntype = nprop.get("type", "string")
                        ndesc = nprop.get("description", "")
                        nested.append(f"    - {name}[].{nkey} ({ntype}) {ndesc}")
                    if nested:
                        line += "\n" + "\n".join(nested)
        except Exception:
            pass
        param_lines.append(line)
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

    fields: dict[str, tuple[type, Any]] = {}
    for original_name, prop in props.items():
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
        is_required = original_name in required

        # Handle default values: make non-required fields truly optional
        default_value = prop.get("default", ...)
        if not is_required and default_value == ...:
            # Use None as a sentinel default so Pydantic treats field as optional
            # Combined with exclude_unset=True, unset optionals won't be sent
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

        # Preserve nested schema hints for arrays/objects so model_json_schema() retains details
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
                validated.model_dump(
                    by_alias=True,          # Use original parameter names
                    exclude_unset=True,     # Don't send unset parameters
                    exclude_none=False,     # Preserve explicit None values
                    exclude_defaults=False  # Preserve default values (service may require them)
                )
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
            validated.model_dump(
                by_alias=True,          # Use original parameter names
                exclude_unset=True,     # Don't send unset parameters
                exclude_none=False,     # Preserve explicit None values
                exclude_defaults=False  # Preserve default values (service may require them)
            )
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

