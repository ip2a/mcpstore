"""Historical tool proxy typing helpers."""

from typing import Any, Dict, List, Optional, Protocol


class CallToolResultProtocol(Protocol):
    data: Any
    content: List[Any]
    structured_content: Optional[Dict[str, Any]]
    is_error: bool


__all__ = ["CallToolResultProtocol"]
