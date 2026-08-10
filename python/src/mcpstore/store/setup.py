"""Python-only setup normalization around the Rust store constructor."""

from __future__ import annotations

import importlib
import os
from typing import Any, Dict, Optional
from urllib.parse import quote


def normalize_cache_config(cache_config: Any) -> Any:
    if cache_config is None or hasattr(cache_config, "store_name"):
        return cache_config
    if isinstance(cache_config, str):
        from mcpstore.config import StoreConfig

        return StoreConfig(store=cache_config)
    if isinstance(cache_config, dict):
        store = str(cache_config.get("store", "memory")).strip().lower()
        options = dict(cache_config)
        options.pop("store", None)
        from mcpstore.config import StoreConfig
        return StoreConfig(store=store, config=options)
    return cache_config


def cache_options(cache_config: Any) -> tuple[Optional[str], Optional[dict[str, Any]], Optional[str]]:
    if cache_config is None:
        return None, None, None
    store = getattr(cache_config, "store_name", "memory")
    config = getattr(cache_config, "config", None) or {}
    return store, config, getattr(cache_config, "namespace", None)


def setup_backend(backend_cls: type, config_path: Optional[str], cache_config: Any, only_db: bool):
    rust_mod = importlib.import_module("mcpstore._rust")
    path = os.fspath(config_path) if config_path is not None else None
    cache = normalize_cache_config(cache_config)
    store, config, namespace = cache_options(cache)
    rust_store = rust_mod.MCPStore.setup_with_options(path, "db" if only_db else "local", store, namespace)
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
            store_name = getattr(cache, "store_name", None)
            if not store_name or str(store_name).lower() == "memory":
                raise ValueError(
                    "cache_mode='shared' requires a Store that is shared across processes"
                )

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
