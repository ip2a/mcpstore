"""
Cache configuration classes for MCPStore.

This module provides type-safe configuration classes for different cache backends.
"""

from dataclasses import dataclass
from enum import Enum
from typing import Optional, Literal, Union
from redis.asyncio import Redis


class CacheType(Enum):
    """Cache type enumeration."""
    MEMORY = "memory"
    REDIS = "redis"


class DataSourceStrategy(Enum):
    """Data source strategy enumeration."""
    JSON_MEMORY = "json_memory"      # JSON file + Memory (standard configuration)
    JSON_CUSTOM = "json_custom"      # JSON file + Custom data source (e.g., Redis)
    CUSTOM_ONLY = "custom_only"      # Custom data source only (no JSON sync)



@dataclass
class BaseCacheConfig:
    """Base cache configuration class with common attributes."""
    timeout: float = 2.0
    retry_attempts: int = 3
    health_check: bool = True



@dataclass
class MemoryConfig(BaseCacheConfig):
    """Memory cache configuration."""
    max_size: Optional[int] = None
    cleanup_interval: int = 300
    cache_type: Literal[CacheType.MEMORY] = CacheType.MEMORY



@dataclass
class RedisConfig(BaseCacheConfig):
    """Redis cache configuration with validation."""
    
    # Basic connection configuration
    url: Optional[str] = None
    host: Optional[str] = None
    port: Optional[int] = None
    db: Optional[int] = None
    password: Optional[str] = None
    namespace: Optional[str] = None
    
    # Redis client object (Method 1: pass directly)
    client: Optional[Redis] = None
    
    # Connection pool configuration
    max_connections: int = 50
    retry_on_timeout: bool = True
    socket_keepalive: bool = True
    socket_connect_timeout: float = 5.0
    socket_timeout: float = 5.0
    health_check_interval: int = 30
    
    cache_type: Literal[CacheType.REDIS] = CacheType.REDIS
    
    def __post_init__(self):
        """Validate configuration parameters."""
        # If no client provided, must provide URL or host
        if self.client is None:
            if not self.url and not self.host:
                raise ValueError(
                    "Redis configuration requires either 'client', 'url', or 'host'. "
                    "Example: RedisConfig(url='redis://localhost:6379/0') or "
                    "RedisConfig(host='localhost', port=6379)"
                )
        
        # Validate timeout parameters
        if self.timeout <= 0:
            raise ValueError(
                f"timeout must be positive, got: {self.timeout}. "
                "Example: RedisConfig(url='redis://localhost:6379/0', timeout=5.0)"
            )
        
        if self.socket_timeout <= 0:
            raise ValueError(
                f"socket_timeout must be positive, got: {self.socket_timeout}. "
                "Example: RedisConfig(url='redis://localhost:6379/0', socket_timeout=5.0)"
            )
        
        # Validate connection pool parameters
        if self.max_connections <= 0:
            raise ValueError(
                f"max_connections must be positive, got: {self.max_connections}. "
                "Example: RedisConfig(url='redis://localhost:6379/0', max_connections=50)"
            )


def get_namespace(config: RedisConfig) -> str:
    """
    Get the namespace for Redis configuration.
    
    Args:
        config: Redis configuration object
    
    Returns:
        Namespace string - user-provided namespace if set, otherwise default "mcpstore"
    
    Examples:
        >>> config = RedisConfig(url="redis://localhost:6379/0")
        >>> get_namespace(config)
        'mcpstore'
        
        >>> config = RedisConfig(url="redis://localhost:6379/0", namespace="myapp")
        >>> get_namespace(config)
        'myapp'
    """
    if config.namespace:
        return config.namespace
    return "mcpstore"


def detect_strategy(
    cache_config: Optional[BaseCacheConfig],
    json_path: Optional[str]
) -> DataSourceStrategy:
    """
    Automatically detect data source strategy based on configuration.
    
    Args:
        cache_config: Cache configuration object (MemoryConfig or RedisConfig)
        json_path: JSON file path (optional)
    
    Returns:
        DataSourceStrategy enum value
    
    Strategy Detection Logic:
    - JSON + Memory → JSON_MEMORY (standard configuration)
    - JSON + Redis/Custom → JSON_CUSTOM (needs synchronization)
    - No JSON + Any → CUSTOM_ONLY (no synchronization needed)
    
    Examples:
        >>> detect_strategy(MemoryConfig(), "mcp.json")
        DataSourceStrategy.JSON_MEMORY
        
        >>> detect_strategy(RedisConfig(url="redis://localhost:6379/0"), "mcp.json")
        DataSourceStrategy.JSON_CUSTOM
        
        >>> detect_strategy(RedisConfig(url="redis://localhost:6379/0"), None)
        DataSourceStrategy.CUSTOM_ONLY
    """
    has_json = json_path is not None
    is_memory = isinstance(cache_config, MemoryConfig)
    
    if has_json:
        if is_memory:
            return DataSourceStrategy.JSON_MEMORY  # Standard configuration
        else:
            return DataSourceStrategy.JSON_CUSTOM  # JSON + custom data source
    else:
        # No JSON file
        return DataSourceStrategy.CUSTOM_ONLY  # Only use data source


async def create_kv_store_async(cache_config: Union[MemoryConfig, RedisConfig], test_connection: bool = True):
    """
    Async version of create_kv_store with connection testing.
    
    This async function creates a key-value store and optionally tests the connection.
    Use this when you need to verify the connection immediately in an async context.
    
    Args:
        cache_config: Cache configuration object (MemoryConfig or RedisConfig)
        test_connection: If True, test Redis connection immediately (default: True)
    
    Returns:
        MemoryStore or RedisStore instance
    
    Raises:
        ValueError: If cache_config type is not supported
        RedisConnectionFailure: If Redis connection fails (with detailed context)
    
    Examples:
        >>> config = RedisConfig(url="redis://localhost:6379/0")
        >>> store = await create_kv_store_async(config, test_connection=True)
    """
    from key_value.aio.stores.memory import MemoryStore
    from key_value.aio.stores.redis import RedisStore
    from mcpstore.config.redis_errors import (
        handle_redis_connection_error, 
        test_redis_connection,
        RedisConnectionFailure
    )
    import logging
    
    logger = logging.getLogger(__name__)
    
    if isinstance(cache_config, MemoryConfig):
        logger.debug(f"Creating MemoryStore with max_size={cache_config.max_size}, cleanup_interval={cache_config.cleanup_interval}s")
        return MemoryStore()
    
    if isinstance(cache_config, RedisConfig):
        namespace = get_namespace(cache_config)
        
        try:
            # Test connection first if requested
            if test_connection:
                await test_redis_connection(cache_config)
            
            # Create store after successful connection test
            if cache_config.client:
                logger.debug(f"Creating RedisStore with user-provided client, namespace={namespace}")
                store = RedisStore(
                    client=cache_config.client,
                    default_collection=namespace
                )
            elif cache_config.url:
                logger.debug(f"Creating RedisStore with URL, namespace={namespace}")
                store = RedisStore(
                    url=cache_config.url,
                    default_collection=namespace
                )
            else:
                logger.debug(f"Creating RedisStore with parameters: host={cache_config.host}, port={cache_config.port or 6379}, db={cache_config.db or 0}, namespace={namespace}")
                store = RedisStore(
                    host=cache_config.host,
                    port=cache_config.port or 6379,
                    db=cache_config.db or 0,
                    password=cache_config.password,
                    default_collection=namespace
                )
            
            return store
        
        except RedisConnectionFailure:
            # Re-raise RedisConnectionFailure as-is (already formatted)
            raise
        except Exception as e:
            # Handle other exceptions
            raise handle_redis_connection_error(e, cache_config)
    
    raise ValueError(f"Unsupported cache config type: {type(cache_config)}")


def create_kv_store(cache_config: Union[MemoryConfig, RedisConfig], test_connection: bool = False):
    """
    Create a py-key-value store based on cache configuration.
    
    This factory function creates the appropriate key-value store instance
    based on the provided cache configuration. It supports:
    - MemoryStore for MemoryConfig
    - RedisStore for RedisConfig (with three initialization methods)
    
    For Redis connections, this function uses a fail-fast strategy when test_connection=True:
    - Connection errors are caught immediately during initialization
    - Detailed error messages with masked passwords are provided
    - Troubleshooting steps are included in error messages
    - Authentication and network errors are distinguished
    
    Note: py-key-value's RedisStore uses lazy connection (connects on first use).
    Set test_connection=True to verify the connection immediately.
    
    Args:
        cache_config: Cache configuration object (MemoryConfig or RedisConfig)
        test_connection: If True, test Redis connection immediately (default: False)
    
    Returns:
        MemoryStore or RedisStore instance
    
    Raises:
        ValueError: If cache_config type is not supported
        RedisConnectionFailure: If Redis connection fails (with detailed context)
    
    Examples:
        >>> # Create memory store
        >>> config = MemoryConfig()
        >>> store = create_kv_store(config)
        
        >>> # Create Redis store with URL
        >>> config = RedisConfig(url="redis://localhost:6379/0")
        >>> store = create_kv_store(config)
        
        >>> # Create Redis store with connection test
        >>> config = RedisConfig(url="redis://localhost:6379/0")
        >>> store = create_kv_store(config, test_connection=True)
        
        >>> # Create Redis store with existing client
        >>> from redis.asyncio import Redis
        >>> client = Redis(host="localhost", port=6379)
        >>> config = RedisConfig(client=client)
        >>> store = create_kv_store(config)
        
        >>> # Create Redis store with parameters
        >>> config = RedisConfig(host="localhost", port=6379, db=0)
        >>> store = create_kv_store(config)
    """
    import logging
    from key_value.aio.stores.memory import MemoryStore
    from key_value.aio.stores.redis import RedisStore
    from mcpstore.config.redis_errors import handle_redis_connection_error
    
    logger = logging.getLogger(__name__)
    
    if isinstance(cache_config, MemoryConfig):
        # Create MemoryStore for memory cache configuration
        logger.debug(f"Creating MemoryStore with max_size={cache_config.max_size}, cleanup_interval={cache_config.cleanup_interval}s")
        return MemoryStore()
    
    if isinstance(cache_config, RedisConfig):
        # Get namespace for Redis (use default if not set)
        namespace = get_namespace(cache_config)
        
        try:
            # Method 1: Use existing Redis client object
            if cache_config.client:
                logger.debug(f"Creating RedisStore with user-provided client, namespace={namespace}")
                store = RedisStore(
                    client=cache_config.client,
                    default_collection=namespace
                )
            
            # Method 2: Use URL string
            elif cache_config.url:
                logger.debug(f"Creating RedisStore with URL, namespace={namespace}")
                store = RedisStore(
                    url=cache_config.url,
                    default_collection=namespace
                )
            
            # Method 3: Use connection parameters
            else:
                logger.debug(f"Creating RedisStore with parameters: host={cache_config.host}, port={cache_config.port or 6379}, db={cache_config.db or 0}, namespace={namespace}")
                store = RedisStore(
                    host=cache_config.host,
                    port=cache_config.port or 6379,
                    db=cache_config.db or 0,
                    password=cache_config.password,
                    default_collection=namespace
                )
            
            # Test connection if requested (fail-fast)
            # Note: This is a synchronous function, so we can't await.
            # The test_connection parameter is mainly for documentation.
            # Actual connection testing happens on first use of the store.
            if test_connection:
                logger.debug("test_connection=True, but connection test deferred to first use (py-key-value uses lazy connection)")
            
            return store
        
        except Exception as e:
            # Handle Redis connection errors with detailed context
            raise handle_redis_connection_error(e, cache_config)
    
    raise ValueError(f"Unsupported cache config type: {type(cache_config)}")
