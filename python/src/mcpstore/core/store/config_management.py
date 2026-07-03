"""Rust-backed compatibility facade for historical config management."""

from __future__ import annotations

from typing import Any, Dict, Optional


class ConfigManagementMixin:
    """Expose config snapshot helpers without reviving the Python manager."""

    def _rust_context(self):
        context = getattr(self, "_context", None)
        if context is not None:
            return context
        if hasattr(self, "get_store_context"):
            return self.get_store_context()
        return self

    def get_json_config(self, client_id: Optional[str] = None) -> Dict[str, Any]:
        if client_id not in (None, "global_agent_store"):
            raise NotImplementedError(
                "client_id-scoped get_json_config() belonged to the legacy Python client manager. "
                "Use Rust-backed context.show_config()/show_mcpjson() snapshots instead."
            )
        return self._rust_context().get_json_config()

    def show_mcpjson(self) -> Dict[str, Any]:
        return self._rust_context().show_mcpjson()

    def show_mcpconfig(self) -> Dict[str, Any]:
        return self.show_mcpjson()

    def get_unified_config(self) -> Any:
        raise NotImplementedError(
            "get_unified_config() was part of the legacy Python configuration manager. "
            "The Rust-backed SDK exposes configuration snapshots through setup_config(), "
            "show_config(), and show_mcpjson() instead."
        )


__all__ = ["ConfigManagementMixin"]
