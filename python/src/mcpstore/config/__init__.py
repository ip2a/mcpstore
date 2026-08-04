"""配置模块懒加载导出。"""

from __future__ import annotations


def __getattr__(name: str):
    if name in {
        "CacheType",
        "MemoryConfig",
        "OpenKeyvConfig",
        "RedisConfig",
        "get_namespace",
    }:
        from . import cache_config as _cache_config

        value = getattr(_cache_config, name)
        globals()[name] = value
        return value

    if name in {"LoggingConfig"}:
        from . import config as _config

        value = getattr(_config, name)
        globals()[name] = value
        return value

    raise AttributeError(f"module '{__name__}' has no attribute '{name}'")


__all__ = [
    "LoggingConfig",
    "CacheType",
    "MemoryConfig",
    "OpenKeyvConfig",
    "RedisConfig",
    "get_namespace",
]
