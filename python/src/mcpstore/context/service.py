"""Public Service resource over the Rust facade."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from mcpstore.native.records import _base_config_payload, _record_value


class Service:
    """A service identified within one Rust-owned scope."""

    def __init__(self, native: Any):
        self._native = native

    def info(self) -> Dict[str, Any]:
        return _record_value(self._native.info())

    def state(self) -> Dict[str, Any]:
        return _record_value(self._native.state())

    def config(self) -> Dict[str, Any]:
        return _record_value(self._native.config())

    def wait_service(self, timeout: float = 10.0) -> "Service":
        return Service(self._native.wait_service(timeout))

    def disconnect_service(self) -> "Service":
        return Service(self._native.disconnect_service())

    def restart_service(self) -> "Service":
        return Service(self._native.restart_service())

    def patch_service(self, updates: Dict[str, Any]) -> "Service":
        return Service(
            self._native.patch_service(
                _base_config_payload(updates, "Service base config patch")
            )
        )

    def update_service(self, config: Dict[str, Any]) -> "Service":
        return Service(
            self._native.update_service(
                _base_config_payload(config, "Service base config update")
            )
        )

    def remove_service(self) -> bool:
        return bool(self._native.remove_service())

    def list_resources(self) -> List[Dict[str, Any]]:
        return _record_value(self._native.list_resources())

    def list_resource_templates(self) -> List[Dict[str, Any]]:
        return _record_value(self._native.list_resource_templates())

    def read_resource(self, uri: str) -> Dict[str, Any]:
        return _record_value(self._native.read_resource(uri))

    def list_prompts(self) -> List[Dict[str, Any]]:
        return _record_value(self._native.list_prompts())

    def get_prompt(
        self,
        prompt_name: str,
        arguments: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._native.get_prompt(
                prompt_name,
                {} if arguments is None else arguments,
            )
        )

    def list_tools(self) -> List["Tool"]:
        from .tool import Tool

        return [Tool(native) for native in self._native.list_tools()]

    def find_tool(self, tool_name: str) -> "Tool":
        from .tool import Tool

        return Tool(self._native.find_tool(tool_name))


__all__ = ["Service"]
