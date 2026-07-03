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
        "OpenKeyvRedisConfig",
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

    if name in {
        "initialize_config_system",
        "ensure_config_directory",
        "create_default_config_if_not_exists",
        "get_user_config_path",
        "get_user_data_dir",
        "get_user_default_mcp_path",
    }:
        from . import path_utils as _path_utils

        value = getattr(_path_utils, name)
        globals()[name] = value
        return value

    if name in {
        "RedisConnectionFailure",
        "mask_password_in_url",
        "get_connection_info",
        "handle_redis_connection_error",
        "test_redis_connection",
    }:
        from . import redis_errors as _redis_errors

        value = getattr(_redis_errors, name)
        globals()[name] = value
        return value

    if name in {"RedisHealthCheck", "start_health_check"}:
        from . import health_check as _health_check

        value = getattr(_health_check, name)
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
    "OpenKeyvRedisConfig",
    "get_namespace",
    "detect_strategy",
    "create_kv_store",
    "create_kv_store_async",
    "RedisHealthCheck",
    "start_health_check",
    "RedisConnectionFailure",
    "mask_password_in_url",
    "get_connection_info",
    "handle_redis_connection_error",
    "test_redis_connection",
    "initialize_config_system",
    "ensure_config_directory",
    "create_default_config_if_not_exists",
    "get_user_config_path",
    "get_user_data_dir",
    "get_user_default_mcp_path",
]
