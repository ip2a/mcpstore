"""Lazy re-exports for the config module."""

from __future__ import annotations


def __getattr__(name: str):
    if name in {
        "MemoryConfig",
        "StoreConfig",
        "FileConfig",
        "RedisConfig",
        "ValkeyConfig",
        "MemcachedConfig",
        "SqliteConfig",
        "PostgresConfig",
        "DuckDBConfig",
        "RocksDBConfig",
        "DiskConfig",
        "S3Config",
        "DynamoDBConfig",
        "MongoDBConfig",
        "FileTreeConfig",
        "get_namespace",
    }:
        from . import kv_config as _kv_config

        value = getattr(_kv_config, name)
        globals()[name] = value
        return value

    if name in {"LoggingConfig"}:
        from . import config as _config

        value = getattr(_config, name)
        globals()[name] = value
        return value

    raise AttributeError(f"module '{__name__}' has no attribute '{name}'")


__all__ = [
    "LoggingConfig",

    "MemoryConfig",
    "StoreConfig",
    "FileConfig",
    "RedisConfig",
    "ValkeyConfig",
    "MemcachedConfig",
    "SqliteConfig",
    "PostgresConfig",
    "DuckDBConfig",
    "RocksDBConfig",
    "DiskConfig",
    "S3Config",
    "DynamoDBConfig",
    "MongoDBConfig",
    "FileTreeConfig",
    "get_namespace",
]
