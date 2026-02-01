# 补充类型提示文件，用于静态检查与可读性
# 将 MCP 协议返回的 CallToolResult 抽象为 Protocol，避免直接绑定具体实现

from typing import Protocol, Any, List, Dict, Optional


class CallToolResultProtocol(Protocol):
    data: Any
    content: List[Any]
    structured_content: Optional[Dict[str, Any]]
    is_error: bool

