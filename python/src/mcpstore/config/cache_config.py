"""
Cache configuration classes for MCPStore.

Non-sensitive configuration is loaded from MCPStoreConfig, sensitive from environment.
"""

from dataclasses import dataclass
from typing import Any, Optional


@dataclass
class BaseCacheConfig:
    """Base cache configuration class with common attributes."""
    timeout: float = 2.0
    retry_attempts: int = 3


@dataclass
class StoreConfig(BaseCacheConfig):
    """Base configuration for one concrete Store."""

    store: str = "memory"
    config: dict[str, Any] | None = None
    namespace: Optional[str] = None

    @property
    def store_name(self) -> str:
        return self.store.strip().lower()

    def __post_init__(self) -> None:
        self.store = self.store.strip().lower()
        if not self.store:
            raise ValueError("Store name cannot be empty")
        if self.config is None:
            self.config = {}


@dataclass
class MemoryConfig(BaseCacheConfig):
    """Memory cache configuration."""
    max_size: Optional[int] = None
    cleanup_interval: int = 300
    store: str = "memory"

    @property
    def store_name(self) -> str:
        return "memory"

    @property
    def config(self) -> dict[str, Any]:
        return {}




@dataclass
class FileConfig(BaseCacheConfig):
    """Local file source for service definitions (e.g. mcp.json).

    The file is the *definition source* and is forwarded to the Rust core via
    ``config_path``.  The runtime Store that backs the cache is in-process
    memory; OpenKeyv has no ``file`` store capable of the CAS/ChangeFeed
    capabilities MCPStore requires.
    """
    path: Optional[str] = None
    namespace: Optional[str] = None

    @property
    def store_name(self) -> str:
        return "memory"

    @property
    def config(self) -> dict[str, Any]:
        return {}


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

    store: str = "redis"

    @property
    def store_name(self) -> str:
        return "redis"

    @property
    def redis_url(self) -> str:
        if self.url:
            return self.url
        if self.password:
            return f"redis://:{self.password}@{self.host}:{self.port or 6379}/{self.db or 0}"
        return f"redis://{self.host}:{self.port or 6379}/{self.db or 0}"

    @property
    def config(self) -> dict[str, Any]:
        return {"url": self.redis_url}

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
