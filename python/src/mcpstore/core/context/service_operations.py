"""Rust-backed compatibility helpers for historical service operations."""

from __future__ import annotations

from typing import Any, Dict, Optional, Union


class AddServiceWaitStrategy:
    """Historical wait parser retained as a pure compatibility helper."""

    default_timeouts = {"remote": 2000, "local": 4000}

    def parse_wait_parameter(self, wait_param: Union[str, int, float]) -> Optional[float]:
        if wait_param == "auto":
            return None
        try:
            return max(0.1, min(30.0, float(wait_param) / 1000.0))
        except (TypeError, ValueError):
            return None

    def get_service_wait_timeout(self, service_config: Dict[str, Any]) -> float:
        return (self.default_timeouts["remote"] if service_config.get("url") else self.default_timeouts["local"]) / 1000.0

    def get_max_wait_timeout(self, services_config: Dict[str, Dict[str, Any]]) -> float:
        if not services_config:
            return 2.0
        return max(self.get_service_wait_timeout(config) for config in services_config.values())


class ServiceOperationsMixin:
    """Compatibility mixin delegating service operations to RustStoreContext."""

    @staticmethod
    def _find_mcp_servers_key(config: Dict[str, Any]) -> Optional[str]:
        if not isinstance(config, dict):
            return None
        return next((key for key in config if str(key).lower() == "mcpservers"), None)

    @staticmethod
    def _normalize_mcp_servers(config: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        key = ServiceOperationsMixin._find_mcp_servers_key(config)
        if key is None:
            return None
        if key == "mcpServers":
            return config
        normalized = {item_key: value for item_key, value in config.items() if item_key != key}
        normalized["mcpServers"] = config[key]
        return normalized

    def list_services(self):
        return self._rust_context().list_services()

    async def list_services_async(self):
        return self.list_services()

    def add_service(self, config=None, json_file=None, auth=None, token=None, api_key=None, headers=None):
        if auth is not None and token is None:
            token = auth
        final_headers = dict(headers or {})
        if token:
            final_headers["Authorization"] = f"Bearer {token}"
        if api_key:
            final_headers["X-API-Key"] = api_key
        return self._rust_context().add_service(
            config,
            json_file=json_file,
            headers=final_headers or None,
        )

    async def add_service_async(self, *args, **kwargs):
        return self.add_service(*args, **kwargs)

    def add_service_with_details(self, config=None):
        self.add_service(config)
        return {"success": True, "services": self.list_services()}

    async def add_service_with_details_async(self, config=None):
        return self.add_service_with_details(config)

    def get_service_info(self, name: str):
        return self._rust_context().get_service_info(name)

    async def get_service_info_async(self, name: str):
        return self.get_service_info(name)

    def get_service_status(self, name: str):
        return self._rust_context().get_service_status(name)

    async def get_service_status_async(self, name: str):
        return self.get_service_status(name)

    def check_services(self):
        return self._rust_context().check_services()

    async def check_services_async(self):
        return self.check_services()

    def update_service(self, name: str, config: Any):
        return self._rust_context().update_service(name, config)

    async def update_service_async(self, name: str, config: Any):
        return self.update_service(name, config)

    def patch_service(self, name: str, updates: Any):
        return self._rust_context().patch_service(name, updates)

    async def patch_service_async(self, name: str, updates: Any):
        return self.patch_service(name, updates)

    def delete_service(self, name: str):
        return self._rust_context().delete_service(name)

    def remove_service(self, name: str):
        return self.delete_service(name)

    async def delete_service_async(self, name: str):
        return self.delete_service(name)

    async def remove_service_async(self, name: str):
        return self.remove_service(name)

    def restart_service(self, name: str):
        return self._rust_context().restart_service(name)

    async def restart_service_async(self, name: str):
        return self.restart_service(name)

    def connect_service(self, name: str):
        return self._rust_context().connect_service(name)

    def disconnect_service(self, name: str):
        return self._rust_context().disconnect_service(name)

    def show_config(self, scope: str = "all"):
        return self._rust_context().show_config(scope)

    async def show_config_async(self, scope: str = "all"):
        return self.show_config(scope)

    def reset_config(self):
        return self._rust_context().reset_config()

    async def reset_config_async(self):
        return self.reset_config()

    def wait_service(self, name: str, status=None, timeout: float = 10.0):
        return self._rust_context().wait_service(name, status=status, timeout=timeout)

    def wait_services(self, names, status=None, timeout: float = 10.0):
        return self._rust_context().wait_services(names, status=status, timeout=timeout)

    def _rust_context(self):
        return getattr(self, "_context", self)


__all__ = ["AddServiceWaitStrategy", "ServiceOperationsMixin"]
