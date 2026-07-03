"""
Cache configuration classes for MCPStore.

This module provides type-safe configuration classes for different cache backends.
Non-sensitive configuration is loaded from MCPStoreConfig, sensitive configuration from environment variables.
"""

from dataclasses import dataclass
from enum import Enum
from typing import Any, Optional, Literal


class CacheType(Enum):
    """Cache type enumeration."""
    MEMORY = "memory"
    REDIS = "redis"
    OPENKEYV_MEMORY = "openkeyv_memory"
    OPENKEYV_REDIS = "openkeyv_redis"


class DataSourceStrategy(Enum):
    """Compatibility strategy labels for Python callers.

    The Rust core owns the actual source-mode implementation. These labels keep
    the old Python configuration surface usable for code that only needs to
    inspect or describe setup behavior.
    """

    LOCAL_DB = "local_db"
    ONLY_DB = "only_db"


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

    # Kept for old Python API shape only. Rust-backed stores cannot reuse a
    # Python Redis client object across processes or runtimes.
    client: Optional[Any] = None

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
        if self.client is None and not self.allow_partial and not self.url and not self.host:
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


@dataclass
class OpenKeyvRedisConfig(RedisConfig):
    """openkeyv Redis cache configuration for Rust core."""

    cache_type: Literal[CacheType.OPENKEYV_REDIS] = CacheType.OPENKEYV_REDIS


def get_namespace(config: object, default: str = "mcpstore") -> str:
    """Return the configured cache namespace, or the MCPStore default."""

    return getattr(config, "namespace", None) or default


def detect_strategy(
    cache_config: Optional[BaseCacheConfig],
    json_path: Optional[str],
    *,
    only_db: bool = False,
) -> DataSourceStrategy:
    """Describe the Rust source-mode strategy for compatibility callers.

    The current implementation no longer runs a Python cache wrapper. It maps
    the legacy strategy names onto the Rust setup semantics: normal setup uses
    local config plus the selected cache backend, while ``only_db`` uses only
    the cache backend.
    """

    _ = cache_config, json_path
    if only_db:
        return DataSourceStrategy.ONLY_DB
    return DataSourceStrategy.LOCAL_DB


def create_kv_store(*args, **kwargs):
    """Compatibility placeholder for the removed Python key_value backend."""

    _ = args, kwargs
    raise NotImplementedError(
        "create_kv_store no longer creates Python key_value stores. "
        "Use MCPStore.setup_store(..., cache=...) or store.switch_cache(...) so "
        "Python and Rust runtimes share the Rust-backed cache implementation."
    )


async def create_kv_store_async(*args, **kwargs):
    """Async compatibility placeholder for the removed Python key_value backend."""

    _ = args, kwargs
    raise NotImplementedError(
        "create_kv_store_async no longer creates Python key_value stores. "
        "Use MCPStore.setup_store_async(..., cache=...) or store.switch_cache(...) so "
        "Python and Rust runtimes share the Rust-backed cache implementation."
    )
