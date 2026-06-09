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
        external_db: Optional[Dict[str, Any]] = None,
        static_config: Optional[Dict[str, Any]] = None,
        cache_mode: str = "auto",
        only_db: bool = False,
        mcp_config_file: str | None = None,
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
                external_db=external_db,
                static_config=static_config,
                cache_mode=cache_mode,
                only_db=only_db,
                mcp_config_file=mcp_config_file,
                extra_options=kwargs,
            )
        )

    @staticmethod
    async def setup_store_async(
        mcpjson_path: str | None = None,
        debug: bool | str = False,
        cache: Optional[Union["MemoryConfig", "RedisConfig"]] = None,
        external_db: Optional[Dict[str, Any]] = None,
        static_config: Optional[Dict[str, Any]] = None,
        cache_mode: str = "auto",
        only_db: bool = False,
        mcp_config_file: str | None = None,
        **kwargs: Any,
    ):
        """Initialize MCPStore asynchronously with the Rust core."""
        return await StoreSetupManager._setup_store_internal(
            mcpjson_path=mcpjson_path,
            debug=debug,
            cache=cache,
            external_db=external_db,
            static_config=static_config,
            cache_mode=cache_mode,
            only_db=only_db,
            mcp_config_file=mcp_config_file,
            extra_options=kwargs,
        )

    @staticmethod
    async def _setup_store_internal(
        mcpjson_path: str | None,
        debug: bool | str,
        cache: Optional[Union["MemoryConfig", "RedisConfig"]],
        external_db: Optional[Dict[str, Any]],
        static_config: Optional[Dict[str, Any]],
        cache_mode: str,
        only_db: bool,
        mcp_config_file: str | None,
        extra_options: Dict[str, Any],
    ):
        from mcpstore.config.config import LoggingConfig

        LoggingConfig.setup_logging(debug=debug)

        if "config_path" in extra_options:
            if mcpjson_path or mcp_config_file:
                raise ValueError("config_path 不能与 mcpjson_path/mcp_config_file 同时传入")
            mcpjson_path = extra_options.pop("config_path")
        if "cache_config" in extra_options:
            if cache is not None:
                raise ValueError("cache_config 不能与 cache 同时传入")
            cache = extra_options.pop("cache_config")

        if extra_options:
            unsupported = ", ".join(sorted(extra_options))
            raise ValueError(f"Rust core 当前不支持 setup_store 参数: {unsupported}")
        if static_config:
            raise ValueError("Rust core 当前不支持 static_config，请改用 Rust core 已暴露的显式 API。")

        config_path = StoreSetupManager._normalize_path(mcpjson_path or mcp_config_file)
        resolved_cache, resolved_only_db = StoreSetupManager._normalize_cache_options(
            cache=cache,
            external_db=external_db,
            cache_mode=cache_mode,
            only_db=only_db,
        )

        return StoreSetupManager._setup_rust_store(
            mcpjson_path=config_path,
            debug=debug,
            cache=resolved_cache,
            only_db=resolved_only_db,
        )

    @staticmethod
    def _normalize_cache_options(
        cache: Optional[Union["MemoryConfig", "RedisConfig"]],
        external_db: Optional[Dict[str, Any]],
        cache_mode: str,
        only_db: bool,
    ):
        mode = (cache_mode or "auto").lower()
        if mode not in {"auto", "local", "hybrid", "shared"}:
            raise ValueError(f"Rust core 当前不支持 cache_mode={cache_mode!r}")

        resolved_cache = cache
        if external_db:
            if cache is not None:
                raise ValueError("cache 和 external_db 不能同时传入；请只使用一种缓存配置方式。")
            resolved_cache = StoreSetupManager._cache_from_external_db(external_db)

        resolved_only_db = False if mode == "local" else only_db or mode == "shared"
        if mode == "local" and resolved_cache is None:
            return None, resolved_only_db
        return resolved_cache, resolved_only_db

    @staticmethod
    def _cache_from_external_db(external_db: Dict[str, Any]):
        if not isinstance(external_db, dict):
            raise ValueError("external_db 必须是 dict")

        cache_config = external_db.get("cache", external_db)
        if not isinstance(cache_config, dict):
            raise ValueError("external_db.cache 必须是 dict")

        cache_type = str(cache_config.get("type", "memory")).lower()
        if cache_type in {"memory", "openkeyv_memory"}:
            from mcpstore.config import MemoryConfig, OpenKeyvMemoryConfig

            cls = OpenKeyvMemoryConfig if cache_type == "openkeyv_memory" else MemoryConfig
            return cls(
                timeout=cache_config.get("timeout", 2.0),
                retry_attempts=cache_config.get("retry_attempts", 3),
                health_check=cache_config.get("health_check", True),
                max_size=cache_config.get("max_size"),
                cleanup_interval=cache_config.get("cleanup_interval", 300),
            )

        if cache_type in {"redis", "openkeyv_redis"}:
            from mcpstore.config import RedisConfig

            redis_config = RedisConfig(
                url=cache_config.get("url"),
                host=cache_config.get("host"),
                port=cache_config.get("port"),
                db=cache_config.get("db"),
                password=cache_config.get("password"),
                namespace=cache_config.get("namespace"),
                max_connections=cache_config.get("max_connections", 50),
                retry_on_timeout=cache_config.get("retry_on_timeout", True),
                socket_keepalive=cache_config.get("socket_keepalive", True),
                socket_connect_timeout=cache_config.get("socket_connect_timeout", 5.0),
                socket_timeout=cache_config.get("socket_timeout", 5.0),
                health_check_interval=cache_config.get("health_check_interval", 30),
                allow_partial=bool(cache_config.get("allow_partial", False)),
                timeout=cache_config.get("timeout", 2.0),
                retry_attempts=cache_config.get("retry_attempts", 3),
                health_check=cache_config.get("health_check", True),
            )
            if cache_type == "openkeyv_redis":
                from mcpstore.config.cache_config import CacheType

                redis_config.cache_type = CacheType.OPENKEYV_REDIS
            return redis_config

        raise ValueError(f"不支持的 external_db.cache.type: {cache_type}")

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
        from mcpstore.core.store.rust_backend import RustStoreBackend

        return RustStoreBackend.setup(
            config_path=mcpjson_path,
            cache_config=cache,
            only_db=only_db,
        )
