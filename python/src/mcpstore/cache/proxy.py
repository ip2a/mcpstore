"""Python interface for store cache inspection."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict

if TYPE_CHECKING:
    from mcpstore.store.store import RustStoreBackend

class RustCacheProxy:
    def __init__(self, backend: RustStoreBackend):
        self._backend = backend

    def health_check(self) -> Dict[str, Any]:
        return self._backend.cache_health_check()

    def inspect(self) -> Dict[str, Any]:
        return self._backend.cache_inspect()

    def reset_request_metrics(self) -> None:
        self._backend.reset_cache_request_metrics()


CacheProxy = RustCacheProxy

__all__ = ["CacheProxy", "RustCacheProxy"]
