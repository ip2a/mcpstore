"""Python interface for store KV inspection."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict

if TYPE_CHECKING:
    from mcpstore.store.store import RustStoreBackend

class RustKvProxy:
    def __init__(self, backend: RustStoreBackend):
        self._backend = backend

    def health_check(self) -> Dict[str, Any]:
        return self._backend.kv_health_check()

    def inspect(self) -> Dict[str, Any]:
        return self._backend.kv_inspect()

    def reset_request_metrics(self) -> None:
        self._backend.reset_kv_request_metrics()


KvProxy = RustKvProxy

__all__ = ["KvProxy", "RustKvProxy"]
