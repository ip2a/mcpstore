"""Public Store and Agent scope contexts over Rust ``ScopeContext``."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from mcpstore.native.records import _base_config_payload, _record_value

from .service import Service
from .tool import Tool


class StoreContext:
    """Thin Python view of one Rust-owned Store scope."""

    def __init__(self, native: Any):
        self._native = native
        self.scope = _record_value(native.scope())

    def _context(self, native: Any) -> "StoreContext":
        return type(self)(native)

    def show_config(self) -> Dict[str, Any]:
        return _record_value(self._native.show_config())

    def reset_config(self) -> bool:
        return bool(self._native.reset_config())

    def add_service_config(
        self, service_name: str, config: Dict[str, Any]
    ) -> "StoreContext":
        return self._context(self._native.add_service_config(service_name, config))

    def add_service(self, config: Any) -> "StoreContext":
        return self._context(self._native.add_service(config))

    def wait_service(
        self,
        *,
        service_name: Optional[str] = None,
        instance_id: Optional[str] = None,
        timeout: float = 10.0,
    ) -> "StoreContext":
        return self._context(
            self._native.wait_service(
                service_name=service_name,
                instance_id=instance_id,
                timeout=timeout,
            )
        )

    def list_services(self) -> List[Service]:
        return [Service(native) for native in self._native.list_services()]

    def find_service(
        self,
        *,
        service_name: Optional[str] = None,
        instance_id: Optional[str] = None,
    ) -> Service:
        return Service(
            self._native.find_service(
                service_name=service_name,
                instance_id=instance_id,
            )
        )

    def remove_service(
        self,
        *,
        service_name: Optional[str] = None,
        instance_id: Optional[str] = None,
    ) -> bool:
        return bool(
            self._native.remove_service(
                service_name=service_name,
                instance_id=instance_id,
            )
        )

    def disconnect_service(
        self,
        *,
        service_name: Optional[str] = None,
        instance_id: Optional[str] = None,
    ) -> "StoreContext":
        return self._context(
            self._native.disconnect_service(
                service_name=service_name,
                instance_id=instance_id,
            )
        )

    def restart_service(
        self,
        *,
        service_name: Optional[str] = None,
        instance_id: Optional[str] = None,
    ) -> "StoreContext":
        return self._context(
            self._native.restart_service(
                service_name=service_name,
                instance_id=instance_id,
            )
        )

    def patch_service(
        self,
        *,
        service_name: Optional[str] = None,
        instance_id: Optional[str] = None,
        updates: Dict[str, Any],
    ) -> "StoreContext":
        return self._context(
            self._native.patch_service(
                service_name=service_name,
                instance_id=instance_id,
                updates=_base_config_payload(updates, "Service base config patch"),
            )
        )

    def update_service(
        self,
        *,
        service_name: Optional[str] = None,
        instance_id: Optional[str] = None,
        config: Dict[str, Any],
    ) -> "StoreContext":
        return self._context(
            self._native.update_service(
                service_name=service_name,
                instance_id=instance_id,
                config=_base_config_payload(config, "Service base config update"),
            )
        )

    def list_tools(self) -> List[Tool]:
        return [Tool(native) for native in self._native.list_tools()]

    def find_tool(self, tool_name: str) -> Tool:
        return Tool(self._native.find_tool(tool_name))

    def call_tool(
        self, tool_name: str, args: Optional[Dict[str, Any]] = None
    ) -> Dict[str, Any]:
        return _record_value(self._native.call_tool(tool_name, args or {}))


class AgentContext(StoreContext):
    """Thin Python view of one Rust-owned Agent scope."""


__all__ = ["AgentContext", "StoreContext"]
