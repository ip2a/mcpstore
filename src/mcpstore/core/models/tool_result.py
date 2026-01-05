"""
CallToolResult 辅助模型

提供与 FastMCP 官方 `CallToolResult` 完全兼容的失败结果封装，保证调用链
无论成功还是失败都能拿到统一的数据结构。
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional, Any

from mcp import types as mcp_types


@dataclass
class CallToolFailureResult:
    """
    FastMCP CallToolResult 的失败封装。

    通过标准的文本内容块返回错误信息，同时补齐 FastMCP 客户端常用的
    `structured_content`、`data`、`error`、`is_error` 等字段，便于调用方直接
    当作官方结果使用。
    """

    message: str
    cause: Optional[Any] = None
    _result: mcp_types.CallToolResult = field(init=False, repr=False)

    def __post_init__(self) -> None:
        text_block = mcp_types.TextContent(type="text", text=self.message)
        failure = mcp_types.CallToolResult(
            content=[text_block],
            structuredContent=None,
            isError=True,
        )
        # FastMCP 官方对象同时会暴露蛇形和驼峰字段，这里补齐常用别名
        setattr(failure, "structured_content", None)
        setattr(failure, "data", None)
        setattr(failure, "error", self.message)
        setattr(failure, "is_error", True)
        if self.cause is not None:
            setattr(failure, "cause", str(self.cause))
        self._result = failure

    def unwrap(self) -> mcp_types.CallToolResult:
        """返回 FastMCP 官方 CallToolResult 对象。"""
        return self._result

    def __getattr__(self, item: str) -> Any:
        return getattr(self._result, item)

    def __repr__(self) -> str:
        return f"CallToolFailureResult(message={self.message!r})"
