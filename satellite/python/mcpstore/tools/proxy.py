"""Python interface for one tool on one service instance."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict, Optional

if TYPE_CHECKING:
    from mcpstore.store.store import RustStoreBackend

class RustToolProxy:
    """Proxy for one tool owned by one concrete instance."""

    def __init__(self, backend: RustStoreBackend, instance_id: str, tool_name: str):
        self._backend = backend
        self.instance_id = instance_id
        self.tool_name = tool_name

    def call(self, args: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return self._backend.call_tool(self.instance_id, self.tool_name, args)



ToolProxy = RustToolProxy

__all__ = ["RustToolProxy", "ToolProxy"]
