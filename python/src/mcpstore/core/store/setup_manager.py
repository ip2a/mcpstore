"""
Rust-only setup manager for MCPStore.

Python SDK initialization now has a single authoritative core: the Rust core
exposed by ``mcpstore._rust``.
"""

import asyncio
import os
from typing import Any, Dict, Optional, Union


class StoreSetupManager:
    """Setup manager for the single Rust-backed MCPStore entry point."""

    @staticmethod
    def setup_store(
        mcpjson_path: str | None = None,
        debug: bool | str = False,
        cache: Optional[Union["MemoryConfig", "RedisConfig"]] = None,
        static_config: Optional[Dict[str, Any]] = None,
        cache_mode: str = "auto",
        only_db: bool = False,
        **kwargs: Any,
    ):
        """Initialize MCPStore synchronously with the Rust core."""
        try:
            asyncio.get_running_loop()
        except RuntimeError:
            pass
        else:
            raise RuntimeError("检测到正在运行的事件循环：请使用 setup_store_async() 接口。")

        return asyncio.run(
            StoreSetupManager._setup_store_internal(
                mcpjson_path=mcpjson_path,
                debug=debug,
                cache=cache,
                static_config=static_config,
                cache_mode=cache_mode,
                only_db=only_db,
                extra_options=kwargs,
            )
        )

    @staticmethod
    async def setup_store_async(
        mcpjson_path: str | None = None,
        debug: bool | str = False,
        cache: Optional[Union["MemoryConfig", "RedisConfig"]] = None,
        static_config: Optional[Dict[str, Any]] = None,
        cache_mode: str = "auto",
        only_db: bool = False,
        **kwargs: Any,
    ):
        """Initialize MCPStore asynchronously with the Rust core."""
        return await StoreSetupManager._setup_store_internal(
            mcpjson_path=mcpjson_path,
            debug=debug,
            cache=cache,
            static_config=static_config,
            cache_mode=cache_mode,
            only_db=only_db,
            extra_options=kwargs,
        )

    @staticmethod
    async def _setup_store_internal(
        mcpjson_path: str | None,
        debug: bool | str,
        cache: Optional[Union["MemoryConfig", "RedisConfig"]],
        static_config: Optional[Dict[str, Any]],
        cache_mode: str,
        only_db: bool,
        extra_options: Dict[str, Any],
    ):
        from mcpstore.config.config import LoggingConfig

        LoggingConfig.setup_logging(debug=debug)

        if extra_options:
            unsupported = ", ".join(sorted(extra_options))
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
            store.add_service(static_config)
        return store

    @staticmethod
    def _normalize_cache_options(
        cache: Optional[Union["MemoryConfig", "RedisConfig"]],
        cache_mode: str,
        only_db: bool,
    ):
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
        cache: Optional[Union["MemoryConfig", "RedisConfig"]],
        only_db: bool,
    ):
        from mcpstore.core.store.rust_backend import MCPStore

        return MCPStore.setup(
            config_path=mcpjson_path,
            cache_config=cache,
            only_db=only_db,
        )
