"""
Cache configuration classes for MCPStore.

Non-sensitive configuration is loaded from MCPStoreConfig, sensitive from environment.
"""

from dataclasses import dataclass
from enum import Enum
from typing import Optional, Literal


class CacheType(Enum):
    """Cache type enumeration."""
    MEMORY = "memory"
    REDIS = "redis"


@dataclass
class BaseCacheConfig:
    """Base cache configuration class with common attributes."""
    timeout: float = 2.0
    retry_attempts: int = 3


@dataclass
class OpenKeyvConfig(BaseCacheConfig):
    """Configuration for any OpenKeyv backend compiled into the Rust core."""

    backend: str = "memory"
    url: Optional[str] = None
    namespace: Optional[str] = None

    @property
    def cache_type(self) -> str:
        return self.backend.strip().lower()

    def __post_init__(self) -> None:
        self.backend = self.backend.strip().lower()
        if not self.backend:
            raise ValueError("OpenKeyv backend name cannot be empty")
        if self.backend != "memory" and not self.url:
            raise ValueError(f"OpenKeyv backend {self.backend!r} requires a URL")


@dataclass
class MemoryConfig(BaseCacheConfig):
    """Memory cache configuration."""
    max_size: Optional[int] = None
    cleanup_interval: int = 300
    cache_type: Literal[CacheType.MEMORY] = CacheType.MEMORY




@dataclass
class RedisConfig(BaseCacheConfig):
    """Redis cache configuration with validation."""

    url: Optional[str] = None
    host: Optional[str] = None
    port: Optional[int] = None
    db: Optional[int] = None
    password: Optional[str] = None
    namespace: Optional[str] = None

    max_connections: int = 50
    retry_on_timeout: bool = True
    socket_keepalive: bool = True
    socket_connect_timeout: float = 5.0
    socket_timeout: float = 5.0
    health_check_interval: int = 30

    allow_partial: bool = False

    cache_type: Literal[CacheType.REDIS] = CacheType.REDIS

    def __post_init__(self):
        """Validate configuration parameters."""
        if not self.allow_partial and not self.url and not self.host:
            raise ValueError(
                "Redis configuration requires either 'url' or 'host'. "
                "Example: RedisConfig(url='redis://localhost:6379/0') or "
                "RedisConfig(host='localhost', port=6379)"
            )

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

        if self.max_connections <= 0:
            raise ValueError(
                f"max_connections must be positive, got: {self.max_connections}. "
                "Example: RedisConfig(url='redis://localhost:6379/0', max_connections=50)"
            )




def get_namespace(config: object, default: str = "mcpstore") -> str:
    """Return the configured cache namespace, or the MCPStore default."""
    return getattr(config, "namespace", None) or default
