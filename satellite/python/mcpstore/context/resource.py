"""Public Resource resources over the Rust facade."""

from __future__ import annotations

from typing import Any, Dict, Optional

from mcpstore.native.records import _record_value


class Resource:
    """A resource identified within one Rust-owned scope and service."""

    def __init__(self, native: Any):
        self._native = native

    def info(self) -> Dict[str, Any]:
        return _record_value(self._native.info())

    def read(self) -> Dict[str, Any]:
        return _record_value(self._native.read())

    def set_override(self, patch: Dict[str, Any]) -> Dict[str, Any]:
        return _record_value(self._native.set_override(patch))

    def get_override(self) -> Optional[Dict[str, Any]]:
        return _record_value(self._native.get_override())

    def delete_override(self) -> None:
        self._native.delete_override()

    def enable(self) -> None:
        self._native.enable()

    def disable(self) -> None:
        self._native.disable()


class ResourceTemplate:
    """A resource template identified within one Rust-owned service."""

    def __init__(self, native: Any):
        self._native = native

    def info(self) -> Dict[str, Any]:
        return _record_value(self._native.info())

    def set_override(self, patch: Dict[str, Any]) -> Dict[str, Any]:
        return _record_value(self._native.set_override(patch))

    def get_override(self) -> Optional[Dict[str, Any]]:
        return _record_value(self._native.get_override())

    def delete_override(self) -> None:
        self._native.delete_override()

    def enable(self) -> None:
        self._native.enable()

    def disable(self) -> None:
        self._native.disable()


__all__ = ["Resource", "ResourceTemplate"]
