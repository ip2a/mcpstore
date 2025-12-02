"""
Configuration module
"""

# Direct import of original config module
from .config import LoggingConfig, load_app_config

# Import cache configuration classes (optional, with fallback)
try:
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
    _cache_config_available = True
except ImportError as e:
    print(f"Warning: Cache configuration not available: {e}")
    _cache_config_available = False
    # Create placeholder classes
    CacheType = None
    DataSourceStrategy = None
    BaseCacheConfig = None
    MemoryConfig = None
    RedisConfig = None
    def get_namespace(*args, **kwargs): pass
    def detect_strategy(*args, **kwargs): return None
    def create_kv_store(*args, **kwargs): return None
    def create_kv_store_async(*args, **kwargs): return None

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

# Import TOML configuration management
from .toml_config import (
    initialize_config_system,
    ensure_config_directory,
    create_default_config_if_not_exists,
    get_user_config_path,
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
    'test_redis_connection',
    'initialize_config_system',
    'ensure_config_directory',
    'create_default_config_if_not_exists',
    'get_user_config_path',
]
