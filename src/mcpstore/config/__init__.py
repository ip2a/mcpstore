"""
配置模块
"""

# 直接导入原始的config模块
from .config import LoggingConfig, load_app_config

# 导入缓存配置类
from .cache_config import (
    CacheType,
    DataSourceStrategy,
    BaseCacheConfig,
    MemoryConfig,
    RedisConfig,
    get_namespace,
    detect_strategy,
    create_kv_store,
    create_kv_store_async
)

# 导入健康检查功能
from .health_check import (
    RedisHealthCheck,
    start_health_check
)

# 导入错误处理
from .redis_errors import (
    RedisConnectionFailure,
    mask_password_in_url,
    get_connection_info,
    handle_redis_connection_error,
    test_redis_connection
)

__all__ = [
    'LoggingConfig',
    'load_app_config',
    'CacheType',
    'DataSourceStrategy',
    'BaseCacheConfig',
    'MemoryConfig',
    'RedisConfig',
    'get_namespace',
    'detect_strategy',
    'create_kv_store',
    'create_kv_store_async',
    'RedisHealthCheck',
    'start_health_check',
    'RedisConnectionFailure',
    'mask_password_in_url',
    'get_connection_info',
    'handle_redis_connection_error',
    'test_redis_connection'
]
