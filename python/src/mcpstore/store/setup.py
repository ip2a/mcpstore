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


class StoreSetupManager:
    """Setup manager for the single Rust-backed MCPStore entry point."""

    @staticmethod
    def setup_store(
        mcpjson_path: str | None = None,
        debug: bool | str = False,
        cache: Any = None,
        static_config: Optional[Dict[str, Any]] = None,
        cache_mode: str = "auto",
        only_db: bool = False,
        **kwargs: Any,
    ):
        """Initialize MCPStore synchronously with the Rust core."""
        from mcpstore.config.config import LoggingConfig

        LoggingConfig.setup_logging(debug=debug)

        mcpjson_path, cache = StoreSetupManager._apply_setup_aliases(
            mcpjson_path=mcpjson_path,
            cache=cache,
            extra_options=kwargs,
        )

        if kwargs:
            unsupported = ", ".join(sorted(kwargs))
            raise ValueError(f"Rust core 当前不支持 setup_store 参数: {unsupported}")
        config_path = StoreSetupManager._normalize_path(mcpjson_path)
        resolved_cache, resolved_only_db = StoreSetupManager._normalize_cache_options(
            cache=cache,
            cache_mode=cache_mode,
            only_db=only_db,
        )

        store = StoreSetupManager._setup_rust_store(
            mcpjson_path=config_path,
            debug=debug,
            cache=resolved_cache,
            only_db=resolved_only_db,
        )
        if static_config:
            StoreSetupManager._add_static_config(store, static_config)
        return store

    @staticmethod
    def _add_static_config(store: Any, static_config: Dict[str, Any]) -> None:
        services = static_config.get("mcpServers")
        if not isinstance(services, dict):
            raise ValueError("static_config must contain an 'mcpServers' object")
        for service_name, config in services.items():
            if not isinstance(service_name, str) or not service_name:
                raise ValueError("static_config service names must be non-empty strings")
            if not isinstance(config, dict):
                raise ValueError(
                    f"static_config service {service_name!r} must be an object"
                )
            store.add_service(service_name, config)

    @staticmethod
    def _apply_setup_aliases(
        mcpjson_path: Any,
        cache: Any,
        extra_options: Dict[str, Any],
    ):
        path_aliases = [name for name in ("config_path", "mcp_config_file") if name in extra_options]
        if path_aliases:
            if mcpjson_path is not None:
                raise ValueError("setup_store 参数冲突: mcpjson_path 不能和 config_path/mcp_config_file 同时使用")
            if len(path_aliases) > 1:
                raise ValueError("setup_store 参数冲突: config_path 和 mcp_config_file 只能使用一个")
            mcpjson_path = extra_options.pop(path_aliases[0])

        if "cache_config" in extra_options:
            if cache is not None:
                raise ValueError("setup_store 参数冲突: cache 不能和 cache_config 同时使用")
            cache = extra_options.pop("cache_config")

        return mcpjson_path, cache

    @staticmethod
    def _normalize_cache_options(
        cache: Any,
        cache_mode: str,
        only_db: bool,
    ):
        from mcpstore.store.store import RustStoreBackend

        cache = RustStoreBackend._normalize_cache_config(cache)
        mode = (cache_mode or "auto").lower()
        if mode not in {"auto", "local", "shared"}:
            raise ValueError(f"Rust core 当前不支持 cache_mode={cache_mode!r}")

        if mode == "shared":
            cache_type_value = getattr(cache, "cache_type", None)
            cache_type = getattr(cache_type_value, "value", cache_type_value)
            if cache_type not in {"redis", "openkeyv_redis"}:
                raise ValueError("cache_mode='shared' 需要 RedisConfig；memory 后端无法跨进程共享 session")

        resolved_only_db = False if mode == "local" else only_db or mode == "shared"
        if mode == "local" and cache is None:
            return None, resolved_only_db
        return cache, resolved_only_db

    @staticmethod
    def _normalize_path(path: Any) -> str | None:
        if path is None:
            return None
        return os.fspath(path)

    @staticmethod
    def _setup_rust_store(
        mcpjson_path: str | None,
        debug: bool | str,
        cache: Any,
        only_db: bool,
    ):
        from mcpstore.store.store import MCPStore

        return MCPStore.setup(
            config_path=mcpjson_path,
            cache_config=cache,
            only_db=only_db,
        )
