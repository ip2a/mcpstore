"""Rust-backed compatibility facade for async-safe service management."""

from __future__ import annotations

import time
from typing import Any, Dict, Optional, Tuple

from .service_management import ServiceManagementMixin


class AsyncSafeServiceManagement(ServiceManagementMixin):
    """Historical async-safe service API backed by RustStoreContext.

    The old Python implementation avoided nested async calls by reading from
    Python registries first. Rust now owns the service state, so this facade
    keeps only the public cache behavior and delegates all service reads and
    writes to the Rust-backed context.
    """

    def __init__(self, context: Any = None, *, cache_timeout: float = 5.0):
        if context is not None:
            self._context = context
        self._service_info_cache: Dict[str, Tuple[float, Any]] = {}
        self._cache_timeout = float(cache_timeout)

    def get_service_info(self, name: str, use_cache: bool = True):
        if use_cache:
            cached_info = self._get_from_cache(name)
            if cached_info is not None:
                return cached_info

        service_info = ServiceManagementMixin.get_service_info(self, name)
        self._update_cache(name, service_info)
        return service_info

    def service_info(self, name: str, use_cache: bool = True):
        return self.get_service_info(name, use_cache=use_cache)

    async def get_service_info_async(self, name: str, use_cache: bool = True):
        return self.get_service_info(name, use_cache=use_cache)

    async def service_info_async(self, name: str, use_cache: bool = True):
        return self.get_service_info(name, use_cache=use_cache)

    def clear_cache(self, name: Optional[str] = None):
        if name:
            self._service_info_cache.pop(name, None)
        else:
            self._service_info_cache.clear()

    def get_cache_stats(self) -> Dict[str, Any]:
        current_time = time.time()
        stats = {
            "total_cached": len(self._service_info_cache),
            "valid_entries": 0,
            "expired_entries": 0,
            "entries": [],
        }

        for name, (cached_time, info) in self._service_info_cache.items():
            age = current_time - cached_time
            is_valid = age <= self._cache_timeout
            if is_valid:
                stats["valid_entries"] += 1
            else:
                stats["expired_entries"] += 1
            stats["entries"].append(
                {
                    "name": name,
                    "age": age,
                    "is_valid": is_valid,
                    "tools_count": self._tools_count(info),
                }
            )

        return stats

    def _get_from_cache(self, name: str):
        cache_entry = self._service_info_cache.get(name)
        if cache_entry is None:
            return None

        cached_time, cached_info = cache_entry
        if time.time() - cached_time > self._cache_timeout:
            self._service_info_cache.pop(name, None)
            return None
        return cached_info

    def _update_cache(self, name: str, info: Any):
        self._service_info_cache[name] = (time.time(), info)

    @staticmethod
    def _tools_count(info: Any) -> int:
        if isinstance(info, dict):
            if "tools_count" in info:
                return int(info.get("tools_count") or 0)
            tools = info.get("tools")
            return len(tools) if isinstance(tools, list) else 0

        tools_count = getattr(info, "tools_count", None)
        if tools_count is not None:
            return int(tools_count or 0)
        tools = getattr(info, "tools", None)
        return len(tools) if isinstance(tools, list) else 0


class AsyncSafeServiceManagementFactory:
    @staticmethod
    def create_service_management(*args, **kwargs):
        return AsyncSafeServiceManagement(*args, **kwargs)

    @staticmethod
    def migrate_from_standard_management(standard_management):
        context = getattr(standard_management, "_context", standard_management)
        migrated = AsyncSafeServiceManagement(context)
        migrated._service_info_cache.update(
            getattr(standard_management, "_service_info_cache", {})
        )
        cache_timeout = getattr(standard_management, "_cache_timeout", None)
        if cache_timeout is not None:
            migrated._cache_timeout = float(cache_timeout)
        return migrated


__all__ = ["AsyncSafeServiceManagement", "AsyncSafeServiceManagementFactory"]
