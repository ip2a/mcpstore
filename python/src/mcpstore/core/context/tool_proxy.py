"""Compatibility exports for tool proxy objects."""

from __future__ import annotations

from datetime import datetime
from typing import Any, Dict

from mcpstore.core.store.rust_backend import RustToolProxy as ToolProxy


class ToolCallResult:
    """Small compatibility wrapper for MCP tool call results."""

    def __init__(self, mcp_result: Any, tool_name: str, arguments: Dict[str, Any]):
        self._result = mcp_result
        self._tool_name = tool_name
        self._arguments = arguments
        self._called_at = datetime.now()

    @property
    def data(self):
        return _read_field(self._result, "data")

    @property
    def content(self):
        return _read_field(self._result, "content", default=[])

    @property
    def artifacts(self):
        artifacts = []
        for item in self.content or []:
            item_type = _read_field(item, "type", default=None)
            if item_type == "text" or (item_type is None and _read_field(item, "text", default=None) is not None):
                continue
            artifacts.append(_plain_value(item))
        return artifacts

    @property
    def structured_content(self):
        return _read_field(self._result, "structured_content", "structuredContent")

    @property
    def is_error(self) -> bool:
        return bool(_read_field(self._result, "is_error", "isError", default=False))

    @property
    def text_output(self) -> str:
        text = _read_field(self._result, "text_output", default=None)
        if text is not None:
            return str(text)
        content = self.content or []
        if content:
            first = content[0]
            value = _read_field(first, "text", default=None)
            if value is not None:
                return str(value)
        if self.data is not None:
            return str(self.data)
        return ""

    @property
    def tool_name(self) -> str:
        return self._tool_name

    @property
    def arguments(self) -> Dict[str, Any]:
        return self._arguments

    @property
    def called_at(self) -> datetime:
        return self._called_at

    def to_dict(self) -> Dict[str, Any]:
        return {
            "tool_name": self.tool_name,
            "arguments": self.arguments,
            "called_at": self.called_at.isoformat(),
            "is_error": self.is_error,
            "data": self.data,
            "content": _plain_value(self.content),
            "artifacts": self.artifacts,
            "text_output": self.text_output,
            "structured_content": _plain_value(self.structured_content),
            "has_structured_content": self.structured_content is not None,
        }

    def __repr__(self) -> str:
        status = "ERROR" if self.is_error else "SUCCESS"
        return f"ToolCallResult(tool={self.tool_name!r}, status={status})"

    __str__ = __repr__


def _read_field(value: Any, *names: str, default: Any = None) -> Any:
    for name in names:
        if isinstance(value, dict) and name in value:
            return value[name]
        if hasattr(value, name):
            return getattr(value, name)
    return default


def _plain_value(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: _plain_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_plain_value(item) for item in value]
    if hasattr(value, "to_dict"):
        return value.to_dict()
    if not isinstance(value, (str, bytes, int, float, bool, type(None))):
        fields = {}
        for name in (
            "type",
            "text",
            "data",
            "mime_type",
            "mimeType",
            "resource",
            "annotations",
            "uri",
            "name",
            "title",
            "description",
            "size",
            "width",
            "height",
        ):
            if hasattr(value, name):
                field_value = getattr(value, name)
                if field_value is not None:
                    fields[name] = _plain_value(field_value)
        if fields:
            return fields
    return value


__all__ = ["ToolProxy", "ToolCallResult"]
