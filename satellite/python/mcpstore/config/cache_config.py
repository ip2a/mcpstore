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






@dataclass
class ValkeyConfig(BaseCacheConfig):
    """Valkey Store source. Fields aligned with OpenKeyv factory.rs."""
    url: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "valkey"

    @property
    def store_name(self) -> str:
        return "valkey"

    @property
    def config(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        if self.url is not None:
            result["url"] = self.url
        return result

    def __post_init__(self) -> None:
        if not self.store:
            raise ValueError("store name cannot be empty")


@dataclass
class MemcachedConfig(BaseCacheConfig):
    """Memcached Store source. ``url`` is required by OpenKeyv factory.rs."""
    url: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "memcached"

    @property
    def store_name(self) -> str:
        return "memcached"

    @property
    def config(self) -> dict[str, Any]:
        return {"url": self.url}

    def __post_init__(self) -> None:
        if not self.url:
            raise ValueError("Memcached configuration requires 'url'")


@dataclass
class SqliteConfig(BaseCacheConfig):
    """SQLite Store source."""
    path: Optional[str] = None
    table: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "sqlite"

    @property
    def store_name(self) -> str:
        return "sqlite"

    @property
    def config(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        if self.path is not None:
            result["path"] = self.path
        if self.table is not None:
            result["table"] = self.table
        return result

    def __post_init__(self) -> None:
        if not self.store:
            raise ValueError("store name cannot be empty")


@dataclass
class PostgresConfig(BaseCacheConfig):
    """Postgres Store source. ``url`` is required by OpenKeyv factory.rs."""
    url: Optional[str] = None
    table: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "postgres"

    @property
    def store_name(self) -> str:
        return "postgres"

    @property
    def config(self) -> dict[str, Any]:
        result: dict[str, Any] = {"url": self.url}
        if self.table is not None:
            result["table"] = self.table
        return result

    def __post_init__(self) -> None:
        if not self.url:
            raise ValueError("Postgres configuration requires 'url'")


@dataclass
class DuckDBConfig(BaseCacheConfig):
    """DuckDB Store source."""
    path: Optional[str] = None
    table: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "duckdb"

    @property
    def store_name(self) -> str:
        return "duckdb"

    @property
    def config(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        if self.path is not None:
            result["path"] = self.path
        if self.table is not None:
            result["table"] = self.table
        return result

    def __post_init__(self) -> None:
        if not self.store:
            raise ValueError("store name cannot be empty")


@dataclass
class RocksDBConfig(BaseCacheConfig):
    """RocksDB Store source. ``path`` is required by OpenKeyv factory.rs."""
    path: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "rocksdb"

    @property
    def store_name(self) -> str:
        return "rocksdb"

    @property
    def config(self) -> dict[str, Any]:
        return {"path": self.path}

    def __post_init__(self) -> None:
        if not self.path:
            raise ValueError("RocksDB configuration requires 'path'")


@dataclass
class DiskConfig(BaseCacheConfig):
    """Disk Store source. ``path`` is required by OpenKeyv factory.rs."""
    path: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "disk"

    @property
    def store_name(self) -> str:
        return "disk"

    @property
    def config(self) -> dict[str, Any]:
        return {"path": self.path}

    def __post_init__(self) -> None:
        if not self.path:
            raise ValueError("Disk configuration requires 'path'")


@dataclass
class S3Config(BaseCacheConfig):
    """S3 Store source. ``bucket`` is required by OpenKeyv factory.rs."""
    bucket: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "s3"

    @property
    def store_name(self) -> str:
        return "s3"

    @property
    def config(self) -> dict[str, Any]:
        return {"bucket": self.bucket}

    def __post_init__(self) -> None:
        if not self.bucket:
            raise ValueError("S3 configuration requires 'bucket'")


@dataclass
class DynamoDBConfig(BaseCacheConfig):
    """DynamoDB Store source. ``table`` is required by OpenKeyv factory.rs."""
    table: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "dynamodb"

    @property
    def store_name(self) -> str:
        return "dynamodb"

    @property
    def config(self) -> dict[str, Any]:
        return {"table": self.table}

    def __post_init__(self) -> None:
        if not self.table:
            raise ValueError("DynamoDB configuration requires 'table'")


@dataclass
class MongoDBConfig(BaseCacheConfig):
    """MongoDB Store source. ``url`` is required by OpenKeyv factory.rs."""
    url: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "mongodb"

    @property
    def store_name(self) -> str:
        return "mongodb"

    @property
    def config(self) -> dict[str, Any]:
        return {"url": self.url}

    def __post_init__(self) -> None:
        if not self.url:
            raise ValueError("MongoDB configuration requires 'url'")


@dataclass
class FileTreeConfig(BaseCacheConfig):
    """FileTree Store source. ``path`` is required by OpenKeyv factory.rs.

    Note: FileTree is a basic Store without CAS/ChangeFeed capabilities; it
    cannot back an MCPStore runtime. Provided for source-declaration parity.
    """
    path: Optional[str] = None
    namespace: Optional[str] = None
    store: str = "filetree"

    @property
    def store_name(self) -> str:
        return "filetree"

    @property
    def config(self) -> dict[str, Any]:
        return {"path": self.path}

    def __post_init__(self) -> None:
        if not self.path:
            raise ValueError("FileTree configuration requires 'path'")


def get_namespace(config: object, default: str = "mcpstore") -> str:
    """Return the configured cache namespace, or the MCPStore default."""
    return getattr(config, "namespace", None) or default
