"""Public Prompt resource over the Rust facade."""

from __future__ import annotations

from typing import Any, Dict, Optional

from mcpstore.native.records import _record_value


class Prompt:
    """A prompt identified within one Rust-owned scope and service."""

    def __init__(self, native: Any):
        self._native = native

    def info(self) -> Dict[str, Any]:
        return _record_value(self._native.info())

    def get(self, arguments: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return _record_value(self._native.get({} if arguments is None else arguments))

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


__all__ = ["Prompt"]
