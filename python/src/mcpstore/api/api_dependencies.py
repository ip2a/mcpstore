"""Shared store dependency for custom FastAPI applications."""

from __future__ import annotations

from typing import Any, Optional


_store: Optional[Any] = None


def set_store(store: Any) -> None:
    global _store
    _store = store


def get_store() -> Any:
    if _store is None:
        raise RuntimeError("MCPStore has not been configured. Call api_set_store(store) first.")
    return _store
