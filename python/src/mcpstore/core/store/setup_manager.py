"""
Rust-only setup manager for MCPStore.

Python SDK initialization now has a single authoritative core: the Rust core
exposed by ``mcpstore._rust``.
"""

import asyncio
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
    ):
        """Initialize MCPStore asynchronously with the Rust core."""
        return await StoreSetupManager._setup_store_internal(
            mcpjson_path=mcpjson_path,
            debug=debug,
            cache=cache,
            static_config=static_config,
            cache_mode=cache_mode,
            only_db=only_db,
        )

    @staticmethod
    async def _setup_store_internal(
        mcpjson_path: str | None,
        debug: bool | str,
        cache: Optional[Union["MemoryConfig", "RedisConfig"]],
        static_config: Optional[Dict[str, Any]],
        cache_mode: str,
        only_db: bool,
    ):
        from mcpstore.config.config import LoggingConfig

        LoggingConfig.setup_logging(debug=debug)

        if static_config:
            raise ValueError("Rust core 当前不支持 static_config，请改用 Rust core 已暴露的显式 API。")
        if cache_mode != "auto":
            raise ValueError("Rust core 当前不支持 cache_mode，请使用 only_db 和 cache 配置表达数据源。")

        return StoreSetupManager._setup_rust_store(
            mcpjson_path=mcpjson_path,
            debug=debug,
            cache=cache,
            only_db=only_db,
        )

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
