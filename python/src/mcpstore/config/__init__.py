"""配置模块懒加载导出。"""

from __future__ import annotations


def __getattr__(name: str):
    if name in {
        "CacheType",
        "DataSourceStrategy",
        "BaseCacheConfig",
        "MemoryConfig",
        "RedisConfig",
        "OpenKeyvMemoryConfig",
        "get_namespace",
        "detect_strategy",
        "create_kv_store",
        "create_kv_store_async",
    }:
        from . import cache_config as _cache_config

        value = getattr(_cache_config, name)
        globals()[name] = value
        return value

    if name in {"LoggingConfig", "load_app_config"}:
        from . import config as _config

        value = getattr(_config, name)
        globals()[name] = value
        return value

    raise AttributeError(f"module '{__name__}' has no attribute '{name}'")


__all__ = [
    "LoggingConfig",
    "load_app_config",
    "CacheType",
    "DataSourceStrategy",
    "BaseCacheConfig",
    "MemoryConfig",
    "RedisConfig",
    "OpenKeyvMemoryConfig",
    "get_namespace",
    "detect_strategy",
    "create_kv_store",
    "create_kv_store_async",
]
