"""Python interface for one concrete service instance."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict, List, Optional

from mcpstore.native.records import _record_value
from mcpstore.tools.proxy import RustToolProxy

if TYPE_CHECKING:
    from mcpstore.store.store import RustStoreBackend

class RustServiceProxy:
    """Proxy for one concrete service instance."""

    def __init__(self, backend: RustStoreBackend, instance_id: str):
        self._backend = backend
        self.instance_id = instance_id

    def info(self) -> Dict[str, Any]:
        record = self._backend.find_instance(self.instance_id)
        if record is None:
            raise KeyError(f"Instance not found: {self.instance_id}")
        return record

    @property
    def service_name(self) -> str:
        return str(self.info()["service_name"])

    @property
    def scope(self) -> Dict[str, Any]:
        return _record_value(self.info()["scope"])

    def connect(self) -> "RustServiceProxy":
        self._backend.connect_service(self.instance_id)
        return self

    def disconnect(self) -> "RustServiceProxy":
        self._backend.disconnect_service(self.instance_id)
        return self

    def restart(self) -> "RustServiceProxy":
        self._backend.restart_service(self.instance_id)
        return self

    def wait_ready(self, timeout_secs: int = 10) -> Dict[str, Any]:
        return self._backend.wait_instance_ready(self.instance_id, timeout_secs)

    def list_tools(self) -> List[Dict[str, Any]]:
        return self._backend.list_tools(self.instance_id)

    def find_tool(self, tool_name: str) -> "RustToolProxy":
        return RustToolProxy(self._backend, self.instance_id, tool_name)

    def call_tool(
        self,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._backend.call_tool(self.instance_id, tool_name, args)

    def export_config(self, format: Optional[str] = None) -> Dict[str, Any]:
        return self._backend.export_instance_config(self.instance_id, format)


ServiceProxy = RustServiceProxy

__all__ = ["RustServiceProxy", "ServiceProxy"]
