"""
Cache configuration classes for MCPStore.

This module provides type-safe configuration classes for different cache backends.
Non-sensitive configuration is loaded from MCPStoreConfig, sensitive configuration from environment variables.
"""

from dataclasses import dataclass
from enum import Enum
from typing import TYPE_CHECKING, Any, Optional, Literal

if TYPE_CHECKING:
    from redis.asyncio import Redis
else:
    Redis = Any


class CacheType(Enum):
    """Cache type enumeration."""
    MEMORY = "memory"
    REDIS = "redis"
    OPENKEYV_MEMORY = "openkeyv_memory"
    OPENKEYV_REDIS = "openkeyv_redis"


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
class OpenKeyvMemoryConfig(MemoryConfig):
    """openkeyv memory cache configuration for Rust core."""
    cache_type: Literal[CacheType.OPENKEYV_MEMORY] = CacheType.OPENKEYV_MEMORY



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

    # Allow partial configuration for testing/default scenarios
    allow_partial: bool = False

    cache_type: Literal[CacheType.REDIS] = CacheType.REDIS

    def __post_init__(self):
        """Validate configuration parameters."""
        # If no client provided, must provide URL or host (unless partial allowed)
        if self.client is None and not self.allow_partial:
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
