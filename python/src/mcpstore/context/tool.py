"""Public Tool resource over the Rust facade."""

from __future__ import annotations

from typing import Any, Dict, Optional

from mcpstore.native.records import _record_value


class Tool:
    """A tool identified within one Rust-owned scope and service."""

    def __init__(self, native: Any):
        self._native = native

    def info(self) -> Dict[str, Any]:
        return _record_value(self._native.info())

    def call(self, args: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return _record_value(self._native.call(args or {}))


__all__ = ["Tool"]
