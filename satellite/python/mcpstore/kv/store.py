"""Store-level KV and event operations."""

from __future__ import annotations

from typing import Any, Dict, List

from mcpstore.kv.proxy import RustKvProxy
from mcpstore.native.records import _record_value
from mcpstore.store.setup import _config_triplet

def event_history(backend, count: int = 100) -> List[Dict[str, Any]]:
    return _record_value(backend._inner.event_history(count))

def event_capability_report(backend) -> Dict[str, Any]:
    return _record_value(backend._inner.event_capability_report())

def kv_health_check(backend) -> Dict[str, Any]:
    return _record_value(backend._inner.kv_health_check())

def kv_inspect(backend) -> Dict[str, Any]:
    return _record_value(backend._inner.kv_inspect())

def reset_kv_request_metrics(backend) -> None:
    backend._inner.reset_kv_request_metrics()

def find_kv(backend) -> "RustKvProxy":
    return RustKvProxy(backend)

def swap_store(store, source: Any) -> Dict[str, Any]:
    """Swap the runtime Store to a new source config (e.g. RedisConfig)."""
    store_name, store_config_data, _ = _config_triplet(source)
    if store_name is None:
        raise ValueError("source config is required")
    result = store._inner.swap_store(store_name, store_config_data or {})
    return _record_value(result)
