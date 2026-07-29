"""Python-only setup normalization around the Rust store constructor."""

from __future__ import annotations

import importlib
import os
from typing import Any, Dict, Optional
from urllib.parse import quote


def normalize_cache_config(cache_config: Any) -> Any:
    if cache_config is None or hasattr(cache_config, "cache_type"):
        return cache_config
    if isinstance(cache_config, str):
        cache_type = cache_config.strip().lower()
        if cache_type == "memory":
            from mcpstore.config import MemoryConfig
            return MemoryConfig()
        if cache_type == "openkeyv_memory":
            from mcpstore.config import OpenKeyvMemoryConfig
            return OpenKeyvMemoryConfig()
        raise ValueError(f"Unsupported Rust cache type: {cache_config!r}")
    if isinstance(cache_config, dict):
        raw_type = cache_config.get("type", cache_config.get("cache_type"))
        cache_type = getattr(raw_type, "value", raw_type)
        options = dict(cache_config)
        options.pop("type", None)
        options.pop("cache_type", None)
        configs = {
            "memory": "MemoryConfig",
            "openkeyv_memory": "OpenKeyvMemoryConfig",
            "redis": "RedisConfig",
            "openkeyv_redis": "OpenKeyvRedisConfig",
        }
        config_name = configs.get(cache_type)
        if config_name:
            from mcpstore import config as config_module
            return getattr(config_module, config_name)(**options)
        raise ValueError(f"Unsupported Rust cache type: {cache_type!r}")
    return cache_config


def redis_url(cache_config: Any) -> Optional[str]:
    url = getattr(cache_config, "url", None)
    if url:
        return str(url)
    host = getattr(cache_config, "host", None)
    if not host:
        return None
    port = getattr(cache_config, "port", None) or 6379
    db = getattr(cache_config, "db", None) or 0
    password = getattr(cache_config, "password", None)
    auth = f":{quote(str(password), safe='')}@" if password else ""
    return f"redis://{auth}{host}:{port}/{db}"


def cache_options(cache_config: Any) -> tuple[Optional[str], Optional[str], Optional[str]]:
    if cache_config is None:
        return None, None, None
    raw_type = getattr(cache_config, "cache_type", None)
    cache_type = getattr(raw_type, "value", raw_type)
    backend = {
        "memory": "memory",
        "openkeyv_memory": "openkeyv-memory",
        "redis": "redis",
        "openkeyv_redis": "openkeyv-redis",
    }.get(cache_type)
    if backend is None:
        raise ValueError(f"Unsupported Rust cache type: {cache_type!r}")
    url = redis_url(cache_config) if "redis" in backend else None
    if "redis" in backend and not url:
        raise ValueError("Redis cache configuration requires url or host")
    return backend, url, getattr(cache_config, "namespace", None)


def setup_backend(backend_cls: type, config_path: Optional[str], cache_config: Any, only_db: bool):
    rust_mod = importlib.import_module("mcpstore._rust")
    path = os.fspath(config_path) if config_path is not None else None
    cache = normalize_cache_config(cache_config)
    backend, redis, namespace = cache_options(cache)
    rust_store = rust_mod.MCPStore.setup_with_options(
        path, "db" if only_db else "local", backend, redis, namespace
    )
    store = backend_cls(rust_store)
    store._config_path = path
    store._cache_config = cache
    store._only_db = only_db
    store.load_from_config()
    return store
