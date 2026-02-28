# src/mcpstore/adapters/common.py
"""
适配器公共工具模块

提供所有适配器共享的工具函数，避免代码重复：
- is_nullable: 检查 JSON Schema 属性是否可为 null
- process_tool_args: 统一处理工具参数转换
- create_args_schema: 创建 Pydantic 参数模型
- call_tool_response_helper: 统一处理工具调用结果
"""
from __future__ import annotations

import inspect
import json
import keyword
import warnings
from typing import TYPE_CHECKING, Callable, Any, Type, List, Dict, Optional, Tuple

from pydantic import BaseModel, create_model, Field, ConfigDict

if TYPE_CHECKING:
    from ..core.context.base_context import MCPStoreContext
    from ..core.models.tool import ToolInfo


__all__ = [
    # 公共工具函数
    'is_nullable',
    'process_tool_args',
    'enhance_description',
    'create_args_schema',
    'call_tool_response_helper',
    # 执行器构建
    'build_sync_executor',
    'build_async_executor',
    'attach_signature_from_schema',
    # 数据类
    'ToolCallView',
]


# ============================================================================
# 数据类
# ============================================================================

class ToolCallView(BaseModel):
    """标准化 MCPStore CallToolResult 的辅助视图。"""

    text: str = ""
    artifacts: List[Dict[str, Any]] = Field(default_factory=list)
    structured: Any = None
    data: Any = None
    is_error: bool = False
    error_message: Optional[str] = None
    raw: Any = None


# ============================================================================
# JSON Schema 工具函数
# ============================================================================

def is_nullable(prop: Dict[str, Any]) -> bool:
    """
    检查 JSON Schema 属性是否可为 null。

    支持以下 nullability 表示方式：
    - nullable: true
    - type: ["string", "null"]
    - anyOf: [{"type": "string"}, {"type": "null"}]
    - oneOf: [{"type": "string"}, {"type": "null"}]
    - default: null

    Args:
        prop: JSON Schema 属性定义

    Returns:
        bool: 是否可为 null
    """
    try:
        # 显式 nullable 标记
        if prop.get("nullable") is True:
            return True

        # type 数组包含 "null"
        t = prop.get("type")
        if isinstance(t, list) and "null" in t:
            return True

        # anyOf 包含 null 类型
        any_of = prop.get("anyOf") or []
        if isinstance(any_of, list) and any(
            (isinstance(x, dict) and x.get("type") == "null") for x in any_of
        ):
            return True

        # oneOf 包含 null 类型
        one_of = prop.get("oneOf") or []
        if isinstance(one_of, list) and any(
            (isinstance(x, dict) and x.get("type") == "null") for x in one_of
        ):
            return True

        # default 值为 null
        if prop.get("default", object()) is None:
            return True

    except Exception:
        pass

    return False


# ============================================================================
# 参数处理
# ============================================================================

def process_tool_args(
    args_schema: Type[BaseModel],
    args: tuple,
    kwargs: dict
) -> Dict[str, Any]:
    """
    统一处理工具参数转换。

    将各种参数传递方式（位置参数、关键字参数、字典）转换为标准的工具输入字典。
    支持无参数工具和开放 schema 工具。

    Args:
        args_schema: Pydantic 参数模型
        args: 位置参数元组
        kwargs: 关键字参数字典

    Returns:
        Dict[str, Any]: 标准化的工具输入字典
    """
    tool_input: Dict[str, Any] = {}

    try:
        # 获取模型字段信息
        schema_info = args_schema.model_json_schema()
        schema_fields = schema_info.get('properties', {})
        field_names = list(schema_fields.keys())
        allow_extra = bool(schema_info.get("additionalProperties", False))

        # 处理无参数工具 / 开放 schema 工具
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
            # 有声明字段时的参数处理
            if kwargs:
                tool_input = dict(kwargs)
            elif args:
                if len(args) == 1:
                    if isinstance(args[0], dict):
                        tool_input = dict(args[0])
                    else:
                        # 单个位置参数映射到第一个字段
                        tool_input = {field_names[0]: args[0]}
                else:
                    # 多个位置参数按顺序映射到字段
                    for i, arg_value in enumerate(args):
                        if i < len(field_names):
                            tool_input[field_names[i]] = arg_value

    except Exception:
        # 降级处理：直接使用 kwargs
        tool_input = dict(kwargs) if kwargs else {}

    return tool_input


# ============================================================================
# 结果处理
# ============================================================================

def _extract_text_blocks(contents: list) -> List[str]:
    """从内容块中提取文本。"""
    blocks: List[str] = []
    for block in contents or []:
        text = getattr(block, "text", None)
        if isinstance(text, str):
            blocks.append(text)
    return blocks


def _extract_artifacts(contents: list) -> List[Dict[str, Any]]:
    """从内容块中提取工件（非文本内容）。"""
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
    将 MCPStore CallToolResult 统一转换为 ToolCallView。

    Args:
        result: 工具调用结果

    Returns:
        ToolCallView: 标准化的结果视图
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


# ============================================================================
# Schema 构建
# ============================================================================

# 类型映射表
TYPE_MAPPING = {
    "string": str,
    "number": float,
    "integer": int,
    "boolean": bool,
    "array": list,
    "object": dict,
}

# 保留字段名（避免与 BaseModel 属性冲突）
RESERVED_NAMES = set(dir(BaseModel)) | {
    "schema", "model_json_schema", "model_dump", "dict", "json",
    "copy", "parse_obj", "parse_raw", "construct", "validate",
    "schema_json", "__fields__", "__root__", "Config", "model_config",
}


def _is_valid_field_name(name: str) -> bool:
    """检查字段名是否有效（合法标识符、非关键字、非保留名）。"""
    return (
        bool(name)
        and name.isidentifier()
        and not keyword.iskeyword(name)
        and name not in RESERVED_NAMES
        and not name.startswith("_")
    )


def enhance_description(tool_info: 'ToolInfo') -> str:
    """增强工具描述（当前仅返回原描述）。"""
    return tool_info.description or ""


def create_args_schema(tool_info: 'ToolInfo') -> Type[BaseModel]:
    """
    从 ToolInfo 创建 Pydantic 参数模型。

    Args:
        tool_info: 工具信息对象

    Returns:
        Type[BaseModel]: Pydantic 模型类
    """
    props = tool_info.inputSchema.get("properties", {})
    required = tool_info.inputSchema.get("required", [])

    fields: Dict[str, Tuple[type, Any]] = {}
    has_invalid_field = False

    for original_name, prop in props.items():
        if not _is_valid_field_name(original_name):
            has_invalid_field = True
            break

        field_type = TYPE_MAPPING.get(prop.get("type", "string"), str)

        # 使用公共函数检查 nullability
        nullable = is_nullable(prop)

        # 获取默认值
        default_value = prop.get("default", ...)

        # 应用 Optional 类型
        if nullable and field_type is not Any:
            try:
                from typing import Optional as _Optional
                field_type = _Optional[field_type]  # type: ignore
            except Exception:
                pass

        field_kwargs: Dict[str, Any] = {"description": prop.get("description", "")}

        # 保留嵌套 schema 提示（数组/对象）
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

        # 构建字段定义
        if default_value != ...:
            fields[original_name] = (field_type, Field(default=default_value, **field_kwargs))
        else:
            fields[original_name] = (field_type, Field(**field_kwargs))

    # 检查是否允许额外属性
    additional_properties = tool_info.inputSchema.get("additionalProperties", False)
    allow_extra = bool(additional_properties)

    # 构建模型
    model_name = f"{tool_info.name.capitalize().replace('_', '')}Input"

    with warnings.catch_warnings():
        warnings.filterwarnings("ignore", category=UserWarning, module="pydantic")

        if (not fields or has_invalid_field) and allow_extra:
            # 无字段但开放对象：创建允许 extra 的宽松模型
            base = type("OpenArgsBase", (BaseModel,), {"model_config": ConfigDict(extra="allow")})
            return create_model(model_name, __base__=base)

        # 正常模型
        base = BaseModel
        if allow_extra:
            base = type("OpenArgsBase", (BaseModel,), {"model_config": ConfigDict(extra="allow")})

        return create_model(model_name, __base__=base, **fields)


# ============================================================================
# 执行器构建
# ============================================================================

def build_sync_executor(
    context: 'MCPStoreContext',
    tool_name: str,
    args_schema: Type[BaseModel]
) -> Callable[..., Any]:
    """
    构建同步工具执行器。

    Args:
        context: MCPStore 上下文
        tool_name: 工具名称
        args_schema: 参数模型

    Returns:
        Callable: 同步执行函数
    """
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


def build_async_executor(
    context: 'MCPStoreContext',
    tool_name: str,
    args_schema: Type[BaseModel]
) -> Callable[..., Any]:
    """
    构建异步工具执行器。

    Args:
        context: MCPStore 上下文
        tool_name: 工具名称
        args_schema: 参数模型

    Returns:
        Callable: 异步执行函数
    """
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
    """
    根据 args_schema 为函数附加 inspect.Signature。

    Args:
        fn: 目标函数
        args_schema: 参数模型
    """
    schema_props = args_schema.model_json_schema().get('properties', {})
    params = [inspect.Parameter(k, inspect.Parameter.KEYWORD_ONLY) for k in schema_props.keys()]
    fn.__signature__ = inspect.Signature(parameters=params)  # type: ignore
