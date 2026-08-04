"""Store-level cache and event operations."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from mcpstore.cache.proxy import RustCacheProxy
from mcpstore.native.records import _record_value

def event_history(backend, count: int = 100) -> List[Dict[str, Any]]:
    return _record_value(backend._inner.event_history(count))

def event_capability_report(backend) -> Dict[str, Any]:
    return _record_value(backend._inner.event_capability_report())

def cache_health_check(backend) -> Dict[str, Any]:
    return _record_value(backend._inner.cache_health_check())

def cache_inspect(backend) -> Dict[str, Any]:
    return _record_value(backend._inner.cache_inspect())

def reset_cache_request_metrics(backend) -> None:
    backend._inner.reset_cache_request_metrics()

def find_cache(backend) -> "RustCacheProxy":
    return RustCacheProxy(backend)

def switch_cache(
    backend,
    cache_config: Any,
) -> Dict[str, Any]:
    normalized = backend._normalize_cache_config(cache_config)
    backend_name, backend_url, namespace = backend._cache_options(normalized)
    if backend_name is None:
        raise ValueError("cache_config is required")
    result = backend._inner.switch_cache_storage(backend_name, backend_url, namespace)
    backend._cache_config = normalized
    return _record_value(result)
