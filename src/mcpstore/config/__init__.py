"""
Configuration module
"""

# Direct import of original config module
from .config import LoggingConfig, load_app_config

# Import cache configuration classes
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

# Import health check functionality
from .health_check import (
    RedisHealthCheck,
    start_health_check
)

# Import error handling
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
