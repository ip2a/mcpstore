# src/mcpstore/adapters/common.py
from __future__ import annotations

import inspect
import json
import keyword
import warnings
from typing import TYPE_CHECKING, Callable, Any, Type, List, Dict, Optional

from pydantic import BaseModel, create_model, Field, ConfigDict

if TYPE_CHECKING:
    from ..core.context.base_context import MCPStoreContext
    from ..core.models.tool import ToolInfo


class ToolCallView(BaseModel):
    """标准化 MCPStore CallToolResult 的辅助视图。"""

    text: str = ""
    artifacts: List[Dict[str, Any]] = Field(default_factory=list)
    structured: Any = None
    data: Any = None
    is_error: bool = False
    error_message: Optional[str] = None
    raw: Any = None


def _extract_text_blocks(contents: list) -> List[str]:
    blocks: List[str] = []
    for block in contents or []:
        text = getattr(block, "text", None)
        if isinstance(text, str):
            blocks.append(text)
    return blocks


def _extract_artifacts(contents: list) -> List[Dict[str, Any]]:
    artifacts: List[Dict[str, Any]] = []
    for block in contents or []:
        if hasattr(block, "text"):
            continue
        artifact = {"type": getattr(block, "type", block.__class__.__name__.lower())}
        for attr in ("uri", "mime", "mime_type", "name", "filename", "size", "bytes", "width", "height"):
            if hasattr(block, attr):
                value = getattr(block, attr)
                if value is not None:
                    artifact[attr] = value
        artifacts.append(artifact)
    return artifacts


def call_tool_response_helper(result: Any) -> ToolCallView:
    """
    将 MCPStore CallToolResult 统一转换为 ToolCallView，供适配器复用。
    """
    contents = getattr(result, "content", []) or []
    text_blocks = _extract_text_blocks(contents)
    artifacts = _extract_artifacts(contents)
    text_output = "\n".join(text_blocks).strip()

    structured = getattr(result, "structured_content", None)
    data = getattr(result, "data", None)
    if not text_output and data is not None:
        text_output = str(data)

    is_error = bool(getattr(result, "is_error", False) or getattr(result, "isError", False))
    error_message = getattr(result, "error", None)
    if is_error and not error_message:
        error_message = text_output or "Tool execution failed"

    return ToolCallView(
        text=text_output,
        artifacts=artifacts,
        structured=structured,
        data=data,
        is_error=is_error,
        error_message=error_message,
        raw=result,
    )


def enhance_description(tool_info: 'ToolInfo') -> str:
    return tool_info.description or ""


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

    def is_valid_field_name(name: str) -> bool:
        return bool(name) and name.isidentifier() and not keyword.iskeyword(name) and name not in reserved_names and not name.startswith("_")

    fields: dict[str, tuple[type, Any]] = {}
    has_invalid_field = False
    for original_name, prop in props.items():
        if not is_valid_field_name(original_name):
            has_invalid_field = True
            break
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

        # Handle default values without injecting implicit defaults
        default_value = prop.get("default", ...)

        # Apply Optional typing if nullable
        try:
            if is_nullable and field_type is not Any:
                from typing import Optional as _Optional
                field_type = _Optional[field_type]  # type: ignore
        except Exception:
            pass

        field_kwargs = {"description": prop.get("description", "")}

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
            fields[original_name] = (field_type, Field(default=default_value, **field_kwargs))
        else:
            fields[original_name] = (field_type, Field(**field_kwargs))

    # Detect whether schema allows additionalProperties
    additional_properties = tool_info.inputSchema.get("additionalProperties", False)
    allow_extra = bool(additional_properties)  # dict/True both considered as allowed

    if (not fields or has_invalid_field) and allow_extra:
        # No declared fields but open object: create permissive model with extra=allow
        base = type("OpenArgsBase", (BaseModel,), {"model_config": ConfigDict(extra="allow")})
        with warnings.catch_warnings():
            # Ignore pydantic warnings about field name conflicts (handled via sanitize_name)
            warnings.filterwarnings(
                "ignore",
                category=UserWarning,
                module="pydantic",
            )
            return create_model(f"{tool_info.name.capitalize().replace('_', '')}Input", __base__=base)

    if not fields:
        pass

    # Suppress specific Pydantic warning about shadowing BaseModel attributes
    with warnings.catch_warnings():
        # Ignore pydantic warnings about field name conflicts (already handled by sanitize_name)
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
            tool_input = dict(kwargs)
            result = context.call_tool(tool_name, tool_input)
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
        result = await context.call_tool_async(tool_name, dict(kwargs))
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
